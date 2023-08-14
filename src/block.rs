use crc;

pub type CRC = u16;
pub type FsId = u32;
pub type BlockId = u64;

pub const CRC_ALGORITHM: crc::Crc<CRC> = crc::Crc::<CRC>::new(&crc::CRC_16_CDMA2000);

pub(crate) mod fields {
    use core::mem::size_of;

    pub(crate) const CRC_BEGIN: usize = 0;
    pub(crate) const CRC_LEN: usize = size_of::<super::CRC>();
    pub(crate) const CRC_END: usize = CRC_BEGIN + CRC_LEN;

    pub(crate) const FS_ID_BEGIN: usize = CRC_END;
    pub(crate) const FS_ID_LEN: usize = size_of::<super::FsId>();
    pub(crate) const FS_ID_END: usize = FS_ID_BEGIN + FS_ID_LEN;

    pub(crate) const BLOCK_ID_BEGIN: usize = FS_ID_END;
    pub(crate) const BLOCK_ID_LEN: usize = size_of::<super::BlockId>();
    pub(crate) const BLOCK_ID_END: usize = BLOCK_ID_BEGIN + BLOCK_ID_LEN;

    pub(crate) const DATA_BEGIN: usize = BLOCK_ID_END;
}

#[derive(Debug)]
pub struct Block<'a, const S: usize> {
    pub data: &'a [u8],
    pub crc: CRC,
}

impl<'a, const S: usize> Block<'a, S> {
    pub fn from_buffer(buf: &'a [u8]) -> Self {
        let crc = Self::calculated_crc(buf);
        Self { data: buf, crc }
    }

    pub fn from_other(other: Block<'a, S>) -> Self {
        Self {
            data: other.data,
            crc: other.crc,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.stored_crc() == self.crc
    }

    pub fn stored_crc(&self) -> CRC {
        let mut data = [0_u8; fields::CRC_LEN];
        data[..].copy_from_slice(&self.data[fields::CRC_BEGIN..fields::CRC_END]);

        CRC::from_be_bytes(data)
    }

    pub(crate) fn set_crc(buf: &mut [u8]) {
        let crc = CRC::to_be_bytes(Self::calculated_crc(buf));
        buf[fields::CRC_BEGIN..fields::CRC_END].copy_from_slice(&crc[..]);
    }

    pub fn id(&self) -> BlockId {
        let mut data = [0_u8; fields::BLOCK_ID_LEN];
        data[..].copy_from_slice(&self.data[fields::BLOCK_ID_BEGIN..fields::BLOCK_ID_END]);

        BlockId::from_be_bytes(data)
    }

    pub(crate) fn set_id(buf: &mut [u8], id: BlockId) {
        let id = BlockId::to_be_bytes(id);
        buf[fields::BLOCK_ID_BEGIN..fields::BLOCK_ID_END].copy_from_slice(&id[..]);
    }

    pub(crate) fn fs_id(&self) -> FsId {
        let mut data = [0_u8; fields::FS_ID_LEN];
        data[..].copy_from_slice(&self.data[fields::FS_ID_BEGIN..fields::FS_ID_END]);

        FsId::from_be_bytes(data)
    }

    pub(crate) fn set_fs_id(buf: &mut [u8], id: FsId) {
        let id: [u8; 4] = FsId::to_be_bytes(id);
        buf[fields::FS_ID_BEGIN..fields::FS_ID_END].copy_from_slice(&id[..]);
    }

    pub fn calculated_crc(data: &[u8]) -> CRC {
        CRC_ALGORITHM.checksum(&data[fields::CRC_END..])
    }

    pub const fn attributes_size() -> usize {
        fields::DATA_BEGIN
    }
}

#[derive(Debug)]
pub struct BlockFactory {
    pub id: BlockId,
}

impl BlockFactory {
    pub fn new() -> BlockFactory {
        BlockFactory { id: 0 }
    }

    pub(crate) fn set_id(&mut self, id: BlockId) {
        self.id = id;
    }

    pub fn create_with_writer<'a, F, const S: usize>(
        &mut self,
        buf: &'a mut [u8],
        fs_id: FsId,
        writer: F,
    ) -> Block<'a, S>
    where
        F: FnOnce(&mut [u8]),
    {
        writer(&mut buf[fields::DATA_BEGIN..]);
        Block::<'a, S>::set_id(buf, self.get_next_id());
        Block::<'a, S>::set_fs_id(buf, fs_id);
        Block::<'a, S>::set_crc(buf);

        Block::<'a, S>::from_buffer(buf)
    }

    pub fn get_next_id(&mut self) -> BlockId {
        let id = self.id;
        self.id += 1;

        id
    }
}

impl Default for BlockFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct BlockInfo<const S: usize> {
    pub id: u64,
    pub fs_id: u32,
    pub is_valid: bool,
}

impl<const BS: usize> BlockInfo<BS> {
    pub fn from_block(block: &Block<BS>) -> Self {
        let is_valid = block.is_valid();
        let fs_id = block.fs_id();
        let id = if is_valid { block.id() } else { 0 };

        Self {
            id,
            fs_id,
            is_valid,
        }
    }

    pub fn from_buffer(data: &[u8]) -> Self {
        Self::from_block(&Block::<BS>::from_buffer(data))
    }
}
