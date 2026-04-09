<!--
README.md - Publish-facing overview for the rlvgl-bsps-stm crate.
-->

# rlvgl-bsps-stm
Package: `rlvgl-bsps-stm`

`rlvgl-bsps-stm` contains generated STM32 board-support modules used by the
`rlvgl-creator` BSP workflow. The crate packages Rust modules produced from
STM32CubeMX `.ioc` files so board-aware generation can target a published,
versioned crate instead of relying only on in-repo generated output.

## What It Provides

- generated STM32 board support modules under `src/`
- feature-gated family support across STM32 C0, F0/F1/F2/F3/F4/F7, G0/G4, H5/H7,
  L0/L1/L4/L5, WB, and WL lines
- a stable output target for `rlvgl-creator` and the workspace BSP scripts

## Regeneration Workflow

Regenerate the crate contents with:

```sh
scripts/gen_ioc_bsps.sh
```

That script runs the creator pipeline across the STM32CubeMX board set under
`chips/stm/STM32_open_pin_data/boards` and writes the generated modules into
this crate. MCU metadata comes from `rlvgl-chips-stm`.

## Notes

- the older board-overlay path is still present for compatibility, but the
  generated BSP path is the preferred direction
- some upstream boards are skipped when the required HAL family support or
  importer coverage is not ready yet

## License

BSD-3-Clause

## More Information

For more information, visit [softoboros.com](https://softoboros.com).

<p>
  <a href="https://softoboros.com">
    <img src="../../../assets/branding/Softoboros-Letter-Logo.svg" alt="Softoboros" width="240" />
  </a>
</p>
