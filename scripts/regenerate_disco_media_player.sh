#!/usr/bin/env bash
# regenerate_disco_media_player.sh - Rebuild the Disco media-player source from QML.

set -euo pipefail

usage() {
    echo "usage: $0 --check|--write" >&2
    exit 2
}

[[ $# -eq 1 ]] || usage
mode="$1"
[[ "$mode" == "--check" || "$mode" == "--write" ]] || usage

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
qml="vendor/scjson/tutorial/Examples/Qt/SkodaBoleroInfotainment/Qml/Media/FrameMedia.qml"
overlay="$root/examples/apps/disco-demo/codegen/media_player_gen.patch"
target="$root/examples/apps/disco-demo/src/media_player_gen.rs"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/rlvgl-disco-media.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

mkdir -p "$tmp/emitted"
(
    cd "$root"
    RUSTFLAGS="" cargo run --quiet --features creator --bin rlvgl-creator -- \
        qt emit \
        "$qml" \
        "$tmp/emitted" \
        --target rlvgl \
        --scxml-context scxmlBolero=media_player
)

emitted="$tmp/emitted/FrameMedia.rlvgl.rs"
[[ -f "$emitted" ]] || {
    echo "expected generator output is missing: $emitted" >&2
    exit 1
}

cp "$emitted" "$tmp/media_player_gen.rs"
# The overlay is intentionally exact: any emitter-shape drift must stop here
# so the no-heap backend is reviewed against the new direct generator output.
patch -s -d "$tmp" -p1 -F 0 < "$overlay"

if [[ "$mode" == "--check" ]]; then
    if cmp -s "$tmp/media_player_gen.rs" "$target"; then
        echo "Disco media-player source is current."
        exit 0
    fi
    echo "Disco media-player source is stale; run $0 --write." >&2
    diff -u "$target" "$tmp/media_player_gen.rs" || true
    exit 1
fi

install -m 0644 "$tmp/media_player_gen.rs" "$target.tmp"
mv "$target.tmp" "$target"
echo "Regenerated $target"
