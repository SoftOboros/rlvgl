//! LPAR-13 two-dimensional snap-to-tile navigation widget.
//!
//! A [`Tileview`] is a grid of fixed-size content panes (tiles). Each tile
//! occupies the full widget viewport and is registered at a `(col, row)` grid
//! position with allowed navigation directions. Navigation snaps directly to
//! `(col * viewport_width, row * viewport_height)` — exact positioning, not a
//! nearest-snap search — so there is no parallel snap algorithm (LPAR-13 §5.H).
//!
//! Only the active tile's content draws; inactive pane suppression follows the
//! same pattern as [`crate::tabview::Tabview`].
//!
//! # Key navigation
//!
//! Call the `navigate_*` helpers from an `ObjectEvent::Key` handler wired by
//! the application. No raw `Event::KeyDown` interception inside
//! `Widget::handle_event` (LPAR-12 pattern).

extern crate alloc;

use alloc::vec::Vec;

use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Sentinel value for [`TileId`] meaning "no tile".
const TILE_NONE_VALUE: u16 = u16::MAX;

// ---------------------------------------------------------------------------
// TileId
// ---------------------------------------------------------------------------

/// Opaque identifier for a tile within a [`Tileview`].
///
/// Assigned sequentially by [`Tileview::add_tile`].
/// The sentinel [`TILE_NONE`](Tileview::TILE_NONE) indicates an absent tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileId(pub u16);

// ---------------------------------------------------------------------------
// TileDir — bitflags
// ---------------------------------------------------------------------------

/// Allowed navigation directions from a tile.
///
/// Multiple directions can be combined with bitwise OR.
/// `TileDir::ALL` allows all four cardinal directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileDir(u8);

impl TileDir {
    /// No navigation allowed from this tile.
    pub const NONE: Self = Self(0);
    /// Navigation toward the tile above is allowed.
    pub const UP: Self = Self(1 << 0);
    /// Navigation toward the tile below is allowed.
    pub const DOWN: Self = Self(1 << 1);
    /// Navigation toward the tile to the left is allowed.
    pub const LEFT: Self = Self(1 << 2);
    /// Navigation toward the tile to the right is allowed.
    pub const RIGHT: Self = Self(1 << 3);
    /// All four directions are allowed.
    pub const ALL: Self = Self(0b0000_1111);

    /// Returns `true` if this set includes `other`.
    pub fn contains(self, other: TileDir) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl core::ops::BitOr for TileDir {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

// ---------------------------------------------------------------------------
// Internal tile descriptor
// ---------------------------------------------------------------------------

/// Internal descriptor for one registered tile.
#[derive(Clone)]
struct TileDesc {
    /// Horizontal grid coordinate (0-based).
    col: u8,
    /// Vertical grid coordinate (0-based).
    row: u8,
    /// Allowed navigation directions.
    dir: TileDir,
}

// ---------------------------------------------------------------------------
// Tileview
// ---------------------------------------------------------------------------

/// Two-dimensional snap-to-tile navigation grid.
///
/// Create with [`Tileview::new`], register tiles with [`Tileview::add_tile`],
/// and navigate with the direction helpers.
pub struct Tileview {
    /// Widget bounding box.
    bounds: Rect,
    /// Ordered list of registered tiles.
    tiles: Vec<TileDesc>,
    /// Index into `tiles` of the active tile, or `TILE_NONE_VALUE`.
    active_idx: u16,
    /// Next id counter.
    next_id: u16,
    /// Current horizontal content-space scroll offset (`col * bounds.width`).
    offset_x: i32,
    /// Current vertical content-space scroll offset (`row * bounds.height`).
    offset_y: i32,
    /// Overall background style (Part::MAIN).
    pub style: Style,
    /// Background color drawn for the active tile area.
    pub tile_color: Color,
}

impl Tileview {
    /// Sentinel [`TileId`] meaning "no tile selected / invalid".
    pub const TILE_NONE: TileId = TileId(TILE_NONE_VALUE);

    /// Create a tileview with no tiles.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            tiles: Vec::new(),
            active_idx: TILE_NONE_VALUE,
            next_id: 0,
            offset_x: 0,
            offset_y: 0,
            style: Style::default(),
            tile_color: Color(30, 30, 30, 255),
        }
    }

    /// Register a tile at `(col, row)` with the given allowed navigation directions.
    ///
    /// Returns the tile's [`TileId`]. If this is the first tile, it becomes
    /// the active tile automatically.
    pub fn add_tile(&mut self, col: u8, row: u8, dir: TileDir) -> TileId {
        let id = TileId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        let idx = self.tiles.len() as u16;
        self.tiles.push(TileDesc { col, row, dir });
        if self.active_idx == TILE_NONE_VALUE {
            self.active_idx = idx;
            self.update_offset_from_active();
        }
        id
    }

