- Fix the linking issues:
  - Cargo.toml: prefer-dynamic dep
  - Makefile: cp libstd-*
  - `LD_LIBRARY_PATH`

Current comamnd:
`LD_LIBRARY_PATH=/home/soho/Projects/prosecco/charon-rudra/bin:/home/soho/.rustup/toolchains/nightly-2024-08-11-x86_64-unknown-linux-gnu/lib/ /home/soho/Projects/prosecco/charon-rudra/bin/charon-rudra --file insertion_sort.ullbc`

`charon --no-cargo --ullbc --no-merge-goto-chains --input insertion_sort.rs`
