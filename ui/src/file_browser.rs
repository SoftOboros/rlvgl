// SPDX-License-Identifier: MIT
//! File browser widget for navigating storage devices and selecting files.
//!
//! Starts at the device level (QSPI flash, SD card) and lets users navigate
//! directories via double-tap. Non-`.wav` files are greyed out.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::icon_bitmap::{ICON_FILE, ICON_FOLDER, IconBitmap};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

// ── Visual constants ───────────────────────────────────────────────────────

/// Row height in display pixels (font height + padding).
const ROW_HEIGHT: i32 = 24;

/// Left margin before the icon.
const ICON_LEFT_MARGIN: i32 = 4;

/// Gap between the icon and the entry name text.
const ICON_TEXT_GAP: i32 = 6;

/// Target height for icons (matches `FONT_6X10` at 2x scale).
const ICON_TARGET_HEIGHT: i32 = 20;

/// Alpha value for greyed-out (non-interactive) entries.
const GREYED_OUT_ALPHA: u8 = 80;

// ── Data model ─────────────────────────────────────────────────────────────

/// Kind of entry in the file browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A storage device (QSPI flash, SD card) shown at root level.
    Device,
    /// A directory that can be navigated into.
    Directory,
    /// A `.wav` file that can be selected/opened.
    WavFile,
    /// A non-`.wav` file — visible but greyed out and non-interactive.
    OtherFile,
}

/// A single entry in the file browser listing.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Display name of the entry.
    pub name: String,
    /// Classification of the entry.
    pub kind: EntryKind,
}

impl FileEntry {
    /// Whether this entry responds to tap/double-tap.
    pub fn is_interactive(&self) -> bool {
        !matches!(self.kind, EntryKind::OtherFile)
    }

    /// Returns the appropriate icon bitmap for this entry kind.
    fn icon(&self) -> &'static IconBitmap {
        match self.kind {
            EntryKind::Device | EntryKind::Directory => &ICON_FOLDER,
            EntryKind::WavFile | EntryKind::OtherFile => &ICON_FILE,
        }
    }
}

// ── Storage trait ──────────────────────────────────────────────────────────

/// Errors returned while listing storage devices or directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageBrowserError {
    /// The requested device or directory could not be accessed.
    Unavailable,
}

/// Abstraction for listing storage device contents.
///
/// Platform code implements this trait using the appropriate storage drivers
/// (`embedded-sdmmc` for SD, `fatfs` for QSPI flash, etc.).
pub trait StorageBrowser {
    /// List root-level storage devices (e.g. "QSPI Flash", "SD Card").
    fn list_devices(&mut self) -> Vec<FileEntry>;

    /// List contents of a directory on the given device.
    ///
    /// `device_index` identifies which root device (from `list_devices`).
    /// `path` is `"/"` for the device root or a subdirectory path like
    /// `"/audio/drums"`.
    fn list_directory(
        &mut self,
        device_index: usize,
        path: &str,
    ) -> Result<Vec<FileEntry>, StorageBrowserError>;
}

// ── Navigation state ───────────────────────────────────────────────────────

/// Where the browser is currently looking.
#[derive(Debug, Clone)]
enum BrowseLevel {
    /// Root device list.
    DeviceList,
    /// Inside a specific device at a directory path.
    Directory { device_index: usize, path: String },
}

// ── Widget ─────────────────────────────────────────────────────────────────

/// File browser widget.
///
/// Displays a list of entries (devices, directories, files) with icons.
/// Single-tap selects, double-tap navigates or opens `.wav` files.
/// Non-`.wav` files are dimmed and non-interactive.
///
/// Navigation is deferred: the widget sets a `pending_nav` flag on
/// double-tap, and the application must call [`apply_navigation`](Self::apply_navigation)
/// with a [`StorageBrowser`] to populate the new listing.
type FileSelectedCallback = dyn FnMut(&str);

pub struct FileBrowser {
    bounds: Rect,
    /// Visual style (background, border).
    pub style: Style,
    /// Text color for interactive entries.
    pub text_color: Color,
    entries: Vec<FileEntry>,
    selected: Option<usize>,
    scroll_offset: i32,
    level: BrowseLevel,
    pending_nav: Option<BrowseLevel>,
    on_file_selected: Option<Box<FileSelectedCallback>>,
}