    /// Make the tile with the given id active.
    ///
    /// Snaps the view directly to the tile's grid position.
    /// No-op if the id is out of range or `TILE_NONE`.
    pub fn set_active(&mut self, id: TileId) {
        if (id.0 as usize) < self.tiles.len() {
            self.active_idx = id.0;
            self.update_offset_from_active();
        }
    }

    /// Return the [`TileId`] of the currently active tile, or
    /// [`TILE_NONE`](Self::TILE_NONE).
    pub fn active_tile(&self) -> TileId {
        if self.active_idx == TILE_NONE_VALUE {
            Self::TILE_NONE
        } else {
            TileId(self.active_idx)
        }
    }

    /// Activate the tile at grid position `(col, row)`.
    ///
    /// If no tile is registered at that position, this is a no-op.
    pub fn set_active_by_index(&mut self, col: u8, row: u8) {
        if let Some(idx) = self.find_tile(col, row) {
            self.active_idx = idx;
            self.update_offset_from_active();
        }
    }

    /// Return the content-space bounds of the tile with the given id.
    ///
    /// Bounds are always `(col * w, row * h, w, h)` where `w` and `h` are
    /// the widget viewport dimensions. Returns a zero-area rect for invalid ids.
    pub fn tile_bounds(&self, id: TileId) -> Rect {
        let idx = id.0 as usize;
        if idx >= self.tiles.len() {
            return Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            };
        }
        let t = &self.tiles[idx];
        Rect {
            x: t.col as i32 * self.bounds.width,
            y: t.row as i32 * self.bounds.height,
            width: self.bounds.width,
            height: self.bounds.height,
        }
    }

    /// Navigate to the tile above the current one, if allowed.
    ///
    /// Requires `TileDir::UP` to be set on the active tile and a tile registered
    /// at `(col, row - 1)`. Wire to `ObjectEvent::Key(Key::ArrowUp)`.
    pub fn navigate_up(&mut self) {
        self.navigate_direction(TileDir::UP, 0i8, -1i8);
    }

    /// Navigate to the tile below the current one, if allowed.
    ///
    /// Requires `TileDir::DOWN` on the active tile and a tile at `(col, row + 1)`.
    /// Wire to `ObjectEvent::Key(Key::ArrowDown)`.
    pub fn navigate_down(&mut self) {
        self.navigate_direction(TileDir::DOWN, 0i8, 1i8);
    }

    /// Navigate to the tile to the left of the current one, if allowed.
    ///
    /// Requires `TileDir::LEFT` on the active tile and a tile at `(col - 1, row)`.
    /// Wire to `ObjectEvent::Key(Key::ArrowLeft)`.
    pub fn navigate_left(&mut self) {
        self.navigate_direction(TileDir::LEFT, -1i8, 0i8);
    }

    /// Navigate to the tile to the right of the current one, if allowed.
    ///
    /// Requires `TileDir::RIGHT` on the active tile and a tile at `(col + 1, row)`.
    /// Wire to `ObjectEvent::Key(Key::ArrowRight)`.
    pub fn navigate_right(&mut self) {
        self.navigate_direction(TileDir::RIGHT, 1i8, 0i8);
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Common implementation for directional navigation.
    fn navigate_direction(&mut self, required_dir: TileDir, dc: i8, dr: i8) {
        if self.active_idx == TILE_NONE_VALUE {
            return;
        }
        let current = &self.tiles[self.active_idx as usize];
        if !current.dir.contains(required_dir) {
            return;
        }
        let new_col = (current.col as i16 + dc as i16).clamp(0, 255) as u8;
        let new_row = (current.row as i16 + dr as i16).clamp(0, 255) as u8;
        if let Some(idx) = self.find_tile(new_col, new_row) {
            self.active_idx = idx;
            self.update_offset_from_active();
        }
    }

    /// Find the index of the tile at `(col, row)`, or `None`.
    fn find_tile(&self, col: u8, row: u8) -> Option<u16> {
        self.tiles
            .iter()
            .position(|t| t.col == col && t.row == row)
            .map(|i| i as u16)
    }

    /// Set `offset_x`/`offset_y` to the exact tile position.
    fn update_offset_from_active(&mut self) {
        if self.active_idx == TILE_NONE_VALUE {
            return;
        }
        let t = &self.tiles[self.active_idx as usize];
        self.offset_x = t.col as i32 * self.bounds.width;
        self.offset_y = t.row as i32 * self.bounds.height;
    }
}

