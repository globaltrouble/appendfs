use crate::block::{fields, Block, BlockFactory, BlockInfo, ID};
use crate::error::Error;
use crate::storage::Storage;
use crate::utils::trim_block_idx_with_wraparound;

#[derive(Debug)]
pub struct Filesystem<S: Storage, const BS: usize> {
    storage: S,
    offset: usize,
    blk_factory: BlockFactory,
    is_empty: bool,
    is_full: bool,
    buffer: [u8; BS],
}

impl<S: Storage, const BS: usize> Filesystem<S, BS> {
    pub const BLOCK_SIZE: usize = BS;

    pub fn new(storage: S) -> Result<Self, Error> {
        let mut fs = Filesystem {
            storage,
            offset: 0,
            blk_factory: BlockFactory::new(),
            is_empty: true,
            is_full: false,
            buffer: [0_u8; BS],
        };
        fs.init()?;

        Ok(fs)
    }

    fn init_attrs(&mut self, next_offset: usize, next_id: ID, is_empty: bool, is_full: bool) {
        self.offset = next_offset;
        self.blk_factory.set_id(next_id);
        self.is_empty = is_empty;
        self.is_full = is_full;
    }

    pub fn release(self) -> S {
        self.storage
    }

    pub fn append<F>(&mut self, writer: F) -> Result<usize, Error>
    where
        F: FnOnce(&mut [u8]),
    {
        let blk_len = self.storage.block_size();
        let data_buf = &mut self.buffer[..blk_len];
        let _ = self
            .blk_factory
            .create_with_writer::<_, BS>(data_buf, writer);
        self.storage.write(self.offset, data_buf)?;

        self.is_empty = false;
        if self.offset == self.storage.max_block_index() - 1 {
            self.is_full = true;
        }

        self.incr_offset();

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
            self.offset + blk_offset
        } else {
            self.storage.min_block_index() + blk_offset
        };

        let offset = self.trim_offset(base_offset);
        let blk_len = self.storage.block_size();
        let data_buf = &mut self.buffer[..blk_len];
        self.storage.read(offset, data_buf)?;

