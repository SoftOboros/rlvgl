<!--
README.md - Publish-facing overview for the rlvgl-chips-microchip crate.
-->

# rlvgl-chips-microchip
Package: `rlvgl-chips-microchip`

`rlvgl-chips-microchip` is the Microchip board catalog crate used by
`rlvgl-creator` and related code-generation tooling.

## What It Provides

- `vendor()` returning the stable vendor key: `"microchip"`
- `boards()` for a lightweight list of known boards
- `find()` for exact-name board lookup
- `raw_db()` for the embedded raw board-definition blob produced at build time

## Build-Time Data Source

When building from a workspace checkout, set `RLVGL_CHIP_SRC` to a directory of
vendor board-definition files before compiling the crate:

```sh
RLVGL_CHIP_SRC=build/chipdb/microchip cargo build -p rlvgl-chips-microchip
```

If `RLVGL_CHIP_SRC` is not set, the crate still builds and its baked-in board
catalog remains available. The raw embedded database blob simply reflects
whatever the build script packaged during that build.

## Status

The public API is intentionally small and uniform across the vendor chip crates.
Its main job today is to feed board-selection and BSP-generation flows in
`rlvgl-creator`.

## Features

- `serde`: enable serialization support for the exposed data types

## License

MIT

## More Information

For more information, visit [softoboros.com](https://softoboros.com).

<p>
  <a href="https://softoboros.com">
    <img src="../../assets/branding/Softoboros-Letter-Logo.svg" alt="Softoboros" width="240" />
  </a>
</p>
