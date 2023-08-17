#[derive(Clone, Debug)]
pub enum Error {
    TooSmallFilesystem,
    BlockOutOfRange,
    CanNotSeekForRead,
    CanNotSeekForWrite,
    NotEnoughSpaceForRead,
    DataLenNotEqualToBlockSize,
    InvalidBlockSizeForStorage,
    InvalidBlockSizeForRead,
    InvalidBlockSizeForWrite,
    TooSmallBuffer,
    CanNotPerformRead,
    CanNotPerformWrite,
    CanNotWriteConfig,
    NotValidBlockForRead,
    InvalidHeaderBlock,
}
