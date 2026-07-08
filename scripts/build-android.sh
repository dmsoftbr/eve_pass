#!/usr/bin/env bash
# Build evepass-core as Android .so libraries + Kotlin bindings (UniFFI).
# Requires: Android NDK (ANDROID_NDK_HOME or ~/Library/Android/sdk/ndk/<ver>),
# rustup, cargo-ndk (`cargo install cargo-ndk`).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/apps/mobile/native/android"
JNI="$OUT/jniLibs"
BUILD="$ROOT/target"
LIB=libevepass_core.a

# Locate the NDK if not exported.
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
  ANDROID_NDK_HOME="$(ls -d "$HOME/Library/Android/sdk/ndk/"* 2>/dev/null | sort -V | tail -1 || true)"
fi
[ -n "${ANDROID_NDK_HOME:-}" ] || { echo "✗ set ANDROID_NDK_HOME"; exit 1; }
export ANDROID_NDK_HOME
echo "▸ NDK: $ANDROID_NDK_HOME"

echo "▸ ensuring android rust targets + cargo-ndk"
rustup target add aarch64-linux-android x86_64-linux-android >/dev/null
command -v cargo-ndk >/dev/null || cargo install cargo-ndk

echo "▸ building .so (arm64-v8a, x86_64)"
mkdir -p "$JNI"
cargo ndk -t arm64-v8a -t x86_64 -o "$JNI" \
  build -p evepass-core --release

echo "▸ generating Kotlin bindings (needs the cdylib .so, not the .a)"
mkdir -p "$OUT/bindings"
cargo run -p evepass-core --bin uniffi-bindgen -- generate \
  --library "$BUILD/aarch64-linux-android/release/libevepass_core.so" \
  --language kotlin --out-dir "$OUT/bindings"

echo "✅ Android: .so in $JNI + Kotlin bindings in $OUT/bindings"
