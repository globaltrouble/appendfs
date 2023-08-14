use std::io::{self, Write};

use clap::Parser;

use appendfs::error::Error as FsError;
use appendfs::fs::Filesystem;
use appendfs::storage::file::FileStorage;

const DEFAULT_BLOCK_SIZE: u32 = 512;
const DEFAULT_BEGIN_BLOCK_IDX: u32 = 2048;
const DEFAULT_END_BLOCK_IDX: u32 = 1024 * 1024 * 1024 * 3 / DEFAULT_BLOCK_SIZE;

// TODO: make block size configurable
pub type Fs<'a> = Filesystem<'a, FileStorage, { DEFAULT_BLOCK_SIZE as usize }>;

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

    let retries = Some(4);
    let mut storage = match FileStorage::new(
        args.device,
        begin_block,
        end_block,
        args.block_size,
        retries,
    ) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Can't create storage: `{:?}`", e);
            return;
        }
    };

    let mut filesystem = match Fs::restore(&mut storage) {
        Ok(fs) => fs,
        Err(e) => {
            log::error!("Can't restore fs: `{:?}`", e);
            return;
        }
    };

    log::info!(
        "Init filesystem, offset: {:?}, next_id: {:?}",
        filesystem.offset(),
        filesystem.next_id(),
    );

    if filesystem.is_empty() {
        log::warn!("Nothing to read, fs is empty!");
        return;
    }

    let base_offset = filesystem.offset();
    let used = if filesystem.is_full() {
        (end_block - begin_block) as usize
    } else {
        filesystem.offset()
    };

    log::info!(
        "Reading from {} to {} (used={}), base is: {}",
        begin_block,
        end_block,
        used,
        base_offset
    );

    for offset in 0..used {
        let read = filesystem.read(offset as usize, |blk_data| {
            log::info!("Reading offste: {} ...", offset);
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
