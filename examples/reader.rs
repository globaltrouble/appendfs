use std::io::{self, Write};

use clap::Parser;

use appendfs::error::Error as FsError;
use appendfs::fs::Filesystem;
use appendfs::storage::file::FileStorage;

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

fn main() {
    env_logger::init();

    let args = Args::parse();
    log::info!("Reading from device: {}", &args.device);

    let begin_block = args.begin_block;
    let end_block = args.end_block;

    let storage = match FileStorage::new(args.device, begin_block, end_block, args.block_size) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Can't create storage: `{:?}`", e);
            return;
        }
    };

    let mut filesystem = match Fs::new(storage) {
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

    let base_offset = filesystem.offset();
    let len = end_block - begin_block;
    for offset in 0..len {
        let read = filesystem.read(offset as usize, |blk_data| {
            log::info!(
                "Reading base_offset: {}, offste: {} ...",
                base_offset,
                offset
            );
            {
                let mut handle = io::stdout().lock();
                match handle.write_all(blk_data) {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!(
                            "Can't write block base_offset: {}, offste: {}, error: {:?}",
                            base_offset,
                            offset,
                            e
                        );
                    }
                };
            }
        });
        match read {
            Ok(_) => {}
            Err(FsError::NotValidBlock) => {
                log::info!("Finish reading at: {}", offset);
                break;
            }
            Err(e) => {
                log::error!(
                    "Error read block, base_offset: {}, offset: {}, e: {:?}",
                    base_offset,
                    offset,
                    e
                );
                break;
            }
        };
    }
}
