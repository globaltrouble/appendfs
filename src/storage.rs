use crate::error::Error;
use crate::utils::validate_block_index;

pub trait Storage {
    fn read(&mut self, blk_idx: usize, data: &mut [u8]) -> Result<usize, Error>;
    fn write(&mut self, blk_idx: usize, data: &[u8]) -> Result<usize, Error>;

    // Make as member functions to make it configurable
    fn block_size(&self) -> usize;
    fn min_block_index(&self) -> usize;
    fn max_block_index(&self) -> usize;
}

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

#[cfg(test)]
mod tests {
    use super::{RamStorage, Storage};
    use crate::utils::slices_are_equal;

    #[test]
    fn test_ram_storage() {
        const BLOCK: usize = 256;
        const SIZE: usize = BLOCK * 9;

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
                "Can't perform test, slices are equal before read, offset: {}",
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