impl FileBrowser {
    /// Create a new file browser rooted at the device list.
    ///
    /// Call [`apply_navigation`](Self::apply_navigation) with a
    /// [`StorageBrowser`] to populate the initial device list.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            style: Style::default(),
            text_color: Color(255, 255, 255, 255),
            entries: Vec::new(),
            selected: None,
            scroll_offset: 0,
            level: BrowseLevel::DeviceList,
            pending_nav: Some(BrowseLevel::DeviceList),
            on_file_selected: None,
        }
    }

    /// Set the callback invoked when a `.wav` file is double-tapped.
    /// The argument is the full path to the selected file.
    pub fn on_file_selected(mut self, cb: impl FnMut(&str) + 'static) -> Self {
        self.on_file_selected = Some(Box::new(cb));
        self
    }

    /// Whether a navigation is pending. The application should call
    /// [`apply_navigation`](Self::apply_navigation) when this returns `true`.
    pub fn has_pending_navigation(&self) -> bool {
        self.pending_nav.is_some()
    }

    /// Apply the pending navigation using the given storage browser.
    /// Populates the entry list for the new location.
    pub fn apply_navigation(&mut self, storage: &mut dyn StorageBrowser) {
        let Some(nav) = self.pending_nav.take() else {
            return;
        };

        let entries = match &nav {
            BrowseLevel::DeviceList => storage.list_devices(),
            BrowseLevel::Directory { device_index, path } => storage
                .list_directory(*device_index, path)
                .unwrap_or_default(),
        };

        self.entries = entries;
        self.selected = None;
        self.scroll_offset = 0;
        self.level = nav;
    }

    /// Currently selected entry, if any.
    pub fn selected_entry(&self) -> Option<&FileEntry> {
        self.selected.and_then(|i| self.entries.get(i))
    }

    /// The current entries being displayed.
    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    /// Check if a point is within this widget's bounds.
    fn hit_test(&self, x: i32, y: i32) -> bool {
        x >= self.bounds.x
            && x < self.bounds.x + self.bounds.width
            && y >= self.bounds.y
            && y < self.bounds.y + self.bounds.height
    }

    /// Map a y coordinate to an entry index, accounting for scroll.
    fn index_at(&self, y: i32) -> Option<usize> {
        if y < self.bounds.y || y >= self.bounds.y + self.bounds.height {
            return None;
        }
        let local_y = y - self.bounds.y + self.scroll_offset;
        if local_y < 0 {
            return None;
        }
        let idx = (local_y / ROW_HEIGHT) as usize;
        if idx < self.entries.len() {
            Some(idx)
        } else {
            None
        }
    }

    /// Handle double-tap navigation: navigate into device/directory or
    /// select a `.wav` file.
    fn navigate(&mut self, idx: usize) {
        let entry = &self.entries[idx];
        match entry.kind {
            EntryKind::Device => {
                self.pending_nav = Some(BrowseLevel::Directory {
                    device_index: idx,
                    path: String::from("/"),
                });
            }
            EntryKind::Directory => {
                if let BrowseLevel::Directory {
                    device_index,
                    ref path,
                } = self.level
                {
                    let new_path = if entry.name == ".." {
                        // Navigate up
                        if path == "/" {
                            // Back to device list
                            self.pending_nav = Some(BrowseLevel::DeviceList);
                            return;
                        }
                        let trimmed = path.trim_end_matches('/');
                        match trimmed.rfind('/') {
                            Some(0) => String::from("/"),
                            Some(pos) => String::from(&trimmed[..pos]),
                            None => String::from("/"),
                        }
                    } else {
                        let base = path.trim_end_matches('/');
                        let mut p = String::from(base);
                        p.push('/');
                        p.push_str(&entry.name);
                        p
                    };
                    self.pending_nav = Some(BrowseLevel::Directory {
                        device_index,
                        path: new_path,
                    });
                }
            }
            EntryKind::WavFile => {
                if let (Some(cb), BrowseLevel::Directory { path, .. }) =
                    (self.on_file_selected.as_mut(), &self.level)
                {
                    let base = path.trim_end_matches('/');
                    let mut full = String::from(base);
                    full.push('/');
                    full.push_str(&entry.name);
                    cb(&full);
                }
            }
            EntryKind::OtherFile => {}
        }
    }
}

impl Widget for FileBrowser {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        draw_widget_bg(renderer, self.bounds, &self.style);

        let base_alpha = self.style.alpha;
        let icon_w = ICON_FOLDER.scaled_width(ICON_TARGET_HEIGHT);
        let text_x = self.bounds.x + ICON_LEFT_MARGIN + icon_w + ICON_TEXT_GAP;

