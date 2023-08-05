use crate::block::{Block, BlockInfo};
use crate::error::StorageError;
use crate::storage::Storage;

pub struct Filesystem<S: Storage, const N: usize> {
    storage: S,
    offset: usize,
}

impl<S: Storage, const N: usize> Filesystem<S, N> {
    pub fn new(storage: S) -> Result<Self, StorageError> {
        let mut fs = Filesystem { storage, offset: 0 };
        fs.offset = fs.find_offset()?;

        Ok(fs)
    }

    pub fn append(&mut self, data: &[u8]) -> Result<usize, StorageError> {
        let data = &data[..self.data_block_size()];
        // TODO: add headers
        self.storage.write(self.offset, data)?;
        self.incr_offset();

        Ok(data.len())
    }

    pub fn data_block_size(&self) -> usize {
        self.storage.block_size() - Block::attributes_size()
    }

    pub fn incr_offset(&mut self) {
        self.offset =
            (self.offset + 1) % self.storage.max_block_index() + self.storage.min_block_index();
    }

    fn find_offset(&mut self) -> Result<usize, StorageError> {
        let mut buf = [0_u8; N];
        let buf = &mut buf[..];
        let (read_buf, _) = buf.split_at_mut(self.storage.block_size());

        let mut begin = self.storage.min_block_index();
        let mut end = self.storage.max_block_index();
        if begin > usize::MAX - 2 || end < begin + 2 {
            return Err(StorageError::TooSmallStorage);
        }

        self.storage.read(begin, &mut read_buf[..])?;
        let left_block = BlockInfo::from_buffer(&read_buf[..]);
        if !left_block.is_valid {
            // storage wasn't formatted, it is empty, offset is begin
            return Ok(begin);
        }

        self.storage.read(end - 1, &mut read_buf[..])?;
        let mut right_block = BlockInfo::from_buffer(&read_buf[..]);
        if right_block.is_valid && right_block.id > left_block.id {
            // wraparound is after end, next block to write is begin
            return Ok(begin);
        }

        // at least 2 elements must be present
        while end - begin >= 2 {
            let mid = (begin + end) / 2;

            self.storage.read(mid, &mut read_buf[..])?;
            let mid_block = BlockInfo::from_buffer(&read_buf[..]);

            if Self::can_have_tail(&mid_block, &right_block) {
                begin = mid;
            } else {
                end = mid + 1;
                right_block = mid_block;
            };
        }

        // begin will be last value before wraparound
        Ok(begin + 1)
    }

    fn can_have_tail(left: &BlockInfo, right: &BlockInfo) -> bool {
        if !left.is_valid {
            return false;
        }

        if !right.is_valid {
            return true;
        }

        left.id > right.id
    }
}

#[cfg(test)]
mod tests {
    use super::{BlockInfo, Filesystem};
    use crate::storage::RamStorage;

    const BLOCK: usize = 256;
    const SIZE: usize = BLOCK * 4 * 128;

    type DefaultStorage = RamStorage<SIZE, BLOCK>;
    type Fs = Filesystem<DefaultStorage, BLOCK>;

    #[test]
    fn test_fs_empty() {
        let storage = DefaultStorage::new().expect("Can't create storage for test_fs_empty");

        let first_block = BlockInfo::from_buffer(&storage.data[..BLOCK]);
        assert!(
            !first_block.is_valid,
            "First block must not be valid, it contains invalid crc!"
        );

        let fs = Fs::new(storage).expect("Can't create fs for test_fs_empty");
        assert_eq!(
            fs.offset, 0,
            "Storage was not initialized, offset must be eq to 0"
        );
    }

    #[test]
    fn test_fs_full() {
        let storage = DefaultStorage::new().expect("Can't create storage for test_fs_full");

        let first_block = BlockInfo::from_buffer(&storage.data[..BLOCK]);
        assert!(
            !first_block.is_valid,
            "First block must not be valid, it contains invalid crc!"
        );

        let fs = Fs::new(storage).expect("Can't create fs for test_fs_full");
        assert_eq!(
            fs.offset, 0,
            "Storage was not full, last written block was at last blk_idx, offset must be eq to 0"
        );
    }
}
