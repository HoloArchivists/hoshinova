#!/bin/bash
set -e

#
# This script is used to generate release binaries for hoshinova. It uses
# cross-rs to cross-compile the project to multiple architectures.
#
# If you run into any issues with linking with glibc, run `cargo clean`.
# See: https://github.com/cross-rs/cross/issues/724
#

# Pre-build: generate typescript bindings
cargo test

# Build web UI
pushd web
  yarn install
  yarn build
popd

# Install cross
cargo install cross --git https://github.com/cross-rs/cross

targets=(x86_64-unknown-linux-musl aarch64-unknown-linux-musl x86_64-pc-windows-gnu)

for target in "${targets[@]}"; do
  echo "Building for $target"
  cross build --target $target --release

  ext=""
  if [[ $target == *"windows"* ]]; then
    ext=".exe"
  fi

  mv target/$target/release/hoshinova$ext target/hoshinova-$target$ext
done

echo "Done!"
