<!--
COMPONENT_PLAN.md - File-browser-focused Chakra component mapping and rollout plan.
-->

# File Browser Component Plan

## Why This Exists

The current `rlvgl` demo browser is useful for embedded bring-up, but it is
intentionally narrow:

- one list panel at a time
- double-tap navigation
- storage access through a small abstract trait
- file handling tuned to `.wav`-centric workflows

That is not enough for a convincing file-open experience. The target here is a
portable component model that can deliver most of what users expect from a
desktop file dialog while still reducing cleanly to embedded constraints.

## Design Rule

Use `widget` for reusable, stateful controls that should survive the trip to
embedded targets.

Use `ui` for composed shells, overlays, workflows, and higher-order layouts
that orchestrate multiple widgets.

## High-Value Chakra Mapping

Focused on the subset needed to build a strong file browser first, not the
entire Chakra catalog.

| Chakra component | Layer | Why | File browser role | Embedded track |
| --- | --- | --- | --- | --- |
| `Button` / `IconButton` | `widget` | Core action primitive | open, cancel, up, sort, refresh | keep |
| `Input` | `widget` | Direct text entry is foundational | filename, search, filter text | keep |
| `Checkbox` | `widget` | Boolean filter/toggle primitive | embedded-only, include previews | keep |
| `Scroll Area` | `widget` | Viewport behavior, not app chrome | tree pane, file list, metadata panel | keep |
| `TreeView` | `widget` | Reusable selection + expansion control | devices, folders, project tree | keep |
| `Table` | `widget` | Dense structured selection surface | list/details file view | keep |
| `Tabs` | `widget` | Reusable stateful view switcher | preview, metadata, export tabs | keep |
| `Breadcrumb` | `widget` | Path navigation control | path bar and parent traversal | keep |
| `Menu` | `widget` | Reusable command surface | sort, view mode, bulk actions | keep later |
| `Pagination` | `widget` | Optional data windowing control | huge directories, paged media lists | later |
| `Splitter` | `widget` | Resizable pane control | tree/list/details widths | later |
| `Dialog` | `ui` | Multi-widget modal shell | file open / save / import shell | keep as UI |
| `Drawer` | `ui` | Secondary workspace surface | shortcuts, recents, batch queue | later |
| `Alert` | `ui` | State explanation, not primitive input | unsupported asset warnings | keep |
| `Badge` / `Tag` / `Status` | `ui` | Semantic annotation layer | ready/convert/unsupported states | keep |
| `Empty State` | `ui` | Composed fallback scene | empty directories, filtered-out results | keep |
| `Toast` | `ui` | Transient feedback shell | queued import/export actions | later |
| `Stat` / `Card` | `ui` | Summary presentation | preview metadata and telemetry | keep later |
| `Tooltip` / `Hover Card` | `ui` | Progressive disclosure | path truncation and constraints | later |

## Recommended `rlvgl` Extraction Order

### Phase 1: Browser-Critical Widgets

Implement or harden these first in `widget` form:

1. `TreeView`
2. `TableView` or `ListView` with selection model
3. `ScrollArea`
4. `TextInput`
5. `Breadcrumb`
6. `Tabs`

This is the minimum control set that moves the demo beyond a single-panel list.

### Phase 2: File Browser UI Shell

Build the higher-order file browser from those widgets:

1. modal file-open shell
2. command bar with back/forward/up and path search
3. tree + listing + details three-pane layout
4. status and compatibility annotations
5. preview/export affordances

This should live in `ui`, not `widgets`.

### Phase 3: Embedded Reduction Pass

Collapse the shell into smaller targets without changing the core model:

1. one-pane compact mode for 480x800 and smaller
2. no-hover interaction rules
3. keyboard/rotary/encoder focus paths
4. lazy directory loading and bounded row virtualization
5. preview plugins gated by memory budget and asset type

## What The New Next.js Frame Implements Now

- right-edge launcher strip inspired by `stm32h747i-disco`
- left-side wings for settings/info affordances
- a centered file-open dialog
- a portable mock filesystem with embedded-ready and unsupported assets
- tree navigation, breadcrumbs, sortable listing, details tabs, and open/cancel flow

## What To Build Next

1. Move the mock filesystem state and commands into a headless model package.
2. Add list/grid view switching plus pane resizing.
3. Add directory history, recents, favorites, and saved filters.
4. Add explicit conversion workflows for assets that are valid inputs but not
   embedded-ready outputs.
5. Port the widget subset back into `rlvgl-ui` or a new `rlvgl-file-browser`
   module once the interaction model settles.
