use crc;

type ID = u64;
type CRC = u16;

pub const CRC_ALGORITHM: crc::Crc<CRC> = crc::Crc::<CRC>::new(&crc::CRC_16_CDMA2000);

pub struct Block<'a> {
    pub data: &'a [u8],
    pub crc: CRC,
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

    pub fn id(&self) -> ID {
        const LEN: usize = core::mem::size_of::<ID>();
        let mut data = [0_u8; LEN];
        data[..].copy_from_slice(&self.data[..LEN]);

        ID::from_be_bytes(data)
    }

    pub fn is_valid(&self) -> bool {
        self.stored_crc() == self.crc
    }

    pub fn stored_crc(&self) -> CRC {
        const LEN: usize = core::mem::size_of::<CRC>();
        let mut data = [0_u8; LEN];
        data[..].copy_from_slice(&self.data[..LEN]);

        CRC::from_be_bytes(data)
    }

    pub fn calculated_crc(data: &[u8]) -> CRC {
        let len = data.len() - core::mem::size_of::<CRC>();
        CRC_ALGORITHM.checksum(&data[..len])
    }

    pub const fn attributes_size() -> usize {
        core::mem::size_of::<ID>() + core::mem::size_of::<CRC>()
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
        Self::from_block(&Block::new(data))
    }
}
