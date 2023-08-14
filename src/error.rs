#[derive(Clone, Debug)]
pub enum Error {
    TooSmallFilesystem,
    BlockOutOfRange,
    NotEnoughSpace,
    DataLenNotEqualToBlockSize,
    InvalidBlockSize,
    TooSmallBuffer,
    CanNotPerformRead,
    CanNotPerformWrite,
    NotValidBlock,
    InvalidHeaderBlock,
}
