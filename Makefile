.PHONY: all
all:
	cargo build --release
	mkdir -p bin
	cp target/release/charon-rudra bin/charon-rudra
	cp target/release/libstd-* bin/ # For dynamic linking - also need to setup flag: LD_LIBRARY_PATH
