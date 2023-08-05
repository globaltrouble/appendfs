#![no_std]

use crc;

pub type CrcValue = u16;
pub const CRC_ALGORITHM: crc::Crc<CrcValue> = crc::Crc::<CrcValue>::new(&crc::CRC_16_CDMA2000);

// TODO: panic if block_size() * 2 > N

pub enum StorageError {
    TooSmallStorage,
    BlockOutOfRange,
    NotEnoughSpace,
    DataLenNotEqualToBlockSize,
}

pub trait Storage {
    fn read(&mut self, blk_idx: usize, data: &mut [u8]) -> Result<usize, StorageError>;
    fn write(&mut self, blk_idx: usize, data: &[u8]) -> Result<usize, StorageError>;

    // Make as member functions to make it configurable
    fn block_size(&self) -> usize;
    fn min_block_index(&self) -> usize;
    fn max_block_index(&self) -> usize;
}

pub struct Block<'a> {
    pub data: &'a [u8],
    pub crc: CrcValue,
}

impl<'a> Block<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        let crc = Self::calculated_crc(data);
        Self { data, crc }
    }

    pub fn from_other(other: Block<'a>) -> Self {
        Self {
            data: other.data,
            crc: other.crc,
        }
    }

    pub fn id(&self) -> u64 {
        const LEN: usize = core::mem::size_of::<u64>();
        let mut data = [0_u8; LEN];
        data[..].copy_from_slice(&self.data[..LEN]);

        u64::from_be_bytes(data)
    }

    pub fn is_valid(&self) -> bool {
        self.stored_crc() == self.crc
    }

    pub fn stored_crc(&self) -> CrcValue {
        const LEN: usize = core::mem::size_of::<u16>();
        let mut data = [0_u8; LEN];
        data[..].copy_from_slice(&self.data[..LEN]);

        CrcValue::from_be_bytes(data)
    }

    pub fn calculated_crc(data: &[u8]) -> CrcValue {
        let len = data.len() - core::mem::size_of::<CrcValue>();
        CRC_ALGORITHM.checksum(&data[..len])
    }
}

pub struct BlockInfo {
    id: u64,
    is_valid: bool,
}

impl BlockInfo {
    pub fn from_block(block: &Block) -> BlockInfo {
        let is_valid = block.is_valid();
        let id = if is_valid { block.id() } else { 0 };

        Self { id, is_valid }
    }

    pub fn from_buffer(data: &[u8]) -> BlockInfo {
        Self::from_block(&Block::new(data))
    }
}

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
        let data = &data[..self.storage.block_size()];
        self.storage.write(self.offset, data)?;
        self.incr_offset();

        Ok(data.len())
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

pub struct RamStorage<const S: usize, const B: usize> {
    data: [u8; S],
}

impl<const S: usize, const B: usize> RamStorage<S, B> {
    pub fn new() -> Result<Self, ()> {
        if S % B != 0 || S < 2 * B {
            return Err(());
        }

        Ok(Self { data: [0_u8; S] })
    }

    fn validate_block_index(&self, blk_idx: usize) -> Result<(), StorageError> {
        if blk_idx < self.min_block_index() || blk_idx >= self.max_block_index() {
            return Err(StorageError::BlockOutOfRange);
        }

        Ok(())
    }
}

impl<const S: usize, const B: usize> Storage for RamStorage<S, B> {
    fn read(&mut self, blk_idx: usize, data: &mut [u8]) -> Result<usize, StorageError> {
        self.validate_block_index(blk_idx)?;

        if data.len() < self.block_size() {
            return Err(StorageError::NotEnoughSpace);
        }

        let begin = blk_idx * self.block_size();
        let end = begin + self.block_size();

        data[..self.block_size()].copy_from_slice(&self.data[begin..end]);

        Ok(self.block_size())
    }

    fn write(&mut self, blk_idx: usize, data: &[u8]) -> Result<usize, StorageError> {
        self.validate_block_index(blk_idx)?;

        if data.len() != self.block_size() {
            return Err(StorageError::DataLenNotEqualToBlockSize);
        }

        let begin = blk_idx * self.block_size();
        let end = begin + self.block_size();
        self.data[begin..end].copy_from_slice(data);

        Ok(self.block_size())
    }

    fn block_size(&self) -> usize {
        B
    }

    fn min_block_index(&self) -> usize {
        0
    }

    fn max_block_index(&self) -> usize {
        S / B
    }
}

pub fn slices_are_equal<T: core::cmp::PartialEq>(a: &[T], b: &[T]) -> bool {
    a.len() == b.len() && a.starts_with(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ram_storage() {
        const SIZE: usize = 2048 + 256;
        const BLOCK: usize = 256;

        let mut ram_storage = RamStorage::<SIZE, BLOCK>::new().expect("Can't create ramstorage");
        let iter_count = SIZE / BLOCK * 3;

        assert!(iter_count < u8::MAX as usize);
        assert!(
            iter_count > ram_storage.max_block_index(),
            "Storage won't be tested with wraparound"
        );

        let mut actual = [0_u8; BLOCK];

        for i in ram_storage.max_block_index()..ram_storage.max_block_index() + 1 {
            assert!(
                ram_storage.read(i, &mut actual[..]).is_err(),
                "Must be failed, to high block index {}",
                i
            );
        }

        for i in 0..iter_count {
            let offset = i % (ram_storage.max_block_index() - ram_storage.min_block_index())
                + ram_storage.min_block_index();
            assert!(i < u8::MAX as usize);
            let val = (i + 1) as u8;

            assert!(
                ram_storage.read(offset, &mut actual[..]).is_ok(),
                "Can't read from ram storage, offset: {}",
                offset,
            );

            let expected = [val; BLOCK];
            assert!(
                !slices_are_equal(&expected[..], &actual[..]),
                "Can't perform test, slices are equal, offset: {}",
                offset,
            );

            assert!(
                ram_storage.write(offset, &expected[..]).is_ok(),
                "Can't write to ram storage, offset: {}",
                offset,
            );

            assert!(
                ram_storage.read(offset, &mut actual[..]).is_ok(),
                "Can't read from ram storage after write, offset: {}",
                offset,
            );

            assert!(
                slices_are_equal(&expected[..], &actual[..]),
                "Must be equal after write, offset: {}",
                offset,
            );
        }
    }
}
