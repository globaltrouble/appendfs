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


