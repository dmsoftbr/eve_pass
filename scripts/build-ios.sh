#!/usr/bin/env bash
# Build evepass-core as an iOS xcframework + Swift bindings (UniFFI).
# Requires: Xcode, rustup. Run from anywhere; paths are resolved from the repo.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CORE="$ROOT/core"
OUT="$ROOT/apps/mobile/native/ios"          # xcframework + Swift land here
BUILD="$ROOT/target"
LIB=libevepass_core.a

echo "▸ ensuring iOS rust targets"
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios >/dev/null

echo "▸ building static libs (release)"
cargo build -p evepass-core --release --target aarch64-apple-ios
cargo build -p evepass-core --release --target aarch64-apple-ios-sim
cargo build -p evepass-core --release --target x86_64-apple-ios

echo "▸ fat simulator lib (arm64 + x86_64)"
mkdir -p "$BUILD/ios-sim-universal"
lipo -create \
  "$BUILD/aarch64-apple-ios-sim/release/$LIB" \
  "$BUILD/x86_64-apple-ios/release/$LIB" \
  -output "$BUILD/ios-sim-universal/$LIB"

echo "▸ generating Swift bindings"
rm -rf "$OUT" && mkdir -p "$OUT/bindings" "$OUT/headers"
cargo run -p evepass-core --bin uniffi-bindgen -- generate \
  --library "$BUILD/aarch64-apple-ios/release/$LIB" \
  --language swift --out-dir "$OUT/bindings"

# UniFFI emits <name>.swift, <name>FFI.h, <name>FFI.modulemap.
cp "$OUT"/bindings/*FFI.h "$OUT/headers/" 2>/dev/null || true
# xcframework needs a module.modulemap named exactly that.
if ls "$OUT"/bindings/*FFI.modulemap >/dev/null 2>&1; then
  cp "$OUT"/bindings/*FFI.modulemap "$OUT/headers/module.modulemap"
fi

echo "▸ assembling xcframework"
rm -rf "$OUT/EvepassCore.xcframework"
xcodebuild -create-xcframework \
  -library "$BUILD/aarch64-apple-ios/release/$LIB" -headers "$OUT/headers" \
  -library "$BUILD/ios-sim-universal/$LIB" -headers "$OUT/headers" \
  -output "$OUT/EvepassCore.xcframework"

echo "✅ iOS: $OUT/EvepassCore.xcframework + Swift bindings in $OUT/bindings"
