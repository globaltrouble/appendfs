# Appendfs

Fast embedded filesystem, main advantages:
* pure rust only, no_std by default, no allocations
* minumal memory footprint, require just BLOCK_SIZE bytes + some space for stack variables
* as fast as possible, can perform writes with minimum memory copy (just write to single buffer and it will be written to storage)
* auto rotation, new data will overwrite old one

Ideal for storing binary logs on embedded device, some internals:
* ring buffer under the hood as a data storage, new data will overwrite old one
* each block contains id and crc
* during the startup last block will be found with binary search, performs `log_2(STORAGE_SIZE / BLOCK_SIZE) + 2` reads to init filesystem.


### Test
cargo test --lib

### Build & run examples:
* build writer:
    ```
    cargo build --example writer --features=file_storage
    ```
* build reader:
    ```
    cargo build --example reader --features=file_storage
    ```

* run writer (flush all lines from stdin to fs):
    ```
    ./target/x86_64-unknown-linux-gnu/debug/examples/writer --device /dev/sda
    ```
* run reader (read all data from fs to stdout):
    ```
    ./target/x86_64-unknown-linux-gnu/debug/examples/reader --device /dev/sda
    ```

### TODO:
* add first superblock with fs attributes (version)
* add config support (single block, its offset can be tracked in each block)
* add persistent blocks (will be not overwritten, only with force flag) to store critical events
* test with example reader and example writer
* release embedded hal sd_card storage
