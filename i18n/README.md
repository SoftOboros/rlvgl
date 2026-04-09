<!--
README.md - Publish-facing overview for the rlvgl-i18n crate.
-->

# rlvgl-i18n
Package: `rlvgl-i18n`

`rlvgl-i18n` provides a small compile-time localization layer for `rlvgl`.
Locale JSON files are compiled at build time into a compact binary blob, and
the crate exposes generated locale/key enums plus lightweight lookup helpers.

## What It Provides

- build-time compilation of `locales/*.json` into the RLTN translation blob
- generated `Locale` and `Key` enums
- the `t!` macro for plain and parameterized lookups
- `set_locale()` and `locale()` for runtime locale selection
- `builtin_blob()` and `load_translations()` for swapping in an external blob

## Quick Start

```rust
use rlvgl_i18n::{Locale, set_locale, t};

set_locale(Locale::En);

let title = t!("demo.title", version = "0.1.9");
let clicks = t!("demo.clicks", count = 42);
```

## Runtime Override

The built-in translations are embedded into the binary, but the crate can also
switch to a translation blob loaded from media at runtime. This is intended for
deployments where translations live on removable storage or in a writable flash
partition.

`load_translations()` is `unsafe` because the override blob must match the
compiled locale/key tables.

## Notes

- `rlvgl-i18n` is `no_std` and uses `alloc`
- the build script generates the translation tables and the `t!` macro backing
  code during compilation
- the RLTN blob format is designed to stay small and cheap to query at runtime

## License

MIT

## More Information

For more information, visit [softoboros.com](https://softoboros.com).

<p>
  <a href="https://softoboros.com">
    <img src="../assets/branding/Softoboros-Letter-Logo.svg" alt="Softoboros" width="240" />
  </a>
</p>
