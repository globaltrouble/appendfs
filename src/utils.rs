use crate::error::Error;
use crate::storage::Storage;

pub fn slices_are_equal<T: core::cmp::PartialEq>(a: &[T], b: &[T]) -> bool {
    a.len() == b.len() && a.starts_with(b)
}

pub fn validate_block_index<S: Storage>(storage: &S, blk_idx: usize) -> Result<(), Error> {
    // TODO: move to helper
    if blk_idx < storage.min_block_index() || blk_idx >= storage.max_block_index() {
        return Err(Error::BlockOutOfRange);
    }

    Ok(())
}

pub fn trim_block_idx_with_wraparound(blk_idx: usize, min_blk: usize, max_blk: usize) -> usize {
    if blk_idx < min_blk {
        min_blk
    } else if blk_idx >= max_blk {
        blk_idx % max_blk + min_blk
    } else {
        blk_idx
    }
}
