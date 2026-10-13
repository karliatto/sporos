# Second-stage bootloader

`bootloader.bin` is the stock ESP-IDF bootloader for the ESP32, flashed at
`0x1000`. Nothing here builds it: it is the one espflash 4.5.0 (pinned by
`flake.lock`) bundles, extracted once from a merged image and checked into the
repo so that it cannot change under a toolchain bump.

- Size: 26,304 bytes
- SHA-256: `528faf022d3b07508a136bf17df34b99fc21bb543cdd829c3d2261c393de6d81`

Both `make flash` and `make image` use this file rather than espflash's
built-in copy, so what is flashed during development is what ships.

To turn on secure boot later, replace this file with an ESP-IDF bootloader
built with `CONFIG_SECURE_BOOT` and signed with the project key, and move the
offsets in `../partitions.csv` to make room for it.
