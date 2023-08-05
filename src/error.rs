#[derive(Clone, Debug)]
pub enum StorageError {
    TooSmallStorage,
    BlockOutOfRange,
    NotEnoughSpace,
    DataLenNotEqualToBlockSize,
}
