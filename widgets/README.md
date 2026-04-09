<!--
README.md - Publish-facing overview for the rlvgl-widgets crate.
-->

# rlvgl-widgets

Package: `rlvgl-widgets`

`rlvgl-widgets` contains the built-in widget implementations for the `rlvgl`
toolkit. It depends only on `rlvgl-core` and is meant to be usable in both
embedded and simulator builds.

## Included Widgets

- `Button`
- `Checkbox`
- `Container`
- `Image`
- `Label`
- `List`
- `Progress`
- `Radio`
- `Slider`
- `Switch`

## Design

The crate keeps widget behavior and rendering logic separate from platform code.
Widgets are composed into a `WidgetNode` tree and then rendered by whichever
backend your application uses.

Use this crate directly when you want the toolkit's stock widgets without the
higher-level theming and layout layer from `rlvgl-ui`.

## Relationship To Other Crates

- `rlvgl-core` provides the runtime traits and shared types
- `rlvgl-widgets` provides concrete widget implementations
- `rlvgl-ui` builds more ergonomic components and helpers on top

## License

MIT

## More Information

For more information, visit [softoboros.com](https://softoboros.com).

<p>
  <a href="https://softoboros.com">
    <img src="../assets/branding/Softoboros-Letter-Logo.svg" alt="Softoboros" width="240" />
  </a>
</p>
