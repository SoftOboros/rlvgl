#!/bin/bash
PLUGIN_DIR="core/src/plugins"
OUT_FILE="$PLUGIN_DIR/mod.rs"

echo "//! Plugins for optional media formats and UI integrations." > "$OUT_FILE"
echo "//!" >> "$OUT_FILE"
echo "//! These modules are included conditionally via features such as 'gif', 'jpeg', 'qrcode', etc." >> "$OUT_FILE"
echo "// Auto-generated plugin exports" >> "$OUT_FILE"
echo "" >> "$OUT_FILE"

for f in "$PLUGIN_DIR"/*.rs; do
  name=$(basename "$f" .rs)
  [[ "$name" == "mod" ]] && continue

  echo "#[cfg(feature = \"$name\")]" >> "$OUT_FILE"
  echo "pub mod $name;" >> "$OUT_FILE"
  echo "#[cfg(feature = \"$name\")]" >> "$OUT_FILE"
  echo "pub use $name::*;" >> "$OUT_FILE"
  echo >> "$OUT_FILE"
done
