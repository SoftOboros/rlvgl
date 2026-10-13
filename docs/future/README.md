# Future / Design Sketches

Forward-looking design documents for features that are not yet implemented.
Anything here is intentionally aspirational — it describes where we would
like to go, not what ships today. When one of these lands, the document
moves out of `future/` into the appropriate topic subdir.

## Documents

- [MICROPYTHON-INTEGRATION.md](./MICROPYTHON-INTEGRATION.md) — Historical hardware-first sketch for MicroPython on CM7 + rlvgl on CM4; the active runtime plan is [MPY-00](../concepts/MPY-00-CONCEPTS.md).

## Promoted out of `future/`

- Qt/QML support — moved to [`docs/qt-support/`](../qt-support/) once
  the MVP `qt ingest` subcommand shipped (phase QT-01a). Future
  phases QT-02 onwards are still aspirational; status lives in the
  initiative README.
- Native Wayland backend — promoted to the Draft
  [`WLD` spec-before-code initiative](../wayland/README.md) for v0.2.7.
