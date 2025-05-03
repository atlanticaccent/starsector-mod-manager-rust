#!/bin/sh
# https://github.com/rust-lang/rust/issues/60059#issuecomment-1972748340

CC=${HOMEBREW_PREFIX}/opt/llvm@19/bin/clang \
CXX=${HOMEBREW_PREFIX}/opt/llvm@19/bin/clang++ \
AR=${HOMEBREW_PREFIX}/opt/llvm@19/bin/llvm-ar \
CFLAGS="-flto=full -O3" \
CXXFLAGS="-flto=full -O3" \
CARGO_INCREMENTAL=0 \
RUSTFLAGS="-Zshare-generics=true -Clinker-plugin-lto -Clinker=$PWD/scripts/linker-shim.sh -Clink-arg=-fuse-ld=${HOMEBREW_PREFIX}/Cellar/lld@19.1.5/19.1.5/bin/ld64.lld" \
cargo build --release