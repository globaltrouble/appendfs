# Appendfs

Fast embedded filesystem, main advantages:
* pure rust only, no_std by default, no allocations
* minimal memory footprint, require just BLOCK_SIZE bytes + some space for stack variables
* as fast as possible, can perform writes with minimum memory copy (just write to single buffer and it will be written to storage)
* auto rotation, new data will overwrite old one

Ideal for storing binary logs on embedded device, some internals:
* ring buffer under the hood as a data storage, new data will overwrite old one
* each block contains id and crc
* during the startup last block will be found with binary search, performs `log_2(STORAGE_SIZE / BLOCK_SIZE) + 3` reads to init filesystem.


### Test
cargo test --lib

### Build & run examples.
To perform io on any attached storage (for example sdcard at /dev/sda) run reader/writer and specify `--device=/path/to/your/storage`, example:
    ```
    cargo run --example writer --features=file_storage,logging -- --device=/dev/sda --begin-block=2048 --end-block=262144
    ```
Examples used to be able to read/write data to AppendFs from laptop. Same actions can be performed with a file to be sure fs works.

* build writer:
    ```
    cargo build --example writer --features=file_storage,logging
    ```
* build reader:
    ```
    cargo build --example reader --features=file_storage,logging
    ```

* create 128MB file
    ```
    rm -rf temp/file-fs && mkdir -p temp && dd if=/dev/zero of=temp/file-fs bs=1024 count=131072
    ```

* format file to be able to use it as storage (optional step, writer will format it automaticaly in case it wasn't formatted)
    ```
    cargo run --example writer --features=file_storage,logging -- --device=temp/file-fs --begin-block=2048 --end-block=262144 --format-only
    ```

* run writer and send your data to its stdin, all data from stdin will be flushed to fs
    ```
    cargo run --example writer --features=file_storage,logging -- --device=temp/file-fs --begin-block=2048 --end-block=262144
    ```

* run writer and send your data to its stdin, (to write one more block, ensure write for different blocks)
    ```
    cargo run --example writer --features=file_storage,logging -- --device=temp/file-fs --begin-block=2048 --end-block=262144
    ```

* run reader and read all data you previously write to file
    ```
    cargo run --example reader --features=file_storage,logging -- --device=temp/file-fs --begin-block=2048 --end-block=262144
    ```

### TODO:
* test with example reader and example writer
* add decorator storage with io retries
* add decorator storage with redundancy coding
* add config support (single block, its offset can be tracked in each block)
* add persistent blocks (will be not overwritten, only with force flag) to store critical events
* add list of skipblocks (for unavailable blocks)
* release embedded hal sd_card storage
