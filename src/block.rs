use crc;

type CRC = u16;
type ID = u64;

pub const CRC_ALGORITHM: crc::Crc<CRC> = crc::Crc::<CRC>::new(&crc::CRC_16_CDMA2000);

pub(crate) mod fields {
    use core::mem::size_of;

    pub(crate) const CRC_BEGIN: usize = 0;
    pub(crate) const CRC_LEN: usize = size_of::<super::CRC>();
    pub(crate) const CRC_END: usize = CRC_BEGIN + CRC_LEN;

    pub(crate) const ID_BEGIN: usize = CRC_END;
    pub(crate) const ID_LEN: usize = size_of::<super::ID>();
    pub(crate) const ID_END: usize = ID_BEGIN + ID_LEN;

    pub(crate) const DATA_BEGIN: usize = ID_END;
}

pub struct Block<'a> {
    pub data: &'a [u8],
    pub crc: CRC,
}

impl<'a> Block<'a> {
    pub fn from_buffer(buf: &'a [u8]) -> Self {
        let crc = Self::calculated_crc(buf);
        Self { data: buf, crc }
    }

    pub fn from_other(other: Block<'a>) -> Self {
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

    pub fn id(&self) -> ID {
        let mut data = [0_u8; fields::ID_LEN];
        data[..].copy_from_slice(&self.data[fields::ID_BEGIN..fields::ID_END]);

        ID::from_be_bytes(data)
    }

    pub(crate) fn set_id(buf: &mut [u8], id: ID) {
        let id = ID::to_be_bytes(id);
        buf[fields::ID_BEGIN..fields::ID_END].copy_from_slice(&id[..]);
    }

    pub fn calculated_crc(data: &[u8]) -> CRC {
        CRC_ALGORITHM.checksum(&data[fields::CRC_END..])
    }

    pub const fn attributes_size() -> usize {
        fields::DATA_BEGIN
    }
}

pub struct BlockFactory {
    id: ID,
}

impl BlockFactory {
    pub fn new(id: ID) -> BlockFactory {
        BlockFactory { id }
    }

    pub fn create_from_writer<'a, F, const B: usize>(
        &mut self,
        buf: &'a mut [u8],
        writer: F,
    ) -> Block<'a>
    where
        F: FnOnce(&mut [u8]),
    {
        writer(&mut buf[fields::DATA_BEGIN..]);
        Block::<'a>::set_id(buf, self.get_next_id());
        Block::<'a>::set_crc(buf);

        Block::<'a>::from_buffer(buf)
    }

    pub fn get_next_id(&mut self) -> ID {
        let id = self.id;
        self.id += 1;

        id
    }
}

pub struct BlockInfo {
    pub id: u64,
    pub is_valid: bool,
}

impl BlockInfo {
    pub fn from_block(block: &Block) -> BlockInfo {
        let is_valid = block.is_valid();
        let id = if is_valid { block.id() } else { 0 };

        Self { id, is_valid }
    }

    pub fn from_buffer(data: &[u8]) -> BlockInfo {
        Self::from_block(&Block::from_buffer(data))
    }
}
