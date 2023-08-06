use crate::error::Error;
use crate::storage::Storage;
use crate::utils::validate_block_index;

#[derive(Debug)]
pub struct RamStorage<const S: usize, const B: usize> {
    pub(crate) data: [u8; S],
}

impl<const S: usize, const B: usize> RamStorage<S, B> {
    pub fn new() -> Result<Self, Error> {
        if S < 2 * B {
            return Err(Error::TooSmallBuffer);
        }

        if S % B != 0 {
            return Err(Error::InvalidBlockSize);
        }

        Ok(Self { data: [0_u8; S] })
    }
}

impl<const S: usize, const B: usize> Storage for RamStorage<S, B> {
    fn read(&mut self, blk_idx: usize, data: &mut [u8]) -> Result<usize, Error> {
        validate_block_index(self, blk_idx)?;

        if data.len() < self.block_size() {
            return Err(Error::NotEnoughSpace);
        }

        let begin = blk_idx * self.block_size();
        let end = begin + self.block_size();

        data[..self.block_size()].copy_from_slice(&self.data[begin..end]);

        Ok(self.block_size())
    }

    fn write(&mut self, blk_idx: usize, data: &[u8]) -> Result<usize, Error> {
        validate_block_index(self, blk_idx)?;

        if data.len() != self.block_size() {
            return Err(Error::DataLenNotEqualToBlockSize);
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
