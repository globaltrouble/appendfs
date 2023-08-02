#![no_std]

use crc;

pub type CrcValue = u16;
pub const CRC_ALGORITHM: crc::Crc<CrcValue> = crc::Crc::<CrcValue>::new(&crc::CRC_16_CDMA2000);

// TODO: panic if block_size() * 2 > N

pub enum StorageError {}

pub trait Storage {
    fn read(&mut self, blk_idx: usize, data: &mut [u8]) -> Result<(), StorageError>;
    fn write(&mut self, blk_idx: usize, data: &[u8]) -> Result<(), StorageError>;

    // Make as member functions to make it configurable
    fn block_size(&self) -> usize;
    fn min_block_index(&self) -> usize;
    fn max_block_index(&self) -> usize;
}

pub struct Block<'a> {
    pub data: &'a [u8],
    crc: CrcValue,
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

    pub fn id(&self) -> u32 {
        const LEN: usize = core::mem::size_of::<u32>();
        let mut data = [0_u8; LEN];
        data[..].copy_from_slice(&self.data[..LEN]);

        u32::from_be_bytes(data)
    }

    pub fn is_valid(&self) -> bool {
        self.stored_crc() == self.crc
    }

    pub fn stored_crc(&self) -> u16 {
        const LEN: usize = core::mem::size_of::<u16>();
        let mut data = [0_u8; LEN];
        data[..].copy_from_slice(&self.data[..LEN]);

        u16::from_be_bytes(data)
    }

    pub fn calculated_crc(data: &[u8]) -> u16 {
        let len = data.len() - core::mem::size_of::<CrcValue>();
        CRC_ALGORITHM.checksum(&data[..len])
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

    pub fn write(&mut self, data: &[u8]) -> Result<usize, StorageError> {
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

        let (mut mid_buf, buf) = buf.split_at_mut(self.storage.block_size());
        let (mut right_buf, _) = buf.split_at_mut(self.storage.block_size());

        let mut begin = self.storage.min_block_index();
        let mut end = self.storage.max_block_index();

        {
            self.storage.read(begin, &mut mid_buf[..])?;
            let left_block = Block::new(&mid_buf[..]);
            if !left_block.is_valid() {
                // storage wasn't formatted, it is empty
                return Ok(begin);
            }
        }

        self.storage.read(end - 1, &mut right_buf[..])?;

        // at least 2 elements must be present
        while end - begin >= 2 {
            let mid = (begin + end) / 2;

            let tail_in_right = {
                self.storage.read(mid, &mut mid_buf[..])?;

                let mid_block = Block::new(&mid_buf[..]);
                let right_block = Block::new(&right_buf[..]);

                Self::can_have_tail(&mid_block, &right_block)
            };

            if tail_in_right {
                begin = mid;
            } else {
                end = mid + 1;
                core::mem::swap(&mut mid_buf, &mut right_buf);
            };
        }

        // begin will be last value before wraparound
        Ok(begin + 1)
    }

    fn can_have_tail(left: &Block, right: &Block) -> bool {
        if !left.is_valid() {
            return false;
        }

        if !right.is_valid() {
            return true;
        }

        left.id() > right.id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
