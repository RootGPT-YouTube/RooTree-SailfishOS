# Ambiente Rust per il cross di RooTree: va letto con «.» DENTRO il build engine
# (sfdk engine exec). Il motore è i686, quindi rustup è quello i686; i target
# sono aarch64 e armv7hl, linkati con i cross-gcc nativi del SDK (gli stessi che
# sb2 usa per «c++») contro il rootfs dei target clonati con Qt 6.12.
# Uso: . tools/env-rust.sh aarch64|armv7hl
# Rust sta in <home>/SailfishOS/rust (la home dell'host, montata anche nel
# motore): la home si ricava dal percorso del repo, /home/<utente>/...
_R=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
RUST_HOME=${ROOTREE_RUST_HOME:-$(echo "$_R" | cut -d/ -f1-3)/SailfishOS/rust}
export RUSTUP_HOME=$RUST_HOME/rustup
export CARGO_HOME=$RUST_HOME/cargo
export PATH=$CARGO_HOME/bin:$PATH
X=/srv/mer/toolings/SailfishOS-5.1.0.11/opt/cross/bin
case "${1:-aarch64}" in
  aarch64)
    export SFOS_TARGET=SailfishOS-5.1.0.11-aarch64-qt612
    export RUST_TARGET=aarch64-unknown-linux-gnu
    CROSS=$X/aarch64-meego-linux-gnu ;;
  armv7hl)
    export SFOS_TARGET=SailfishOS-5.1.0.11-armv7hl-qt612
    export RUST_TARGET=armv7-unknown-linux-gnueabihf
    CROSS=$X/armv7hl-meego-linux-gnueabi ;;
  *) echo "arch sconosciuta: $1" >&2; return 1 ;;
esac
export SFOS_SYSROOT=/srv/mer/targets/$SFOS_TARGET
T=$(echo "$RUST_TARGET" | tr a-z- A-Z_)
t=$(echo "$RUST_TARGET" | tr - _)
# il linker di rustc e il compilatore C/C++ dei crate con codice nativo (cc-rs)
export CARGO_TARGET_${T}_LINKER=$CROSS-gcc
# fuori da sb2 il cross-gcc non trova i suoi «as» e «ld» (sb2 li mappa, qui
# invece prende quelli i686 del motore: «as: unrecognized option '-EL'»):
# una cartella con i due nomi senza prefisso, passata con -B
B=$RUST_HOME/cross-bin/${1:-aarch64}
mkdir -p "$B"
for tool in as ld; do ln -sfn "$CROSS-$tool" "$B/$tool"; done
export CARGO_TARGET_${T}_RUSTFLAGS="-C link-arg=--sysroot=$SFOS_SYSROOT -C link-arg=-B$B"
export CC_$t="$CROSS-gcc"
export CXX_$t="$CROSS-g++"
export AR_$t="$CROSS-ar"
export CFLAGS_$t="--sysroot=$SFOS_SYSROOT -B$B"
export CXXFLAGS_$t="--sysroot=$SFOS_SYSROOT -B$B"
# pkg-config: SOLO per il target (variabili col suffisso del target, che il
# crate pkg-config legge per primo). ⛔ Con quelle generiche anche i test, che
# girano sull'host i686 del motore, cercavano OpenSSL nel sysroot aarch64
# («skipping incompatible …/libssl.so», Fase 5, 08/11)
export PKG_CONFIG_SYSROOT_DIR_$t=$SFOS_SYSROOT
export PKG_CONFIG_LIBDIR_$t=$SFOS_SYSROOT/usr/lib64/pkgconfig:$SFOS_SYSROOT/usr/lib/pkgconfig:$SFOS_SYSROOT/usr/share/pkgconfig
export PKG_CONFIG_ALLOW_CROSS=1
unset T t X B CROSS tool _R RUST_HOME