impl Widget for Tileview {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
        // Re-anchor offset so the active tile stays fully visible.
        self.update_offset_from_active();
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if self.bounds.width <= 0 || self.bounds.height <= 0 {
            return;
        }
        // Part::MAIN background.
        draw_widget_bg(renderer, self.bounds, &self.style);
        // Only the active tile's area is filled (content children are placed
        // by the caller inside the viewport; we just mark the active zone).
        if self.active_idx != TILE_NONE_VALUE {
            let color = self.tile_color;
            if color.3 > 0 {
                renderer.fill_rect(self.bounds, color);
            }
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    struct NullRenderer;
    impl rlvgl_core::renderer::Renderer for NullRenderer {
        fn fill_rect(&mut self, _r: Rect, _c: Color) {}
        fn draw_text(&mut self, _pos: (i32, i32), _t: &str, _c: Color) {}
    }

    #[test]
    fn new_is_empty_with_no_active_tile() {
        let tv = Tileview::new(rect(0, 0, 100, 80));
        assert_eq!(tv.active_tile(), Tileview::TILE_NONE);
    }

    #[test]
    fn first_add_becomes_active() {
        let mut tv = Tileview::new(rect(0, 0, 100, 80));
        let id = tv.add_tile(0, 0, TileDir::ALL);
        assert_eq!(tv.active_tile(), id);
    }

    #[test]
    fn tile_bounds_correct_for_grid_position() {
        let mut tv = Tileview::new(rect(0, 0, 100, 80));
        let _ = tv.add_tile(0, 0, TileDir::ALL);
        let id = tv.add_tile(2, 3, TileDir::ALL);
        let b = tv.tile_bounds(id);
        assert_eq!(b.x, 200); // 2 * 100
        assert_eq!(b.y, 240); // 3 * 80
        assert_eq!(b.width, 100);
        assert_eq!(b.height, 80);
    }

    #[test]
    fn navigate_right_moves_to_adjacent_tile() {
        let mut tv = Tileview::new(rect(0, 0, 100, 80));
        let a = tv.add_tile(0, 0, TileDir::RIGHT);
        let b = tv.add_tile(1, 0, TileDir::LEFT);
        tv.set_active(a);
        tv.navigate_right();
        assert_eq!(tv.active_tile(), b);
    }

    #[test]
    fn navigate_blocked_by_missing_direction_flag() {
        let mut tv = Tileview::new(rect(0, 0, 100, 80));
        let a = tv.add_tile(0, 0, TileDir::NONE); // no directions allowed
        let _b = tv.add_tile(1, 0, TileDir::LEFT);
        tv.set_active(a);
        tv.navigate_right(); // not allowed
        assert_eq!(tv.active_tile(), a); // unchanged
    }

    #[test]
    fn navigate_no_op_when_no_target_tile() {
        let mut tv = Tileview::new(rect(0, 0, 100, 80));
        let a = tv.add_tile(0, 0, TileDir::RIGHT); // allows right but no tile there
        tv.set_active(a);
        tv.navigate_right(); // no tile at (1, 0)
        assert_eq!(tv.active_tile(), a);
    }

    #[test]
    fn set_active_by_index_finds_tile() {
        let mut tv = Tileview::new(rect(0, 0, 100, 80));
        let a = tv.add_tile(0, 0, TileDir::ALL);
        let b = tv.add_tile(0, 1, TileDir::ALL);
        tv.set_active_by_index(0, 1);
        assert_eq!(tv.active_tile(), b);
        tv.set_active_by_index(0, 0);
        assert_eq!(tv.active_tile(), a);
    }

    #[test]
    fn offset_updates_to_exact_tile_position() {
        let mut tv = Tileview::new(rect(0, 0, 100, 80));
        tv.add_tile(0, 0, TileDir::RIGHT | TileDir::DOWN);
        tv.add_tile(1, 0, TileDir::LEFT);
        tv.add_tile(0, 1, TileDir::UP);
        tv.navigate_right();
        assert_eq!(tv.offset_x, 100);
        assert_eq!(tv.offset_y, 0);
        tv.navigate_left();
        assert_eq!(tv.offset_x, 0);
        tv.navigate_down();
        assert_eq!(tv.offset_y, 80);
    }

    #[test]
    fn tile_dir_contains_check() {
        let d = TileDir::UP | TileDir::DOWN;
        assert!(d.contains(TileDir::UP));
        assert!(d.contains(TileDir::DOWN));
        assert!(!d.contains(TileDir::LEFT));
    }

    #[test]
    fn draw_does_not_panic() {
        let mut tv = Tileview::new(rect(0, 0, 100, 80));
        tv.add_tile(0, 0, TileDir::ALL);
        let mut r = NullRenderer;
        tv.draw(&mut r);
    }
}
