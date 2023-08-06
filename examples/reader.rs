use std::fs::File;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};

use clap::Parser;

use appendfs::error::Error;
use appendfs::fs::Filesystem;
use appendfs::storage::Storage;
use appendfs::utils::validate_block_index;

const DEFAULT_BLOCK_SIZE: u32 = 512;
const DEFAULT_BEGIN_BLOCK_IDX: u32 = 2048;
const DEFAULT_END_BLOCK_IDX: u32 = 1024 * 1024 * 1024 * 3 / 512;

// TODO: make block size configurable
pub type Fs = Filesystem<FileStorage, { DEFAULT_BLOCK_SIZE as usize }>;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    device: String,

    #[arg(long, default_value_t = DEFAULT_BEGIN_BLOCK_IDX)]
    begin_block: u32,

    #[arg(long, default_value_t = DEFAULT_END_BLOCK_IDX )]
    end_block: u32,

    #[arg(long, default_value_t = DEFAULT_BLOCK_SIZE )]
    block_size: u32,
}

pub struct FileStorage {
    begin_block: u32,
    end_block: u32,
    block_size: u32,
    file: File,
}

impl FileStorage {
    pub fn new(
        device: String,
        begin_block: u32,
        end_block: u32,
        block_size: u32,
    ) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .open(&device[..])
            .map_err(|e| e.to_string())?;

        Ok(FileStorage {
            begin_block,
            end_block,
            block_size,
            file,
        })
    }
}

impl Storage for FileStorage {
    fn read(&mut self, blk_idx: usize, data: &mut [u8]) -> Result<usize, Error> {
        validate_block_index(self, blk_idx)?;

        if data.len() < self.block_size() {
            return Err(Error::NotEnoughSpace);
        }

        let offset = blk_idx * self.block_size();
        self.file
            .seek(SeekFrom::Start(offset as u64))
            .map_err(|_e| Error::BlockOutOfRange)?;

        let data = &mut data[..self.block_size()];
        self.file
            .read_exact(data)
            .map_err(|_e| Error::CanNotPerformRead)?;

        Ok(self.block_size())
    }

    fn write(&mut self, blk_idx: usize, data: &[u8]) -> Result<usize, Error> {
        validate_block_index(self, blk_idx)?;
        if data.len() != self.block_size() {
            return Err(Error::DataLenNotEqualToBlockSize);
        }

        let offset = blk_idx * self.block_size();
        self.file
            .seek(SeekFrom::Start(offset as u64))
            .map_err(|_e| Error::BlockOutOfRange)?;
        self.file
            .write_all(data)
            .map_err(|_e| Error::CanNotPerformWrite)?;

        Ok(self.block_size())
    }

    fn block_size(&self) -> usize {
        self.block_size as usize
    }

    fn min_block_index(&self) -> usize {
        self.begin_block as usize
    }

    fn max_block_index(&self) -> usize {
        self.end_block as usize
    }
}

fn main() {
    env_logger::init();

    let args = Args::parse();
    log::info!("Reading from device: {}", &args.device);

    let storage = match FileStorage::new(
        args.device,
        args.begin_block,
        args.end_block,
        args.block_size,
    ) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Can't create storage: `{:?}`", e);
            return;
        }
    };

    let filesystem = match Fs::new(storage) {
        Ok(fs) => fs,
        Err(e) => {
            log::error!("Can't create fs: `{:?}`", e);
            return;
        }
    };

    log::info!(
        "Init filesystem, offset: {:?}, next_id: {:?}",
        filesystem.offset(),
        filesystem.next_id(),
    );
}
