use crate::block::{fields, Block, BlockFactory, BlockId, BlockInfo, FsId};
use crate::error::Error;
use crate::logging::log;
use crate::storage::Storage;
use crate::utils::trim_block_idx_with_wraparound;

#[derive(Debug)]
pub struct Filesystem<'a, S: Storage, const BS: usize> {
    storage: &'a mut S,
    id: FsId,
    offset: usize,
    blk_factory: BlockFactory,
    is_empty: bool,
    is_full: bool,
    buffer: [u8; BS],
}

impl<'a, S: Storage, const BS: usize> Filesystem<'a, S, BS> {
    pub const BLOCK_SIZE: usize = BS;

    // will create new filesystem or restore previous in case previous one has the same fs_id
    pub fn new(storage: &'a mut S, fs_id: FsId) -> Result<Self, Error> {
        let mut fs = Filesystem {
            storage,
            id: fs_id,
            offset: 0,
            blk_factory: BlockFactory::new(),
            is_empty: true,
            is_full: false,
            buffer: [0_u8; BS],
        };
        fs.init()?;

        Ok(fs)
    }

    /// Restore filesystem from storage, use fs_id from first block as id for the filesystem
    pub fn restore(storage: &'a mut S) -> Result<Self, Error> {
        let buf = &mut [0_u8; BS];
        let first_block = storage.min_block_index();
        storage.read(first_block, buf)?;
        let info = BlockInfo::<BS>::from_buffer(buf);
        if !info.is_valid {
            return Err(Error::InvalidHeaderBlock);
        }
        log!(trace, "Restore storage with fs is: {}", info.fs_id);
        Self::new(storage, info.fs_id)
    }

    fn setup_attributes(
        &mut self,
        next_offset: usize,
        next_id: BlockId,
        is_empty: bool,
        is_full: bool,
    ) {
        log!(
            debug,
            "Setup fs attributes, offset: {:?}, block_id: {:?}, is_empty: {:?}, is_full: {:?}",
            next_offset,
            next_id,
            is_empty,
            is_full
        );
        self.offset = next_offset;
        self.blk_factory.set_id(next_id);
        self.is_empty = is_empty;
        self.is_full = is_full;
    }

    pub fn append<F>(&mut self, writer: F) -> Result<usize, Error>
    where
        F: FnOnce(&mut [u8]),
    {
        let blk_len = self.storage.block_size();
        let data_buf = &mut self.buffer[..blk_len];
        let _ = self
            .blk_factory
            .create_with_writer::<_, BS>(data_buf, self.id, writer);

        log!(trace, "Appending to offset: {}", self.offset);
        self.storage.write(self.offset, data_buf)?;
        self.is_empty = false;
        if self.offset == self.storage.max_block_index() - 1 {
            log!(trace, "Fs is full, next write will overwrite old data");
            self.is_full = true;
        }

        self.incr_offset();
        log!(trace, "Offset changed to {}", self.offset);

        Ok(Self::data_block_size())
    }

    /// Read data from the beginning of the stream (the oldest write).
    pub fn read<F>(&mut self, blk_offset: usize, reader: F) -> Result<usize, Error>
    where
        F: FnOnce(&[u8]),
    {
        // self.offset is next position for write, so it is the oldest position for read
        // in case storage is full, next offset will be position of oldest write
        // in case storage is NOT full, first block will be position of oldest write
        let base_offset = if self.is_full() {
            let base = self.offset + blk_offset;
            log!(trace, "Read from full storage with base offset: {}", base);
            base
        } else {
            let base = self.data_blk_offset() + blk_offset;
            log!(trace, "Read from empty storage with base offset: {}", base);
            base
        };

        let offset = self.trim_offset(base_offset);

        let blk_len = self.storage.block_size();
        let data_buf = &mut self.buffer[..blk_len];

        log!(trace, "Read (trimmed) offset {}", offset);
        self.storage.read(offset, data_buf)?;

        {
            let block = Block::<BS>::from_buffer(data_buf);
            if !block.is_valid() {
                log!(debug, "Block at {} is invalid", offset);
                return Err(Error::NotValidBlockForRead);
            }
        }
        reader(&data_buf[fields::DATA_BEGIN..]);
        Ok(Self::data_block_size())
    }

    pub const fn data_block_size() -> usize {
        BS - Block::<BS>::attributes_size()
    }

