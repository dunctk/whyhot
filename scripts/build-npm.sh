#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mkdir -p "$root/npm/native"

cargo build --manifest-path "$root/Cargo.toml" --release --locked --target aarch64-apple-darwin
cargo build --manifest-path "$root/Cargo.toml" --release --locked --target x86_64-apple-darwin

cp "$root/target/aarch64-apple-darwin/release/whyhot" "$root/npm/native/whyhot-arm64"
cp "$root/target/x86_64-apple-darwin/release/whyhot" "$root/npm/native/whyhot-x64"
chmod 755 "$root/npm/native/whyhot-arm64" "$root/npm/native/whyhot-x64"
