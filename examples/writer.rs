use std::collections::VecDeque;
use std::io::{self, Read};

use clap::Parser;
use rand::Rng;

use appendfs::error::Error as FsError;
use appendfs::fs::Filesystem;
use appendfs::log;
use appendfs::storage::{file::FileStorage, Storage};

const DEFAULT_BLOCK_SIZE: u32 = 512;
const DEFAULT_BEGIN_BLOCK_IDX: u32 = 2048;
const DEFAULT_END_BLOCK_IDX: u32 = 1024 * 1024 * 1024 * 3 / 512;

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

    #[arg(short, long, default_value_t = false)]
    format_only: bool,
}

fn main() {
    env_logger::init();

    let args = Args::parse();
    log!(info, "Writing to file: {}", &args.device);

    let begin_block = args.begin_block;
    let end_block = args.end_block;

    let retries = Some(5);
    let mut storage = match FileStorage::new(
        args.device,
        begin_block,
        end_block,
        args.block_size,
        retries,
    ) {
        Ok(s) => s,
        Err(e) => {
            log!(error, "Can't create storage: `{:?}`", e);
            return;
        }
    };

    if args.format_only {
        let mut header_block = Vec::with_capacity(args.block_size as usize);
        header_block.resize(args.block_size as usize, 0);
        let res = storage.write(begin_block as usize, &mut header_block[..]);
        if res.is_err() {
            log!(error, "Can't format header, err: {:?}", res);
        }

        match Fs::new(&mut storage, rand::thread_rng().gen::<u32>()) {
            Ok(fs) => {
                log!(
                    info,
                    "Successfully formatted storage, offset: {}, id: {}, next_blk_id: {}",
                    fs.offset(),
                    fs.id(),
                    fs.next_blk_id()
                );
            }
            Err(e) => {
                log!(error, "Can't format storage, err: {:?}", e);
            }
        }
        return;
    }

    let mut filesystem = match Fs::restore(&mut storage) {
        Ok(fs) => fs,
        Err(FsError::InvalidHeaderBlock) => {
            log!(info, "Fs can't be restored, creating new one");
            match Fs::new(&mut storage, rand::thread_rng().gen::<u32>()) {
                Ok(fs) => fs,
                Err(e) => {
                    log!(error, "Can't create new fs, `{:?}`", e);
                    return;
                }
            }
        }
        Err(e) => {
            log!(error, "Can't restore fs: `{:?}`", e);
            return;
        }
    };

    log!(
        info,
        "Init filesystem, offset: {}, id: {}, next_blk_id: {}",
        filesystem.offset(),
        filesystem.id(),
        filesystem.next_blk_id()
    );

    let stdin = io::stdin();
    let mut buf: VecDeque<u8> = VecDeque::new();
    let mut i = 0;

    for byte in stdin.lock().bytes() {
        let byte = match byte {
            Ok(b) => b,
            Err(e) => {
                log::warn!("Can't read from stdin: {:?}", e);
                break;
            }
        };

        buf.push_back(byte);

        if buf.len() >= Fs::data_block_size() {
            i += 1;

            let written = filesystem.append(|blk_data| {
                let len = core::cmp::min(blk_data.len(), buf.len());
                for i in 0..len {
                    blk_data[i] = buf.pop_front().unwrap_or(0);
                }

                if len < blk_data.len() {
                    blk_data[len..].fill(0);
                }
            });

            match written {
                Ok(size) => {
                    log!(info, "Written block: {}, size: {}", i, size);
                }
                Err(e) => {
                    log!(info, "Error write block: {}, {:?}", i, e);
                }
            }
        }
    }

    if !buf.is_empty() {
        let written = filesystem.append(|blk_data| {
            let len = core::cmp::min(blk_data.len(), buf.len());
            for i in 0..len {
                blk_data[i] = buf.pop_front().unwrap_or(0);
            }

            if len < blk_data.len() {
                blk_data[len..].fill(0);
            }
        });

        match written {
            Ok(size) => {
                log!(info, "Written block: {}, size: {}", i, size);
            }
            Err(e) => {
                log!(info, "Error write block: {}, {:?}", i, e);
            }
        }
    }
}