    pub fn incr_offset(&mut self) {
        self.offset = self.trim_offset(self.offset + 1);
    }

    fn data_blk_offset(&self) -> usize {
        // first block is FS config, so add 1
        self.storage.min_block_index() + 1
    }

    fn trim_offset(&self, offset: usize) -> usize {
        trim_block_idx_with_wraparound(
            offset,
            self.data_blk_offset(),
            self.storage.max_block_index(),
        )
    }

    fn init(&mut self) -> Result<(), Error> {
        let mut buf = [0_u8; BS];
        let buf = &mut buf[..];
        let (read_buf, _) = buf.split_at_mut(self.storage.block_size());

        let mut begin = self.storage.min_block_index();
        let mut end = self.storage.max_block_index();

        log!(debug, "Init storage with begin: {}, end: {}", begin, end);
        if begin > usize::MAX - 2 || end < begin + 2 {
            return Err(Error::TooSmallFilesystem);
        }

        {
            self.storage.read(begin, &mut read_buf[..])?;
            let left_block = BlockInfo::<BS>::from_buffer(read_buf);
            if !left_block.is_valid || left_block.fs_id != self.id {
                // storage wasn't formatted, it is empty, offset is begin
                log!(debug, "Storage was not formatted. Making empty one");
                let is_empty = true;
                let is_full = false;
                self.write_config(begin)?;
                self.setup_attributes(begin + 1, 0, is_empty, is_full);
                return Ok(());
            }
        }

        begin += 1;
        self.storage.read(begin, &mut read_buf[..])?;
        let left_block = BlockInfo::<BS>::from_buffer(read_buf);
        if !left_block.is_valid || left_block.fs_id != self.id {
            // storage was formatted, but first block was not written, it is empty, offset is begin
            log!(
                debug,
                "Storage was formatted, but first block is not valid. Treat it as empty storage"
            );
            let is_empty = true;
            let is_full = false;
            self.setup_attributes(begin, 0, is_empty, is_full);
            return Ok(());
        }
        // as first block is valid is can't be empty
        let is_empty = false;

        self.storage.read(end - 1, &mut read_buf[..])?;
        let mut right_block = BlockInfo::<BS>::from_buffer(read_buf);
        if right_block.is_valid && right_block.fs_id == self.id && right_block.id > left_block.id {
            // wraparound is after end, next block to write is begin
            log!(debug, "Storage is full, wraparound is after last block, next block is first storage block");
            let is_empty = false;
            let is_full = true;
            self.setup_attributes(begin, right_block.id + 1, is_empty, is_full);
            return Ok(());
        }

        let is_full = right_block.is_valid;

        // must be always the same as begin.id
        let mut last_id = left_block.id;

        // at least 2 elements must be present
        // will found only wraparound, last block must be checked to have wraparound
        // begin of the range will always point to last written element
        while end - begin > 2 {
            let mid = (begin + end) / 2;

            self.storage.read(mid, &mut read_buf[..])?;
            let mid_block = BlockInfo::<BS>::from_buffer(read_buf);
            log!(trace, "Mid: {:?}, right: {:?}", &mid_block, right_block);

            if self.can_have_tail(&mid_block, &right_block) {
                begin = mid;
                last_id = mid_block.id;
            } else {
                end = mid + 1;
                right_block = mid_block;
            };
        }

        // in case not all memory was used wraparound will not exists,
        // place for new block will be after last block
        if end - begin == 2 {
            self.storage.read(begin + 1, &mut read_buf[..])?;
            let block_inf = BlockInfo::<BS>::from_buffer(read_buf);
            log!(trace, "Possible right block: {:?}", &block_inf);
            if block_inf.is_valid && block_inf.fs_id == self.id && block_inf.id > last_id {
                begin += 1;
                last_id = block_inf.id;
            }
        }

        // begin will be last value before wraparound
        self.setup_attributes(begin + 1, last_id + 1, is_empty, is_full);
        Ok(())
    }

    fn can_have_tail(&self, left: &BlockInfo<BS>, right: &BlockInfo<BS>) -> bool {
        if !left.is_valid || left.fs_id != self.id {
            return false;
        }

        if !right.is_valid || right.fs_id != self.id {
            return true;
        }

        left.id > right.id
    }