        {
            let block = Block::<BS>::from_buffer(data_buf);
            if !block.is_valid() {
                return Err(Error::NotValidBlock);
            }
        }
        reader(&data_buf[fields::DATA_BEGIN..]);
        Ok(Self::data_block_size())
    }

    pub const fn data_block_size() -> usize {
        BS - Block::<BS>::attributes_size()
    }

    pub fn incr_offset(&mut self) {
        self.offset = self.trim_offset(self.offset + 1)
    }

    fn trim_offset(&self, offset: usize) -> usize {
        trim_block_idx_with_wraparound(
            offset,
            self.storage.min_block_index(),
            self.storage.max_block_index(),
        )
    }

    fn init(&mut self) -> Result<(), Error> {
        let mut buf = [0_u8; BS];
        let buf = &mut buf[..];
        let (read_buf, _) = buf.split_at_mut(self.storage.block_size());

        let mut begin = self.storage.min_block_index();
        let mut end = self.storage.max_block_index();
        if begin > usize::MAX - 2 || end < begin + 2 {
            return Err(Error::TooSmallFilesystem);
        }

        self.storage.read(begin, &mut read_buf[..])?;
        let left_block = BlockInfo::<BS>::from_buffer(read_buf);
        if !left_block.is_valid {
            // storage wasn't formatted, it is empty, offset is begin
            let is_empty = true;
            let is_full = false;
            self.init_attrs(begin, 0, is_empty, is_full);
            return Ok(());
        }
        // as first block is valid is can't be empty
        let is_empty = false;

        self.storage.read(end - 1, &mut read_buf[..])?;
        let mut right_block = BlockInfo::<BS>::from_buffer(read_buf);
        if right_block.is_valid && right_block.id > left_block.id {
            // wraparound is after end, next block to write is begin
            let is_empty = false;
            let is_full = true;
            self.init_attrs(begin, right_block.id + 1, is_empty, is_full);
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

            if Self::can_have_tail(&mid_block, &right_block) {
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
            if block_inf.is_valid && block_inf.id > last_id {
                begin += 1;
                last_id = block_inf.id;
            }
        }

        // begin will be last value before wraparound
        self.init_attrs(begin + 1, last_id + 1, is_empty, is_full);
        Ok(())
    }

    fn can_have_tail(left: &BlockInfo<BS>, right: &BlockInfo<BS>) -> bool {
        if !left.is_valid {
            return false;
        }

        if !right.is_valid {
            return true;
        }

        left.id > right.id
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn next_id(&self) -> u64 {
        self.blk_factory.id
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
    pub next_id: ID,
}

#[cfg(test)]
mod tests {
    use super::{BlockInfo, Filesystem};
    use crate::block::BlockFactory;
    use crate::error::Error;
    use crate::storage::ram::RamStorage;
    use crate::utils::slices_are_equal;

    #[test]
    fn test_fs_init() {
        const BLOCK_SIZE: usize = 256;
        const BLOCK_COUNT: usize = 512;
        const SIZE: usize = BLOCK_SIZE * BLOCK_COUNT;

        type DefaultStorage = RamStorage<SIZE, BLOCK_SIZE>;
        type Fs = Filesystem<DefaultStorage, BLOCK_SIZE>;

        let storage = DefaultStorage::new().expect("Can't create storage for test_fs_full");

        let first_block = BlockInfo::<BLOCK_SIZE>::from_buffer(&storage.data[..BLOCK_SIZE]);
        assert!(
            !first_block.is_valid,
            "First block must not be valid, it contains invalid crc!"
        );

        let fs = Fs::new(storage).expect("Can't create fs for test_fs_empty");
        assert_eq!(
            fs.offset, 0,
            "Storage was not initialized, offset must be eq to 0"
        );

        let mut storage = fs.release();

        let begin_id = 42;
        let mut factory = BlockFactory::new();
        factory.set_id(begin_id);

        let mut i = 0_u8;
        let mut fill_block = |blk_data: &mut [u8]| {
            blk_data.fill(i);
            i = if i == u8::MAX { 0 } else { i + 1 };
        };

        // fill n-th block,
        // first BLOCK_COUNT iterations test offset initialization for not full storage.
        // next 2 * BLOCK_COUNT iterations test offset initialization for full storage after wraparound
        for i in 0..BLOCK_COUNT * 3 {
            let begin = (i * BLOCK_SIZE) % SIZE;
            let end = begin + BLOCK_SIZE;

            let blk = factory.create_with_writer::<_, BLOCK_SIZE>(
                &mut storage.data[begin..end],
                &mut fill_block,
            );

            let cur_id = begin_id + i as u64;
            assert_eq!(blk.id(), cur_id);

            let fs = Fs::new(storage).expect("Can't create fs for test_fs_full");
            let expected_offset = (i + 1) % BLOCK_COUNT;
            assert_eq!(fs.offset, expected_offset);

            assert_eq!(fs.blk_factory.id, cur_id + 1);

            storage = fs.release();
        }
    }

    #[test]
    fn test_fs_io() {
        const BLOCK_SIZE: usize = 128;
        const BLOCK_COUNT: usize = 80;
        const SIZE: usize = BLOCK_SIZE * BLOCK_COUNT;

        type DefaultStorage = RamStorage<SIZE, BLOCK_SIZE>;
        type Fs = Filesystem<DefaultStorage, BLOCK_SIZE>;

        const DATA_SIZE: usize = Fs::data_block_size();

        let mut storage = DefaultStorage::new().expect("Can't create storage for test_fs_full");

        // write n-th block,
        // first BLOCK_COUNT iterations test IO for not full storage.
        // next 2 * BLOCK_COUNT iterations test IO for full storage after wraparound
        for i in 0..BLOCK_COUNT * 3 {
            let end = (i * BLOCK_SIZE) % SIZE + BLOCK_SIZE;
            let begin = end - DATA_SIZE;
            let mut expected_data = [0_u8; DATA_SIZE];
            expected_data.copy_from_slice(&storage.data[begin..end]);

            let mut fs = Fs::new(storage).expect("Can't create fs for test_fs_full");

            if i == 0 {
                assert!(fs.is_empty(), "Before first write fs must be empty!");
            } else {
                assert!(
                    !fs.is_empty(),
                    "After first write fs must be non empty! i: {}",
                    i
                );
            }

            if i < BLOCK_COUNT {
                assert!(
                    !fs.is_full(),
                    "Fs can't be full before BLOCK_COUNT writes, i: {}",
                    i
                );
            } else {
                assert!(
                    fs.is_full(),
                    "Fs must be full after BLOCK_COUNT writes, i: {}",
                    i
                );
            }

            // read the oldest block, that will be overwritten
            let blk_offset = if i >= BLOCK_COUNT { 0 } else { i };
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
                        i >= BLOCK_COUNT,
                        "Data must be read only after wraparound, i: {}",
                        i
                    );
                }
                Err(Error::NotValidBlock) => {
                    assert!(
                        i < BLOCK_COUNT,
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

            let blk_offset = if i >= BLOCK_COUNT - 1 {
                assert!(fs.is_full(), "Fs must be full after write {}", i);
                BLOCK_COUNT - 1
            } else {
                assert!(!fs.is_full(), "Fs must not be full after write {}", i);
                i
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

            storage = fs.release();
        }
    }
}
