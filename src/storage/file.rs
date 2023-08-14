extern crate std;

use std::fs::File;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::string::{String, ToString};

use crate::error::Error;
use crate::storage::Storage;
use crate::utils::validate_block_index;

const DEFAULT_RETRIES: u16 = 4;

pub struct FileStorage {
    begin_block: u32,
    end_block: u32,
    block_size: u32,
    retries: u16,
    file: File,
}

impl FileStorage {
    pub fn new(
        device: String,
        begin_block: u32,
        end_block: u32,
        block_size: u32,
        retries: Option<u16>,
    ) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&device[..])
            .map_err(|e| e.to_string())?;

        Ok(FileStorage {
            begin_block,
            end_block,
            block_size,
            retries: retries.unwrap_or(DEFAULT_RETRIES),
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

        for i in 0..self.retries {
            let res = self.file.read_exact(data);
            if res.is_ok() {
                break;
            }

            if i + 1 == self.retries && res.is_err() {
                return Err(Error::CanNotPerformRead);
            }
        }

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

        for i in 0..self.retries {
            let res = self.file.write_all(data);
            if res.is_ok() {
                break;
            }

            if i + 1 == self.retries && res.is_err() {
                return Err(Error::CanNotPerformWrite);
            }
        }

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