    fn write_config(&mut self, blk_idx: usize) -> Result<(), Error> {
        let mut config_was_not_written = false;
        let data_buf = &mut [0_u8; BS];
        let _ = self
            .blk_factory
            .create_with_writer::<_, BS>(data_buf, self.id, |block_data| {
                let config = config_block::FsConfigBlock::new();
                let config_data = config_block::FsConfigBlock::to_be_bytes(&config);
                // TODO: add error when data.len() > block_data.len()
                let to_copy = core::cmp::min(config_data.len(), block_data.len());
                if to_copy != config_data.len() {
                    config_was_not_written = true;
                }
                block_data[..to_copy].copy_from_slice(&config_data[..to_copy]);
            });
        self.storage.write(blk_idx, data_buf)?;

        if config_was_not_written {
            return Err(Error::CanNotWriteConfig);
        }

        Ok(())
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn next_blk_id(&self) -> BlockId {
        self.blk_factory.id
    }

    pub fn id(&self) -> FsId {
        self.id
    }

    pub fn is_empty(&self) -> bool {
        self.is_empty
    }

    pub fn is_full(&self) -> bool {
        self.is_full
    }
}

#[derive(Debug)]
pub struct FsInitAttrs {
    pub next_offset: usize,
    pub next_id: BlockId,
}

pub mod config_block {

    /// To add new field:
    /// - add ${FIELD}_BEGIN, ${FIELD}_LEN, ${FIELD}_END, constants
    /// - possible change BLOCK_END constant in case this field will be last one
    /// - implement method write_${field} for FsConfigBlock, see `write_version` as an example
    /// - call `write_${field}` method in `to_be_bytes`
    /// - implement method read_${field} for FsConfigBlock, see `read_version` as an example
    /// - call `read_${field}` method in `from_be_bytes`

    pub type Version = u32;

    // add mapping to map FS_VERSION to package version (detect braking changes)
    pub const FS_VERSION: Version = 0x1;

    pub(crate) const BLOCK_BEGIN: usize = 0;

    pub(crate) const VERSION_BEGIN: usize = BLOCK_BEGIN;
    pub(crate) const VERSION_LEN: usize = core::mem::size_of::<Version>();
    pub(crate) const VERSION_END: usize = VERSION_BEGIN + VERSION_LEN;

    pub(crate) const BLOCK_END: usize = VERSION_END;
    pub(crate) const BLOCK_LEN: usize = BLOCK_END - BLOCK_BEGIN;

    #[derive(Debug)]
    pub struct FsConfigBlock {
        pub version: Version,
    }

    impl FsConfigBlock {
        pub fn new() -> FsConfigBlock {
            FsConfigBlock {
                version: FS_VERSION,
            }
        }

        /// Can be as member method
        /// implemented it as non member method to be aligned with to_be_bytes method in other types
        pub fn to_be_bytes(config: &FsConfigBlock) -> [u8; BLOCK_LEN] {
            let mut buf = [0_u8; BLOCK_LEN];

            config.write_version(&mut buf);

            buf
        }

        fn write_version(&self, buf: &mut [u8; BLOCK_LEN]) {
            let version = self.version.to_be_bytes();
            buf[VERSION_BEGIN..VERSION_END].copy_from_slice(&version[..]);
        }

        pub fn from_be_bytes(block: [u8; BLOCK_LEN]) {
            let mut config: FsConfigBlock = FsConfigBlock::default();
            config.read_version(&block);
        }

        fn read_version(&mut self, block: &[u8; BLOCK_LEN]) {
            let mut buf = [0_u8; VERSION_LEN];
            buf[..].copy_from_slice(&block[VERSION_BEGIN..VERSION_END]);
            self.version = Version::from_be_bytes(buf);
        }
    }

