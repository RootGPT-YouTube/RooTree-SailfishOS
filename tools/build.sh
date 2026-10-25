#!/bin/bash
# Compila RooTree per un'arch: prima il nucleo Rust (cargo nel motore, fuori da
# sb2: sotto sb2 i build script i686 verrebbero linkati col cross-gcc), poi
# l'app Qt con cmake sotto sb2 nel target …-qt612.
# Da lanciare DENTRO il build engine:  sfdk engine exec -- tools/build.sh aarch64|armv7hl
set -e
R=$(cd "$(dirname "$0")/.." && pwd)
ARCH=${1:-aarch64}
. "$R/tools/env-rust.sh" "$ARCH"
(cd "$R/rust" && cargo build --release --target "$RUST_TARGET")
RUST_OUT=$R/rust/target/$RUST_TARGET/release
B=$R/build-$ARCH
mkdir -p "$B"
sb2 -t "$SFOS_TARGET" cmake -S "$R/app" -B "$B" -DCMAKE_BUILD_TYPE=Release -DRUST_LIB_DIR="$RUST_OUT"
sb2 -t "$SFOS_TARGET" cmake --build "$B" -j12
sb2 -t "$SFOS_TARGET" strip "$B/harbour-rootree"
file "$B/harbour-rootree"
