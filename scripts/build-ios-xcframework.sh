#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

source "$HOME/.zshrc" 2>/dev/null || true
export PATH="${HOME}/.cargo/bin:${PATH}"

IOS_TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios)
for target in "${IOS_TARGETS[@]}"; do
  rustup target add "$target"
  cargo build -p metaprobe-ios-ffi --release --target "$target"
done

rm -rf build/ios "Metaprobe.xcframework"
mkdir -p build/ios/headers
cp crates/ios-ffi/include/metaprobe_ios.h build/ios/headers/
cp crates/ios-ffi/include/module.modulemap build/ios/headers/

lipo -create \
  target/aarch64-apple-ios-sim/release/libmetaprobe_ios_ffi.a \
  target/x86_64-apple-ios/release/libmetaprobe_ios_ffi.a \
  -output build/ios/libmetaprobe_ios_ffi_sim.a

xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libmetaprobe_ios_ffi.a \
  -headers build/ios/headers \
  -library build/ios/libmetaprobe_ios_ffi_sim.a \
  -headers build/ios/headers \
  -output "Metaprobe.xcframework"

echo "Created Metaprobe.xcframework"