        for (i, entry) in self.entries.iter().enumerate() {
            let row_y = self.bounds.y + i as i32 * ROW_HEIGHT - self.scroll_offset;

            // Skip rows outside visible area
            if row_y + ROW_HEIGHT <= self.bounds.y || row_y >= self.bounds.y + self.bounds.height {
                continue;
            }

            let alpha = if entry.is_interactive() {
                base_alpha
            } else {
                GREYED_OUT_ALPHA
            };

            // Selection highlight
            if self.selected == Some(i) {
                renderer.blend_rect(
                    Rect {
                        x: self.bounds.x,
                        y: row_y,
                        width: self.bounds.width,
                        height: ROW_HEIGHT,
                    },
                    Color(80, 120, 200, 80),
                );
            }

            // Icon
            let icon_y = row_y + (ROW_HEIGHT - ICON_TARGET_HEIGHT) / 2;
            entry.icon().draw(
                renderer,
                self.bounds.x + ICON_LEFT_MARGIN,
                icon_y,
                ICON_TARGET_HEIGHT,
                self.text_color.with_alpha(alpha),
            );

            // Text (baseline positioned near bottom of row)
            renderer.draw_text(
                (text_x, row_y + ROW_HEIGHT - 4),
                &entry.name,
                self.text_color.with_alpha(alpha),
            );
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::PressRelease { x, y } => {
                if !self.hit_test(*x, *y) {
                    return false;
                }
                if let Some(idx) = self.index_at(*y)
                    && self.entries[idx].is_interactive()
                {
                    self.selected = Some(idx);
                    return true;
                }
                false
            }
            Event::DoubleTap { x, y } => {
                if !self.hit_test(*x, *y) {
                    return false;
                }
                if let Some(idx) = self.index_at(*y)
                    && self.entries[idx].is_interactive()
                {
                    self.navigate(idx);
                    return true;
                }
                false
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<FileEntry> {
        alloc::vec![
            FileEntry {
                name: String::from(".."),
                kind: EntryKind::Directory
            },
            FileEntry {
                name: String::from("drums"),
                kind: EntryKind::Directory
            },
            FileEntry {
                name: String::from("kick.wav"),
                kind: EntryKind::WavFile
            },
            FileEntry {
                name: String::from("readme.txt"),
                kind: EntryKind::OtherFile
            },
            FileEntry {
                name: String::from("snare.wav"),
                kind: EntryKind::WavFile
            },
        ]
    }

    #[test]
    fn greyed_out_entries_not_selectable() {
        let mut fb = FileBrowser::new(Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 200,
        });
        fb.entries = sample_entries();

        // Tap on "readme.txt" (index 3, row_y = 3*24 = 72..96)
        let consumed = fb.handle_event(&Event::PressRelease { x: 50, y: 80 });
        assert!(!consumed);
        assert_eq!(fb.selected, None);
    }

    #[test]
    fn interactive_entries_selectable() {
        let mut fb = FileBrowser::new(Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 200,
        });
        fb.entries = sample_entries();

        // Tap on "kick.wav" (index 2, row_y = 2*24 = 48..72)
        let consumed = fb.handle_event(&Event::PressRelease { x: 50, y: 55 });
        assert!(consumed);
        assert_eq!(fb.selected, Some(2));
    }

    #[test]
    fn double_tap_directory_sets_pending_nav() {
        let mut fb = FileBrowser::new(Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 200,
        });
        fb.entries = sample_entries();
        fb.level = BrowseLevel::Directory {
            device_index: 0,
            path: String::from("/"),
        };

        // Double-tap on "drums" (index 1, row_y = 24..48)
        let consumed = fb.handle_event(&Event::DoubleTap { x: 50, y: 30 });
        assert!(consumed);
        assert!(fb.has_pending_navigation());
    }

    #[test]
    fn double_tap_other_file_ignored() {
        let mut fb = FileBrowser::new(Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 200,
        });
        fb.entries = sample_entries();
        fb.pending_nav = None; // clear initial pending nav

        // Double-tap on "readme.txt" (index 3)
        let consumed = fb.handle_event(&Event::DoubleTap { x: 50, y: 80 });
        assert!(!consumed);
        assert!(!fb.has_pending_navigation());
    }

    #[test]
    fn navigate_up_from_root_goes_to_device_list() {
        let mut fb = FileBrowser::new(Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 200,
        });
        fb.entries = sample_entries();
        fb.level = BrowseLevel::Directory {
            device_index: 0,
            path: String::from("/"),
        };

        // Double-tap on ".." (index 0, row_y = 0..24)
        fb.handle_event(&Event::DoubleTap { x: 50, y: 10 });
        assert!(fb.has_pending_navigation());
        assert!(matches!(fb.pending_nav, Some(BrowseLevel::DeviceList)));
    }

    #[test]
    fn is_interactive_classification() {
        assert!(
            FileEntry {
                name: String::from("sd"),
                kind: EntryKind::Device
            }
            .is_interactive()
        );
        assert!(
            FileEntry {
                name: String::from("d"),
                kind: EntryKind::Directory
            }
            .is_interactive()
        );
        assert!(
            FileEntry {
                name: String::from("a.wav"),
                kind: EntryKind::WavFile
            }
            .is_interactive()
        );
        assert!(
            !FileEntry {
                name: String::from("a.txt"),
                kind: EntryKind::OtherFile
            }
            .is_interactive()
        );
    }
}