    impl Default for FsConfigBlock {
        fn default() -> Self {
            FsConfigBlock {
                version: Version::default(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Block, BlockInfo, Filesystem};
    use crate::block::BlockFactory;
    use crate::error::Error;
    use crate::storage::ram::RamStorage;
    use crate::utils::slices_are_equal;

    const FS_ID: u32 = 522285587;

    #[test]
    fn test_fs_init() {
        crate::logging::init();

        const BLOCK_SIZE: usize = 128;
        const BLOCK_COUNT: usize = 512;
        const SIZE: usize = BLOCK_SIZE * BLOCK_COUNT;
        // first block is fs config block
        const AVAILABLE_BLOCK_COUNT: usize = BLOCK_COUNT - 1;
        const AVAILABLE_SIZE: usize = BLOCK_SIZE * AVAILABLE_BLOCK_COUNT;

        type DefaultStorage = RamStorage<SIZE, BLOCK_SIZE>;
        type Fs<'a> = Filesystem<'a, DefaultStorage, BLOCK_SIZE>;

        let mut storage = DefaultStorage::new().expect("Can't create storage for test_fs_full");

        {
            let first_block = BlockInfo::<BLOCK_SIZE>::from_buffer(&storage.data[..BLOCK_SIZE]);
            assert!(
                !first_block.is_valid,
                "First block must not be valid, it contains invalid crc!"
            );
        }

        {
            let fs = Fs::new(&mut storage, FS_ID).expect("Can't create fs for test_fs_empty");
            assert_eq!(
                fs.offset, 1,
                "Storage has no writes, offset must be eq to 1 (0 is config block, next is 1)"
            );
        }

        let begin_id = 42;
        let mut factory = BlockFactory::new();
        factory.set_id(begin_id);

        let mut i = 0_u8;
        let mut fill_block = |blk_data: &mut [u8]| {
            blk_data.fill(i);
            i = if i == u8::MAX { 0 } else { i + 1 };
        };

        // fill n-th block,
        // first AVAILABLE_BLOCK_COUNT iterations test offset initialization for not full storage.
        // next 2 * AVAILABLE_BLOCK_COUNT iterations test offset initialization for full storage after wraparound
        for i in 0..AVAILABLE_BLOCK_COUNT * 3 {
            // first block is fs config block, so add 1 block offset
            let begin = (i * BLOCK_SIZE) % AVAILABLE_SIZE + 1 * BLOCK_SIZE;
            let end = begin + BLOCK_SIZE;

            let blk = factory.create_with_writer::<_, BLOCK_SIZE>(
                &mut storage.data[begin..end],
                FS_ID,
                &mut fill_block,
            );

            let cur_id = begin_id + i as u64;
            assert_eq!(blk.id(), cur_id);

            {
                let fs = Fs::new(&mut storage, FS_ID).expect("Can't create fs for test_fs_full");
                // first block is skipped so always add 1 to expected offset
                let expected_offset = 1 + (i + 1) % AVAILABLE_BLOCK_COUNT;
                assert_eq!(fs.offset, expected_offset);

                assert_eq!(fs.blk_factory.id, cur_id + 1);
            }
        }

        const NEW_BLOCKS: usize = 35;
        const NEW_FS_ID: u32 = 1585159336;

        // init new blocks with new fs id
        for b in 0..NEW_BLOCKS {
            let begin = b * BLOCK_SIZE;
            let end = begin + BLOCK_SIZE;
            let block_data = &mut storage.data[begin..end];
            // write different fs id to first blocks
            Block::<'_, 256>::set_fs_id(block_data, NEW_FS_ID);
            Block::<'_, 256>::set_crc(block_data);
        }

        // validate storage blockes were actually initialized and they are valid
        for b in 0..AVAILABLE_BLOCK_COUNT {
            let begin = b * BLOCK_SIZE;
            let end = begin + BLOCK_SIZE;
            let block = BlockInfo::<BLOCK_SIZE>::from_buffer(&storage.data[begin..end]);
            // let first_block = BlockInfo::<BLOCK_SIZE>::from_buffer();
            assert!(block.is_valid, "Block {} must be valid after write!", b);

            if b < NEW_BLOCKS {
                assert_eq!(
                    block.fs_id, NEW_FS_ID,
                    "First blocks must be init with new fs id"
                );
            } else {
                assert_eq!(
                    block.fs_id, FS_ID,
                    "Last blocks must be init with old fs id"
                );
            }
        }

        {
            let fs = Fs::new(&mut storage, NEW_FS_ID).expect("Can't create fs for new blocks");
            assert_eq!(
                fs.offset, NEW_BLOCKS,
                "Storage was initialized, offset must be after last new block, old blocks must be skipped during fs init"
            );
        }
    }

    #[test]
    fn test_fs_io() {
        crate::logging::init();

        const BLOCK_SIZE: usize = 128;
        const BLOCK_COUNT: usize = 80;
        const SIZE: usize = BLOCK_SIZE * BLOCK_COUNT;
        // first block is fs config block
        const AVAILABLE_BLOCK_COUNT: usize = BLOCK_COUNT - 1;
        const AVAILABLE_SIZE: usize = BLOCK_SIZE * AVAILABLE_BLOCK_COUNT;

        type DefaultStorage = RamStorage<SIZE, BLOCK_SIZE>;
        type Fs<'a> = Filesystem<'a, DefaultStorage, BLOCK_SIZE>;

        const DATA_SIZE: usize = Fs::data_block_size();

        let mut storage = DefaultStorage::new().expect("Can't create storage for test_fs_full");

        // write n-th block,
        // first BLOCK_COUNT iterations test IO for not full storage.
        // next 2 * BLOCK_COUNT iterations test IO for full storage after wraparound
        for i in 0..AVAILABLE_BLOCK_COUNT * 3 {
            // first block is fs config block, so add 1 block offset, to get block end add additional 1 block offset
            let end = (i * BLOCK_SIZE) % AVAILABLE_SIZE + 2 * BLOCK_SIZE;
            let begin = end - DATA_SIZE;
            let mut expected_data = [0_u8; DATA_SIZE];
            expected_data.copy_from_slice(&storage.data[begin..end]);

            let mut fs = Fs::new(&mut storage, FS_ID).expect("Can't create fs for test_fs_full");

            if i == 0 {
                assert!(fs.is_empty(), "Before first write fs must be empty!");
            } else {
                assert!(
                    !fs.is_empty(),
                    "After first write fs must be non empty! i: {}",
                    i
                );
            }

            if i < AVAILABLE_BLOCK_COUNT {
                assert!(
                    !fs.is_full(),
                    "Fs can't be full before AVAILABLE_BLOCK_COUNT writes, i: {}",
                    i
                );
            } else {
                assert!(
                    fs.is_full(),
                    "Fs must be full after AVAILABLE_BLOCK_COUNT writes, i: {}",
                    i
                );
            }

            // read the oldest block, that will be overwritten
            let blk_offset = if i >= AVAILABLE_BLOCK_COUNT { 0 } else { i };
            let read_before = fs.read(blk_offset, |blk_data| {
                assert!(
                    slices_are_equal(&expected_data[..], &blk_data[..]),
                    "Wrong data was read at i: {}, {:?} vs {:?}",
                    i,
                    &expected_data[..],
                    &blk_data[..]
                );
            });

            match read_before {
                Ok(_) => {
                    assert!(
                        i >= AVAILABLE_BLOCK_COUNT,
                        "Data must be read only after wraparound, i: {}",
                        i
                    );
                }
                Err(Error::NotValidBlockForRead) => {
                    assert!(
                        i < AVAILABLE_BLOCK_COUNT,
                        "Data must not be read before wraparound, i: {}",
                        i
                    );
                }
                Err(e) => {
                    assert!(
                        false,
                        "Err read data before write at i: {}, err: {:?}",
                        i, e
                    );
                }
            }

            assert!(
                i < u8::MAX as usize,
                "I will be wrapped around, can't continue test."
            );

            let fill_value = (i + 1) as u8;
            let write = fs.append(|blk_data| {
                blk_data.fill(fill_value);
            });
            assert!(write.is_ok(), "Err write data i: {}, err: {:?}", i, write);

            expected_data.fill(fill_value);

            const LAST_WRITE_BEFORE_FS_BECOME_FULL: usize = AVAILABLE_BLOCK_COUNT - 1;
            let blk_offset = if i < LAST_WRITE_BEFORE_FS_BECOME_FULL {
                assert!(!fs.is_full(), "Fs must not be full after write {}", i);
                i
            } else {
                assert!(fs.is_full(), "Fs must be full after write {}", i);
                AVAILABLE_BLOCK_COUNT - 1
            };
            let read_after = fs.read(blk_offset, |blk_data| {
                assert!(
                    slices_are_equal(&expected_data[..], &blk_data[..]),
                    "Wrong data was read after write at i: {}, {:?} vs {:?}",
                    i,
                    &expected_data[..],
                    &blk_data[..]
                );
            });
            assert!(
                read_after.is_ok(),
                "Err read data after write at i: {}, err: {:?}",
                i,
                read_after
            );
        }
    }
}
