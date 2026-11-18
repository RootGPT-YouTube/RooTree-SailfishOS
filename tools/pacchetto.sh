#!/bin/bash
# L'RPM di RooTree per un'arch, nel raccoglitore RPMS/: un pacchetto solo,
# col suo Qt 6.12 dentro (/home/rootree/qt6), come RooTitanium.
# Da lanciare DENTRO il build engine:  sfdk engine exec -- tools/pacchetto.sh aarch64|armv7hl
# Il Qt si riassembla solo se manca (scratch/qt6-runtime-<arch>) o con RUNTIME=1.
set -e
R=$(cd "$(dirname "$0")/.." && pwd)
ARCH=${1:-aarch64}
RT=SailfishOS-5.1.0.11-$ARCH.default      # rpmbuild nel target normale, come RooTitanium
case $ARCH in aarch64) RPMARCH=aarch64 ;; armv7hl) RPMARCH=armv7hl ;; esac
mkdir -p "$R/RPMS"

# ── Qt 6.12 dell'app ─────────────────────────────────────────────────────────
P=$R/packaging/qt6
QTS=$R/scratch/qt6-runtime-$ARCH/runtime
if [ "${RUNTIME:-0}" = 1 ] || [ ! -d "$QTS/lib" ]; then
    python3 "$P/assembla.py" --arch "$ARCH"
fi

# ── app ──────────────────────────────────────────────────────────────────────
"$R/tools/build.sh" "$ARCH"
S=$R/scratch/pkg-$ARCH
rm -rf "$S"
mkdir -p "$S/app/qml" "$S/icons"
cp -a "$QTS" "$S/qt6"
cp "$R/build-$ARCH/harbour-rootree" "$S/app/"
# l'helper della modalità root (dentro il pacchetto dell'app dalla 0.9.6), compilato da build.sh con il nucleo
. "$R/tools/env-rust.sh" "$ARCH" >/dev/null
cp "$R/rust/target/$RUST_TARGET/release/rootree-helper" "$S/rootree-helper"
sb2 -t "$SFOS_TARGET" strip "$S/rootree-helper"
cp -r "$R/app/qml/." "$S/app/qml/"
# qt.conf accanto al binario: SOSTITUISCE i percorsi Qt compilati (/usr) invece
# di aggiungersi come QT_PLUGIN_PATH; senza, chi ha Qt Runner si ritroverebbe
# i plugin Qt6 di sistema in /usr/lib64/qt6 (lezione di RooTitanium)
cat > "$S/app/qt.conf" <<'QTCONF'
[Paths]
Prefix = /home/rootree/qt6
Plugins = plugins
Imports = qml
Qml2Imports = qml
Libraries = lib
QTCONF
sb2 -t "$RT" gcc -O2 -Wall -DAPP_EXEC='"/home/rootree/app/harbour-rootree"' -DAPP_ID='"rootree"' \
    -DRUNTIME='"/home/rootree/qt6"' -o "$S/launcher" "$R/packaging/qt6/launcher.c"
sb2 -t "$RT" strip "$S/launcher"
cp "$R/rpm/harbour-rootree.desktop" "$R/LICENSE" "$R/NOTICE.md" "$S/"
for s in 86 108 128 172 256; do
    mkdir -p "$S/icons/${s}x${s}"
    cp "$R/grafica/${s}x${s}/harbour-rootree.png" "$S/icons/${s}x${s}/"
done
sb2 -t "$RT" rpmbuild -bb "$R/rpm/harbour-rootree.spec" \
    --define "_topdir $S/rpmbuild" --define "stagingdir $S" > "$S/rpmbuild.log" 2>&1 \
    || { tail -20 "$S/rpmbuild.log"; exit 1; }
cp "$S"/rpmbuild/RPMS/*/harbour-rootree-*.rpm "$R/RPMS/"
ls -la "$R/RPMS/"*"$RPMARCH.rpm"
