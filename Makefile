## Usage
#
#   $ make build       # compile the firmware
#   $ make image       # compile, then write the release images to firmware/target/image/
#   $ make flash       # compile, flash, then attach the serial monitor
#   $ make monitor     # attach the serial monitor only
#   $ make check       # type-check without producing a binary
#   $ make test        # run the host tests in crates/core
#   $ make fmt-check   # check formatting in both workspaces
#   $ make clippy      # lint both workspaces, warnings as errors
#   $ make check-reproducible   # prove the build is byte-for-byte reproducible
#
# Requires the espup environment to be sourced first (except for `make test`,
# which builds nothing for the device):
#
#   $ source ~/export-esp.sh
#

## Variables
BAUDRATE ?= 115200
DEVICE ?= /dev/ttyACM0
PROFILE ?= release

# The firmware is its own workspace, so that .cargo/config.toml -- and with it
# the Xtensa target and build-std -- applies to the device build and not to the
# host tests. Cargo discovers that config from the working directory, not from
# the manifest, so every device target has to `cd` rather than pass
# --manifest-path.
FIRMWARE := firmware

ELF := $(FIRMWARE)/target/xtensa-esp32-none-elf/$(PROFILE)/sporos

# The firmware's own version, from the first `version =` in its manifest -- the
# [package] one, since the manifest has no other section that sets one.
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' $(FIRMWARE)/Cargo.toml | head -n 1)

# The bootloader and partition table every flash uses. Checked in rather than
# left to espflash's built-in defaults, so they are pinned and can be replaced by
# a signed bootloader for secure boot; see firmware/boot/README.md.
BOOTLOADER := $(FIRMWARE)/boot/bootloader.bin
PARTITIONS := $(FIRMWARE)/partitions.csv
FLASH_LAYOUT := --bootloader $(BOOTLOADER) --partition-table $(PARTITIONS)

# Release images, named for the version. Under target/ so git ignores them.
#
#   bootloader-v<version>.bin   at 0x1000
#   partitions-v<version>.bin   at 0x8000
#   sporos-v<version>.bin       at 0x10000, the app; the one that changes
#   sporos-v<version>-merged.bin  all three in one, at 0x0, for a blank board
IMAGE_DIR := $(FIRMWARE)/target/image
IMAGE := $(IMAGE_DIR)/sporos-v$(VERSION).bin

# Pins the build timestamp in the ESP-IDF app descriptor, which is part of the
# flashed image. Without this the descriptor records wall-clock time and no two
# builds ever match. Nix's stdenv already exports this exact value, so inside
# the dev shell it changes nothing; hard-assigning it means builds outside the
# shell land on the same timestamp too.
#
# esp-bootloader-esp-idf 0.5.0 reads the value as microseconds when it is really
# seconds, so the descriptor ends up reading 1970-01-01. Wrong, but fixed, which
# is all reproducibility asks of it.
export SOURCE_DATE_EPOCH := 315532800

.PHONY: build\
image\
check\
clean\
clippy\
flash\
fmt-check\
monitor\
test\
check-reproducible

build:
	cd $(FIRMWARE) && cargo build --$(PROFILE)

image: build
	mkdir -p $(IMAGE_DIR)
	cp $(BOOTLOADER) $(IMAGE_DIR)/bootloader-v$(VERSION).bin
	espflash partition-table --to-binary $(PARTITIONS) -o $(IMAGE_DIR)/partitions-v$(VERSION).bin
	espflash save-image --chip esp32 $(FLASH_LAYOUT) $(ELF) $(IMAGE)
	espflash save-image --chip esp32 $(FLASH_LAYOUT) --merge --skip-padding $(ELF) $(IMAGE_DIR)/sporos-v$(VERSION)-merged.bin
	@ls -l $(IMAGE_DIR)

check:
	cd $(FIRMWARE) && cargo check --$(PROFILE)

# Runs in the root workspace, which has no device target and no build-std, so
# this needs neither the espup environment nor a board.
test:
	cargo test

# Neither `cargo fmt` nor `cargo clippy` crosses a workspace boundary, so both
# have to be run twice or the firmware silently stops being checked. Wrapped in
# targets so CI and a local run cannot drift apart.
fmt-check:
	cargo fmt --check
	cd $(FIRMWARE) && cargo fmt --check

clippy:
	cargo clippy --all-targets -- -D warnings
	cd $(FIRMWARE) && cargo clippy --$(PROFILE) -- -D warnings

# `cargo run` invokes espflash via the runner configured in .cargo/config.toml.
flash:
	sudo chown ${USER}:${USER} $(DEVICE)
	cd $(FIRMWARE) && ESPFLASH_PORT=$(DEVICE) cargo run --$(PROFILE)

monitor:
	sudo chown ${USER}:${USER} $(DEVICE)
	espflash monitor --port $(DEVICE) --baud $(BAUDRATE)

clean:
	cargo clean
	cd $(FIRMWARE) && cargo clean

# Builds twice from scratch and compares the flashed image, not the ELF -- the
# .bin espflash produces is what actually reaches the device. Two clean builds,
# so expect a couple of minutes.
#
# This only proves determinism on this machine. The cross-machine guarantee also
# needs the pins flake.nix asserts; see the README.
check-reproducible:
	@set -e; \
	d=$$(mktemp -d); \
	trap 'rm -rf "$$d"' EXIT; \
	echo "==> build 1 of 2"; \
	(cd $(FIRMWARE) && cargo clean) >/dev/null; \
	$(MAKE) --no-print-directory build; \
	espflash save-image --chip esp32 $(FLASH_LAYOUT) $(ELF) "$$d/a.bin" >/dev/null; \
	echo "==> build 2 of 2"; \
	(cd $(FIRMWARE) && cargo clean) >/dev/null; \
	$(MAKE) --no-print-directory build; \
	espflash save-image --chip esp32 $(FLASH_LAYOUT) $(ELF) "$$d/b.bin" >/dev/null; \
	if cmp -s "$$d/a.bin" "$$d/b.bin"; then \
		echo "reproducible: $$(sha256sum <"$$d/a.bin" | cut -d' ' -f1)"; \
	else \
		echo "NOT reproducible - the two builds differ"; \
		exit 1; \
	fi
