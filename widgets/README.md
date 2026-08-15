<!--
README.md - Publish-facing overview for the rlvgl-widgets crate.
-->

# rlvgl-widgets

Package: `rlvgl-widgets`

`rlvgl-widgets` contains the built-in widget implementations for the `rlvgl`
toolkit. It depends only on `rlvgl-core` and is meant to be usable in both
embedded and simulator builds.

The crate also publishes the canonical MPY descriptor catalog for the proof
actor set: `Container`, `Label`, `Button`, `Slider`, and `List`. Each descriptor
lives beside its native widget implementation, and `widgets::mpy::CATALOG`
derives the registry-facing catalog from those actor-local definitions.

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

The MPY catalog is always available and does not add a parallel widget model.
Its constructors erase native widgets through `rlvgl-core`'s `ActorOps` adapter
while preserving the same allocation behind the widget-tree handle.

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
