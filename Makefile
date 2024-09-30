.PHONY: all
all: debug

.PHONY: release
release:
	cargo build --release
	mkdir -p bin
	cp target/release/charon-rudra bin/charon-rudra
	cp target/release/libstd-* bin/ # For dynamic linking - also need to setup flag: LD_LIBRARY_PATH

.PHONY: debug
debug:
	cargo build
	mkdir -p bin
	cp target/debug/charon-rudra bin/charon-rudra
	cp target/debug/libstd-* bin/ # For dynamic linking - also need to setup flag: LD_LIBRARY_PATH
