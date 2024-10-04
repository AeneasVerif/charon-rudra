.PHONY: all
all:
	cargo build --release

.PHONY: install
install:
	cargo install --path .
