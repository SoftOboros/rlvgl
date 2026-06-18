//! Asset path, handle, registry, cache, and source implementations.
//!
//! This module is the LPAR-09 asset and filesystem source runtime. It layers
//! above the lower-half traits defined in [`crate::fs`] — [`crate::fs::AssetSource`],
//! [`crate::fs::AssetRead`], [`crate::fs::AssetError`] — and provides:
//!
//! - [`AssetPath`]: a typed source-kind + path enum (the `lv_fs_drv_t` drive-letter
//!   analog in typed form).
//! - [`AssetHandle`]: an opaque `u32` token returned by [`AssetRegistry::register`].
//!   The registry owns the interned `AssetPath`; the handle is `Copy` so any widget
//!   can hold one.
//! - [`AssetRegistry`]: multi-source dispatcher; holds up to
//!   [`ASSET_REGISTRY_MAX_SOURCES`] registered sources. `resolve_image` performs
//!   cache lookup, source open, decode dispatch, and cache insertion in one call.
//! - [`SlotCache`]: a const-generic bounded LRU cache keyed by [`crate::image::CacheHandle`].
//! - [`EmbeddedAssetSource`]: `no_std + alloc`; static slice lookup.
//! - [`MemoryAssetSource`]: `no_std + alloc`; runtime-populated heap table.
//! - [`SimAssetSource`]: `std`-only; host filesystem lookup (gated behind
//!   `cfg(not(target_os = "none"))` plus the `sim` feature).
//!
//! # `no_std` / `std` split
//!
//! Everything in this module except [`SimAssetSource`] compiles under
//! `no_std + alloc`. Decode dispatch to the PNG/JPEG/GIF plugins additionally
//! requires the `png`, `jpeg`, or `gif` feature (those plugins use `std::io::Cursor`
//! today; making them `no_std` is deferred-Safe per LPAR-09 §14).
//!
//! # Determinism
//!
//! [`SlotCache`] uses a monotonic `u32` LRU counter. Given a fixed sequence of
//! `get`/`insert` calls starting from a freshly constructed cache, the eviction
//! order is fully deterministic. Tests that need reproducible cache state should
//! use `SlotCache::new()` and drive calls in a fixed order.

extern crate alloc;

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::fs::{AssetError, AssetRead, AssetSource, FsError};
use crate::image::{CacheHandle, ImageDescriptor, PixelFormat};

// ── AssetPath ────────────────────────────────────────────────────────────────

/// A typed value naming an asset within a specific source kind.
///
/// This is the LPAR-09 analog of LVGL's drive-letter prefix (`A:path`, `S:path`).
/// Instead of a runtime string prefix, the enum variant encodes the source kind at
/// the Rust type level: there is no string to parse and no possibility of silently
/// routing a path to the wrong source.
///
/// # Source-kind set
///
/// The source-kind set is **frozen** (Standards Action per LPAR-09 §9).
/// Adding a new variant requires a §15 amendment to LPAR-09.
///
/// # Path string format
///
/// Within each source kind paths are `/`-separated UTF-8 with no required leading
/// `/`, consistent with [`AssetSource::open`]'s `"fonts/regular.bin"` convention.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetPath {
    /// A static asset linked into the binary via `include_bytes!`.
    ///
    /// The `&'static str` is the symbol name used to look up the byte slice in
    /// the [`EmbeddedAssetSource`] table. Lookup is infallible if the symbol
    /// exists; otherwise `AssetError::Fs(FsError::NoSuchFile)` is returned.
    Embedded(&'static str),
    /// An asset on a FAT volume reached through a `BlockDevice`.
    ///
    /// Available behind the `fatfs` feature in `no_std + alloc` builds.
    /// The `String` is the file path within the FAT volume (leading `/` stripped
    /// before passing to the FAT driver).
    Fatfs(String),
    /// An asset on the host filesystem (simulator builds only).
    ///
    /// Only available when the `sim` feature is enabled and the target is not
    /// `target_os = "none"`. The `String` is a relative or absolute host path.
    Sim(String),
    /// An asset in a runtime-populated in-RAM table.
    ///
    /// Useful for test asset injection and for content pre-loaded from FATFS or
    /// another source before the widget tree starts. `no_std + alloc`.
    Memory(String),
}

impl AssetPath {
    /// Return the path string within this source kind.
    ///
    /// For [`AssetPath::Embedded`] this is the symbol name. For all others it is
    /// the runtime path string.
    pub fn path_str(&self) -> &str {
        match self {
            AssetPath::Embedded(s) => s,
            AssetPath::Fatfs(s) | AssetPath::Sim(s) | AssetPath::Memory(s) => s.as_str(),
        }
    }

    /// Return a string label identifying the source kind (for diagnostics).
    pub fn source_kind(&self) -> &'static str {
        match self {
            AssetPath::Embedded(_) => "embedded",
            AssetPath::Fatfs(_) => "fatfs",
            AssetPath::Sim(_) => "sim",
            AssetPath::Memory(_) => "memory",
        }
    }
}

// ── AssetHandle ───────────────────────────────────────────────────────────────

/// Opaque token identifying a source-backed asset registered with an
/// [`AssetRegistry`].
///
/// The token is a `Copy` `u32`; the registry owns the interned [`AssetPath`].
/// Callers MUST call [`AssetRegistry::resolve_image`] to ensure decoded pixel
/// data is present before blitting. The handle remains valid for the lifetime
/// of the registry that issued it.
///
/// Handle value `0` is **reserved as the null/invalid handle**. Valid handles
/// start at `1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetHandle(u32);

impl AssetHandle {
    /// Return the raw `u32` token.
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

// ── SlotCache ─────────────────────────────────────────────────────────────────

/// Entry stored in a [`SlotCache`] slot.
struct CacheEntry {
    /// Cache key issued at insertion.
    handle: CacheHandle,
    /// Monotonic timestamp — higher is more recent.
    ts: u32,
    /// Decoded image descriptor (with `'static` pixel data).
    desc: ImageDescriptor<'static>,
}

/// Bounded LRU image cache backed by a fixed-size array.
///
/// # Const-generic parameter
///
/// `N` is the number of slots. Choose a value appropriate to the platform's
/// available RAM: on the STM32H747I-DISCO a value of 8–16 is typical.
///
/// # Eviction policy
///
/// On insertion when all `N` slots are occupied, the slot with the smallest
/// (oldest) `ts` value is evicted. On a cache hit, the matching slot's `ts`
/// is updated to the current counter value and the counter increments.
///
/// # Determinism
///
/// Given a fixed sequence of `get`/`insert` calls starting from a freshly
/// constructed `SlotCache`, the eviction order is fully deterministic.
/// Tests that require reproducible cache behaviour should create a fresh
/// `SlotCache::new()` and drive calls in a known order.
///
/// # Handle numbering
///
/// [`CacheHandle`] values are assigned sequentially starting at `1` (0 is
/// reserved). Within a session handles are never reused.
pub struct SlotCache<const N: usize> {
    slots: [Option<CacheEntry>; N],
    /// Monotonic LRU timestamp counter. Starts at 1; advances on every hit
    /// (`get`) and every insertion (`insert`).
    ts_counter: u32,
    /// Counter for issuing new [`CacheHandle`] values. Starts at 1.
    handle_counter: u32,
}

impl<const N: usize> SlotCache<N> {
    /// Create an empty cache with all slots vacant.
    pub const fn new() -> Self {
        // SAFETY: `None` is valid for `Option<CacheEntry>`. We use a manual
        // array-init trick because `CacheEntry` is not `Copy`.
        #[allow(clippy::declare_interior_mutable_const)]
        const NONE_ENTRY: Option<CacheEntry> = None;
        Self {
            slots: [NONE_ENTRY; N],
            ts_counter: 1,
            handle_counter: 1,
        }
    }

    /// Look up `handle` in the cache.
    ///
    /// On a hit the slot's timestamp is updated (LRU touch) and a reference to
    /// the cached [`ImageDescriptor`] is returned. On a miss `None` is returned.
    pub fn get(&mut self, handle: CacheHandle) -> Option<&ImageDescriptor<'static>> {
        let ts = self.ts_counter;
        self.ts_counter = self.ts_counter.wrapping_add(1);
        for entry in self.slots.iter_mut().flatten() {
            if entry.handle == handle {
                entry.ts = ts;
                // Re-borrow immutably to satisfy the borrow checker.
                break;
            }
        }
        // Second pass for immutable return.
        for entry in self.slots.iter().flatten() {
            if entry.handle == handle {
                return Some(&entry.desc);
            }
        }
        None
    }

    /// Insert `descriptor` into the cache and return its [`CacheHandle`].
    ///
    /// If all slots are occupied, the least-recently-used entry is evicted first.
    /// The returned handle is unique within the session.
    pub fn insert(&mut self, descriptor: ImageDescriptor<'static>) -> CacheHandle {
        let handle = CacheHandle::new(self.handle_counter);
        self.handle_counter = self.handle_counter.wrapping_add(1);
        let ts = self.ts_counter;
        self.ts_counter = self.ts_counter.wrapping_add(1);

        // Find a vacant slot first.
        for slot in &mut self.slots {
            if slot.is_none() {
                *slot = Some(CacheEntry {
                    handle,
                    ts,
                    desc: descriptor,
                });
                return handle;
            }
        }

        // All slots occupied — evict the LRU entry (smallest `ts`).
        let evict_idx = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|e| (i, e.ts)))
            .min_by_key(|&(_, t)| t)
            .map(|(i, _)| i)
            .expect("N >= 1 guaranteed by construction");
        self.slots[evict_idx] = Some(CacheEntry {
            handle,
            ts,
            desc: descriptor,
        });
        handle
    }

    /// Evict the entry identified by `handle` if it is present.
    ///
    /// After eviction the decoded pixels are dropped. The next
    /// `resolve_image` call for the same asset will re-decode from the source.
    pub fn evict(&mut self, handle: CacheHandle) {
        for slot in &mut self.slots {
            if slot.as_ref().is_some_and(|e| e.handle == handle) {
                *slot = None;
                return;
            }
        }
    }

    /// Return the number of occupied slots.
    pub fn len(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    /// Return `true` if no slots are occupied.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<const N: usize> Default for SlotCache<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> crate::image::ImageCache<'static> for SlotCache<N> {
    fn get(&self, _handle: CacheHandle) -> Option<&ImageDescriptor<'static>> {
        // The `ImageCache` trait takes `&self` but `SlotCache::get` needs
        // `&mut self` for the LRU timestamp update. Trait implementors that
        // need mutability should use `AssetRegistry` which holds the cache
        // behind interior mutability or mutable references.
        for entry in self.slots.iter().flatten() {
            if entry.handle == _handle {
                return Some(&entry.desc);
            }
        }
        None
    }

    fn put(&mut self, descriptor: ImageDescriptor<'static>) -> CacheHandle {
        self.insert(descriptor)
    }
}

// ── SourceKind tag ────────────────────────────────────────────────────────────

/// Internal tag used by [`AssetRegistry`] to match a registered [`AssetSource`]
/// to an [`AssetPath`] variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceKind {
    Embedded,
    Fatfs,
    Sim,
    Memory,
}

impl AssetPath {
    fn kind(&self) -> SourceKind {
        match self {
            AssetPath::Embedded(_) => SourceKind::Embedded,
            AssetPath::Fatfs(_) => SourceKind::Fatfs,
            AssetPath::Sim(_) => SourceKind::Sim,
            AssetPath::Memory(_) => SourceKind::Memory,
        }
    }
}

// ── AssetRegistry ─────────────────────────────────────────────────────────────

/// Maximum number of [`AssetSource`] implementations that can be registered
/// in a single [`AssetRegistry`].
///
/// The value is `4` — one per source kind. Increasing this requires an Expert
/// Review entry in LPAR-09 §9.
pub const ASSET_REGISTRY_MAX_SOURCES: usize = 4;

/// Registry entry pairing a source kind with its concrete [`AssetSource`]
/// implementation.
struct RegistrySource {
    kind: SourceKind,
    source: Box<dyn AssetSource>,
}

/// Inter-table record mapping an [`AssetHandle`] to its [`AssetPath`] and
/// the optional [`CacheHandle`] for any currently-cached decoded pixels.
struct HandleRecord {
    asset_handle: AssetHandle,
    path: AssetPath,
    cache_handle: Option<CacheHandle>,
}

/// Multi-source asset dispatcher.
///
/// The registry holds up to [`ASSET_REGISTRY_MAX_SOURCES`] registered
/// [`AssetSource`] implementations (one per source kind), an intern table
/// mapping [`AssetHandle`] tokens to their [`AssetPath`]s, and a
/// [`SlotCache`] for decoded [`ImageDescriptor`]s.
///
/// # Usage
///
/// ```ignore
/// let mut registry = AssetRegistry::<8>::new();
/// registry.register_source(AssetPath::Embedded(""),
///     Box::new(EmbeddedAssetSource::new(&ASSET_TABLE)))?;
/// let handle = registry.register(AssetPath::Embedded("icons/ok.raw"));
/// let desc = registry.resolve_image(handle)?;
/// ```
pub struct AssetRegistry<const CACHE_SLOTS: usize = 8> {
    sources: Vec<RegistrySource>,
    handles: Vec<HandleRecord>,
    cache: SlotCache<CACHE_SLOTS>,
    next_asset_handle: u32,
}

/// Error produced when a source registration call fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryError {
    /// The registry already holds [`ASSET_REGISTRY_MAX_SOURCES`] sources.
    Full,
    /// A source for this kind is already registered.
    DuplicateKind,
}

impl<const CACHE_SLOTS: usize> AssetRegistry<CACHE_SLOTS> {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            handles: Vec::new(),
            cache: SlotCache::new(),
            // Asset handle 0 is reserved as the null/invalid handle.
            next_asset_handle: 1,
        }
    }

    /// Register a concrete [`AssetSource`] implementation for the source kind
    /// indicated by `kind_sentinel`.
    ///
    /// `kind_sentinel` is any [`AssetPath`] value whose variant determines which
    /// source kind `source` will serve; the inner path string is ignored.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::Full`] if [`ASSET_REGISTRY_MAX_SOURCES`] sources
    /// are already registered. Returns [`RegistryError::DuplicateKind`] if a
    /// source for this kind is already present.
    pub fn register_source(
        &mut self,
        kind_sentinel: &AssetPath,
        source: Box<dyn AssetSource>,
    ) -> Result<(), RegistryError> {
        if self.sources.len() >= ASSET_REGISTRY_MAX_SOURCES {
            return Err(RegistryError::Full);
        }
        let kind = kind_sentinel.kind();
        if self.sources.iter().any(|s| s.kind == kind) {
            return Err(RegistryError::DuplicateKind);
        }
        self.sources.push(RegistrySource { kind, source });
        Ok(())
    }

    /// Intern an [`AssetPath`] and return an opaque [`AssetHandle`] token.
    ///
    /// If the same logical path is registered twice, two distinct handles are
    /// issued (intern-by-value is not enforced in v1; deduplication is deferred).
    pub fn register(&mut self, path: AssetPath) -> AssetHandle {
        let handle = AssetHandle(self.next_asset_handle);
        self.next_asset_handle = self.next_asset_handle.wrapping_add(1);
        self.handles.push(HandleRecord {
            asset_handle: handle,
            path,
            cache_handle: None,
        });
        handle
    }

    /// Resolve `handle` to a decoded [`ImageDescriptor`].
    ///
    /// The method checks the [`SlotCache`] first. On a hit the cached descriptor
    /// is returned. On a miss the source for the path's kind is asked to `open`
    /// the asset; the resulting bytes are drained and dispatched to the
    /// appropriate decode plugin by file extension / magic bytes; the decoded
    /// descriptor is inserted into the cache; and a reference to the cached
    /// descriptor is returned.
    ///
    /// Decode dispatch is feature-gated:
    ///
    /// | Extension / magic | Feature required | Plugin |
    /// |---|---|---|
    /// | `.raw` / `.rle` | none (no_std-safe) | bytes wrapped in `ImageData::Owned` |
    /// | `.png` / `\x89PNG` | `png` + non-`none` target | `crate::plugins::png::decode` |
    /// | `.jpg` / `.jpeg` / `\xff\xd8` | `jpeg` + non-`none` target | `crate::plugins::jpeg::decode` |
    /// | `.gif` / `GIF8` | `gif` | `crate::plugins::gif::decode` |
    /// | other | none | bytes wrapped in `ImageData::Owned` (raw fallback) |
    ///
    /// # Errors
    ///
    /// - [`AssetError::Fs`]`(`[`FsError::NoSuchFile`]`)` — handle not found in
    ///   registry, or no source registered for this path's kind.
    /// - [`AssetError::Fs`]`(`[`FsError::Device`]`)` — source returned an I/O
    ///   error during open/read.
    /// - [`AssetError::Decode`] — source bytes were read but codec returned an
    ///   error.
    pub fn resolve_image(
        &mut self,
        handle: AssetHandle,
    ) -> Result<&ImageDescriptor<'static>, AssetError> {
        // Look up the handle record.
        let record_idx = self
            .handles
            .iter()
            .position(|r| r.asset_handle == handle)
            .ok_or(AssetError::Fs(FsError::NoSuchFile))?;

        // Determine the effective cache handle to look up at the end of the
        // function.  We may need to insert a new entry (cache miss path) or
        // re-use an existing one (cache hit path).  Both paths converge to a
        // single immutable slot scan at the bottom of the function so that the
        // borrow checker sees only one immutable borrow of `self.cache.slots`.
        let final_cache_handle: CacheHandle;

        // Check whether we already have a valid cache entry for this asset.
        let existing_cache_handle: Option<CacheHandle> = self.handles[record_idx]
            .cache_handle
            .filter(|&ch| self.cache.slots.iter().flatten().any(|e| e.handle == ch));

        if let Some(ch) = existing_cache_handle {
            // Cache hit — update the LRU timestamp.
            let _ = self.cache.get(ch);
            final_cache_handle = ch;
        } else {
            // Cache entry was evicted (or never existed) — clear stale record.
            self.handles[record_idx].cache_handle = None;

            // Read bytes from the source.
            let path_str = self.handles[record_idx].path.path_str().to_owned();
            let kind = self.handles[record_idx].path.kind();

            let source_idx = self
                .sources
                .iter()
                .position(|s| s.kind == kind)
                .ok_or(AssetError::Fs(FsError::NoSuchFile))?;

            let bytes = {
                let source = &self.sources[source_idx].source;
                let mut reader = source.open(&path_str)?;
                let len = reader.len();
                let mut buf = alloc::vec![0u8; len];
                let mut off = 0usize;
                while off < len {
                    let n = reader.read(&mut buf[off..])?;
                    if n == 0 {
                        break;
                    }
                    off += n;
                }
                buf
            };

            // Decode by extension / magic.
            let desc = decode_bytes(&path_str, bytes)?;

            // Insert into the cache and record the handle.
            let new_ch = self.cache.insert(desc);
            self.handles[record_idx].cache_handle = Some(new_ch);
            final_cache_handle = new_ch;
        }

        // Single immutable borrow at the end: return a reference to the slot.
        for e in self.cache.slots.iter().flatten() {
            if e.handle == final_cache_handle {
                return Ok(&e.desc);
            }
        }
        // Unreachable: either the existing slot is present or insert placed it.
        Err(AssetError::Fs(FsError::Device))
    }
}

impl<const CACHE_SLOTS: usize> Default for AssetRegistry<CACHE_SLOTS> {
    fn default() -> Self {
        Self::new()
    }
}

// ── Decode dispatch ───────────────────────────────────────────────────────────

/// Detect the format of `bytes` by file `path` extension then magic, and
/// dispatch to the appropriate decode plugin.
///
/// Returns an [`ImageDescriptor`] with `'static` pixel data (owned bytes).
/// All codec calls are guarded behind the same feature flags that guard the
/// plugin modules themselves, preventing `std`-only codec calls in `no_std`
/// builds.
fn decode_bytes(path: &str, bytes: Vec<u8>) -> Result<ImageDescriptor<'static>, AssetError> {
    let lower = path.to_ascii_lowercase();

    // PNG: by extension or magic.
    #[cfg(all(feature = "png", not(target_os = "none")))]
    if lower.ends_with(".png") || bytes.get(..8).is_some_and(|h| h.starts_with(b"\x89PNG")) {
        return decode_png(bytes);
    }

    // JPEG: by extension or magic.
    #[cfg(all(feature = "jpeg", not(target_os = "none")))]
    if lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || bytes.get(..2).is_some_and(|h| h == b"\xff\xd8")
    {
        return decode_jpeg(bytes);
    }

    // GIF: by extension or magic.
    #[cfg(feature = "gif")]
    if lower.ends_with(".gif") || bytes.get(..4).is_some_and(|h| h == b"GIF8") {
        return decode_gif(bytes);
    }

    // Raw / RLE / unknown: return bytes as-is in Owned storage.
    // This is the no_std-safe fast path for `.raw`/`.rle` and for formats
    // whose codec feature is not enabled.
    let _ = lower; // suppress unused-variable warning when no codec features enabled
    Ok(ImageDescriptor {
        format: PixelFormat::Rgb565,
        width: 0,
        height: 0,
        data: crate::image::ImageData::Owned(bytes),
        stride: None,
    })
}

#[cfg(all(feature = "png", not(target_os = "none")))]
fn decode_png(bytes: Vec<u8>) -> Result<ImageDescriptor<'static>, AssetError> {
    let (colors, w, h) = crate::plugins::png::decode(&bytes)
        .map_err(|e| AssetError::Decode(alloc::format!("png: {e:?}")))?;
    // Convert Vec<Color> → Vec<u8> as packed ARGB8888.
    let mut pixels = Vec::with_capacity(colors.len() * 4);
    for c in &colors {
        pixels.push(c.0); // R
        pixels.push(c.1); // G
        pixels.push(c.2); // B
        pixels.push(c.3); // A
    }
    Ok(ImageDescriptor {
        format: PixelFormat::Argb8888,
        width: w as u16,
        height: h as u16,
        data: crate::image::ImageData::Owned(pixels),
        stride: None,
    })
}

#[cfg(all(feature = "jpeg", not(target_os = "none")))]
fn decode_jpeg(bytes: Vec<u8>) -> Result<ImageDescriptor<'static>, AssetError> {
    let (colors, w, h) = crate::plugins::jpeg::decode(&bytes)
        .map_err(|e| AssetError::Decode(alloc::format!("jpeg: {e:?}")))?;
    let mut pixels = Vec::with_capacity(colors.len() * 4);
    for c in &colors {
        pixels.push(c.0);
        pixels.push(c.1);
        pixels.push(c.2);
        pixels.push(c.3);
    }
    Ok(ImageDescriptor {
        format: PixelFormat::Argb8888,
        width: w,
        height: h,
        data: crate::image::ImageData::Owned(pixels),
        stride: None,
    })
}

#[cfg(feature = "gif")]
fn decode_gif(bytes: Vec<u8>) -> Result<ImageDescriptor<'static>, AssetError> {
    let (frames, w, h) = crate::plugins::gif::decode(&bytes)
        .map_err(|e| AssetError::Decode(alloc::format!("gif: {e:?}")))?;
    // Return the first frame's pixels; multi-frame animation is out of LPAR-09 scope.
    let frame = frames
        .into_iter()
        .next()
        .ok_or_else(|| AssetError::Decode(alloc::string::String::from("gif: no frames")))?;
    let mut pixels = Vec::with_capacity(frame.pixels.len() * 4);
    for c in &frame.pixels {
        pixels.push(c.0);
        pixels.push(c.1);
        pixels.push(c.2);
        pixels.push(c.3);
    }
    Ok(ImageDescriptor {
        format: PixelFormat::Argb8888,
        width: w,
        height: h,
        data: crate::image::ImageData::Owned(pixels),
        stride: None,
    })
}

// ── EmbeddedAssetSource ───────────────────────────────────────────────────────

/// Byte reader wrapping a `&'static [u8]` slice.
struct StaticSliceReader {
    data: &'static [u8],
    pos: usize,
}

impl AssetRead for StaticSliceReader {
    fn read(&mut self, out: &mut [u8]) -> Result<usize, AssetError> {
        let remaining = self.data.len().saturating_sub(self.pos);
        let n = out.len().min(remaining);
        out[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn seek(&mut self, pos: u64) -> Result<u64, AssetError> {
        self.pos = (pos as usize).min(self.data.len());
        Ok(self.pos as u64)
    }
}

/// Asset source backed by a static `(name, bytes)` table generated at build time.
///
/// The table is typically generated by a `build.rs` script following the pattern in
/// `examples/stm32h747i-disco/assets/disco-assets/build.rs`. Lookup is a linear
/// scan over the table for an exact string match on the name.
///
/// Infallible after construction: `AssetError::Fs(FsError::Device)` MUST NOT
/// occur. The only expected failure is [`FsError::NoSuchFile`] when the name is
/// absent from the table.
///
/// # `no_std + alloc`
///
/// Yes. The table and all byte slices are `'static`; no heap allocation occurs
/// on the `open` path.
pub struct EmbeddedAssetSource {
    table: &'static [(&'static str, &'static [u8])],
}

impl EmbeddedAssetSource {
    /// Create a source from a static `(name, bytes)` table.
    ///
    /// The table is typically a `static` generated by a build script; it is
    /// `&'static` so it can be referenced from multiple places without copying.
    pub const fn new(table: &'static [(&'static str, &'static [u8])]) -> Self {
        Self { table }
    }
}

impl AssetSource for EmbeddedAssetSource {
    fn open<'a>(&'a self, path: &str) -> Result<Box<dyn AssetRead + 'a>, AssetError> {
        for &(name, bytes) in self.table {
            if name == path {
                return Ok(Box::new(StaticSliceReader {
                    data: bytes,
                    pos: 0,
                }));
            }
        }
        Err(AssetError::Fs(FsError::NoSuchFile))
    }

    fn exists(&self, path: &str) -> bool {
        self.table.iter().any(|&(name, _)| name == path)
    }

    fn list(&self, _dir: &str) -> Result<crate::fs::AssetIter, AssetError> {
        Ok(crate::fs::AssetIter)
    }
}

// ── MemoryAssetSource ─────────────────────────────────────────────────────────

/// Byte reader wrapping an owned `Vec<u8>`.
struct VecReader {
    data: Vec<u8>,
    pos: usize,
}

impl AssetRead for VecReader {
    fn read(&mut self, out: &mut [u8]) -> Result<usize, AssetError> {
        let remaining = self.data.len().saturating_sub(self.pos);
        let n = out.len().min(remaining);
        out[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn seek(&mut self, pos: u64) -> Result<u64, AssetError> {
        self.pos = (pos as usize).min(self.data.len());
        Ok(self.pos as u64)
    }
}

/// Asset source backed by a runtime-populated in-RAM table.
///
/// Useful for test asset injection and for assets pre-loaded from FATFS or
/// another source before the widget tree starts. The table entries are owned
/// `String` names paired with owned `Vec<u8>` byte buffers.
///
/// # `no_std + alloc`
///
/// Yes. No static storage required.
pub struct MemoryAssetSource {
    entries: Vec<(String, Vec<u8>)>,
}

impl MemoryAssetSource {
    /// Create an empty memory source.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Insert a named asset with its byte contents.
    ///
    /// If an entry with the same name already exists it is replaced.
    pub fn insert(&mut self, name: impl Into<String>, data: Vec<u8>) {
        let name = name.into();
        for entry in &mut self.entries {
            if entry.0 == name {
                entry.1 = data;
                return;
            }
        }
        self.entries.push((name, data));
    }
}

impl Default for MemoryAssetSource {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetSource for MemoryAssetSource {
    fn open<'a>(&'a self, path: &str) -> Result<Box<dyn AssetRead + 'a>, AssetError> {
        for (name, data) in &self.entries {
            if name == path {
                return Ok(Box::new(VecReader {
                    data: data.clone(),
                    pos: 0,
                }));
            }
        }
        Err(AssetError::Fs(FsError::NoSuchFile))
    }

    fn exists(&self, path: &str) -> bool {
        self.entries.iter().any(|(name, _)| name == path)
    }

    fn list(&self, _dir: &str) -> Result<crate::fs::AssetIter, AssetError> {
        Ok(crate::fs::AssetIter)
    }
}

// ── SimAssetSource ────────────────────────────────────────────────────────────

/// Asset source backed by the host filesystem.
///
/// This source is **`std`-only** and MUST NOT be compiled into `target_os = "none"`
/// builds. It is gated behind `cfg(not(target_os = "none"))` in addition to the
/// `sim` or `std` feature requirement.
///
/// `SimAssetSource` is complementary to `fs-sim`'s `SimBlockDevice`: the latter
/// provides a `BlockDevice` over a FAT image file (for testing the FATFS path);
/// this source bypasses FAT entirely and reads host files directly.
#[cfg(all(feature = "sim", not(target_os = "none")))]
pub struct SimAssetSource {
    prefix: std::path::PathBuf,
}

#[cfg(all(feature = "sim", not(target_os = "none")))]
impl SimAssetSource {
    /// Create a source that resolves paths relative to `prefix`.
    ///
    /// Pass `"."` or an empty string for the current working directory.
    pub fn new(prefix: impl Into<std::path::PathBuf>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }
}

#[cfg(all(feature = "sim", not(target_os = "none")))]
struct StdFileReader {
    data: Vec<u8>,
    pos: usize,
}

#[cfg(all(feature = "sim", not(target_os = "none")))]
impl AssetRead for StdFileReader {
    fn read(&mut self, out: &mut [u8]) -> Result<usize, AssetError> {
        let remaining = self.data.len().saturating_sub(self.pos);
        let n = out.len().min(remaining);
        out[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn seek(&mut self, pos: u64) -> Result<u64, AssetError> {
        self.pos = (pos as usize).min(self.data.len());
        Ok(self.pos as u64)
    }
}

#[cfg(all(feature = "sim", not(target_os = "none")))]
impl AssetSource for SimAssetSource {
    fn open<'a>(&'a self, path: &str) -> Result<Box<dyn AssetRead + 'a>, AssetError> {
        use std::io::Read as _;
        let full = self.prefix.join(path);
        let mut file = std::fs::File::open(&full).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AssetError::Fs(FsError::NoSuchFile)
            } else {
                AssetError::Fs(FsError::Device)
            }
        })?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|_| AssetError::Fs(FsError::Device))?;
        Ok(Box::new(StdFileReader { data, pos: 0 }))
    }

    fn exists(&self, path: &str) -> bool {
        self.prefix.join(path).exists()
    }

    fn list(&self, _dir: &str) -> Result<crate::fs::AssetIter, AssetError> {
        Ok(crate::fs::AssetIter)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::{ImageData, PixelFormat};

    // ── EmbeddedAssetSource tests ─────────────────────────────────────────

    static PIXEL_2X1: &[u8] = &[0xFF, 0x00, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF]; // 2×1 ARGB8888
    static EMBED_TABLE: &[(&str, &[u8])] = &[("icons/red_green.raw", PIXEL_2X1)];

    #[test]
    fn embedded_source_open_hit() {
        let src = EmbeddedAssetSource::new(EMBED_TABLE);
        assert!(src.exists("icons/red_green.raw"));
        let mut reader = src.open("icons/red_green.raw").unwrap();
        assert_eq!(reader.len(), 8);
        let mut buf = [0u8; 8];
        let n = reader.read(&mut buf).unwrap();
        assert_eq!(n, 8);
        assert_eq!(&buf, PIXEL_2X1);
    }

    #[test]
    fn embedded_source_open_miss() {
        let src = EmbeddedAssetSource::new(EMBED_TABLE);
        assert!(!src.exists("missing.raw"));
        let result = src.open("missing.raw");
        assert!(matches!(result, Err(AssetError::Fs(FsError::NoSuchFile))));
    }

    // ── MemoryAssetSource tests ───────────────────────────────────────────

    #[test]
    fn memory_source_round_trip() {
        let mut src = MemoryAssetSource::new();
        src.insert("test.raw", vec![1, 2, 3, 4]);
        assert!(src.exists("test.raw"));
        let mut reader = src.open("test.raw").unwrap();
        let mut buf = [0u8; 4];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [1, 2, 3, 4]);
    }

    #[test]
    fn memory_source_replace() {
        let mut src = MemoryAssetSource::new();
        src.insert("a.raw", vec![1]);
        src.insert("a.raw", vec![2, 3]);
        let mut reader = src.open("a.raw").unwrap();
        assert_eq!(reader.len(), 2);
        let mut buf = [0u8; 2];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [2, 3]);
    }

    // ── SlotCache tests ───────────────────────────────────────────────────

    fn make_desc(marker: u8) -> ImageDescriptor<'static> {
        ImageDescriptor {
            format: PixelFormat::Rgb565,
            width: 1,
            height: 1,
            data: ImageData::Owned(alloc::vec![marker]),
            stride: None,
        }
    }

    #[test]
    fn slot_cache_basic_insert_and_get() {
        let mut cache: SlotCache<4> = SlotCache::new();
        let h = cache.insert(make_desc(42));
        let desc = cache.get(h).unwrap();
        assert_eq!(desc.data.as_bytes().unwrap(), &[42u8]);
    }

    #[test]
    fn slot_cache_evicts_lru() {
        let mut cache: SlotCache<2> = SlotCache::new();
        let h1 = cache.insert(make_desc(1)); // slot 0, ts=1
        let h2 = cache.insert(make_desc(2)); // slot 1, ts=3
        // Touch h1 to make it the MRU.
        let _ = cache.get(h1); // h1 ts=5, h2 ts=3 → h2 is LRU
        // Insert a third entry — should evict h2 (oldest ts).
        let h3 = cache.insert(make_desc(3));
        assert!(cache.get(h2).is_none(), "h2 should have been evicted");
        assert!(cache.get(h1).is_some(), "h1 should still be cached");
        assert!(cache.get(h3).is_some(), "h3 should be cached");
    }

    #[test]
    fn slot_cache_evict_explicit() {
        let mut cache: SlotCache<2> = SlotCache::new();
        let h = cache.insert(make_desc(7));
        assert!(cache.get(h).is_some());
        cache.evict(h);
        assert!(cache.get(h).is_none());
    }

    #[test]
    fn slot_cache_touch_updates_recency() {
        let mut cache: SlotCache<2> = SlotCache::new();
        let h1 = cache.insert(make_desc(1));
        let h2 = cache.insert(make_desc(2));
        // h1 was inserted first; without touching it, it would be LRU.
        // Touch h1 to promote it above h2.
        let _ = cache.get(h1);
        // Insert a new entry — h2 should be evicted (older ts).
        let h3 = cache.insert(make_desc(3));
        assert!(cache.get(h2).is_none(), "h2 evicted after h1 was touched");
        assert!(cache.get(h1).is_some());
        assert!(cache.get(h3).is_some());
    }

    // ── AssetRegistry tests ───────────────────────────────────────────────

    #[test]
    fn registry_register_and_resolve_embedded() {
        let mut reg: AssetRegistry<4> = AssetRegistry::new();
        reg.register_source(
            &AssetPath::Embedded(""),
            Box::new(EmbeddedAssetSource::new(EMBED_TABLE)),
        )
        .unwrap();
        let handle = reg.register(AssetPath::Embedded("icons/red_green.raw"));
        let desc = reg.resolve_image(handle).unwrap();
        // Raw path: bytes wrapped as-is with Rgb565 format and 0×0 dimensions.
        assert!(!desc.data.is_empty());
    }

    #[test]
    fn registry_resolve_unknown_handle_errors() {
        let mut reg: AssetRegistry<4> = AssetRegistry::new();
        let fake_handle = AssetHandle(99);
        let err = reg.resolve_image(fake_handle).unwrap_err();
        assert!(matches!(err, AssetError::Fs(FsError::NoSuchFile)));
    }

    #[test]
    fn registry_resolve_cache_hit_on_second_call() {
        let mut reg: AssetRegistry<4> = AssetRegistry::new();
        reg.register_source(
            &AssetPath::Embedded(""),
            Box::new(EmbeddedAssetSource::new(EMBED_TABLE)),
        )
        .unwrap();
        let handle = reg.register(AssetPath::Embedded("icons/red_green.raw"));
        // First call — cache miss, decodes from source.
        let _ = reg.resolve_image(handle).unwrap();
        // Second call — should return from cache.
        let desc = reg.resolve_image(handle).unwrap();
        assert!(!desc.data.is_empty());
    }

    #[test]
    fn registry_no_source_for_kind_errors() {
        let mut reg: AssetRegistry<4> = AssetRegistry::new();
        // Register a Memory source but ask for an Embedded path.
        reg.register_source(
            &AssetPath::Memory(String::new()),
            Box::new(MemoryAssetSource::new()),
        )
        .unwrap();
        let handle = reg.register(AssetPath::Embedded("foo.raw"));
        let err = reg.resolve_image(handle).unwrap_err();
        assert!(matches!(err, AssetError::Fs(FsError::NoSuchFile)));
    }

    #[test]
    fn registry_memory_source_round_trip() {
        let mut src = MemoryAssetSource::new();
        src.insert("logo.raw", vec![0xAA, 0xBB, 0xCC, 0xDD]);

        let mut reg: AssetRegistry<4> = AssetRegistry::new();
        reg.register_source(&AssetPath::Memory(String::new()), Box::new(src))
            .unwrap();
        let handle = reg.register(AssetPath::Memory("logo.raw".into()));
        let desc = reg.resolve_image(handle).unwrap();
        let bytes = desc.data.as_bytes().unwrap();
        assert_eq!(bytes, &[0xAA, 0xBB, 0xCC, 0xDD]);
    }

    #[test]
    fn registry_evict_then_reload() {
        let mut src = MemoryAssetSource::new();
        src.insert("img.raw", vec![0x01, 0x02]);

        let mut reg: AssetRegistry<1> = AssetRegistry::new(); // cache of 1 slot
        reg.register_source(&AssetPath::Memory(String::new()), Box::new(src))
            .unwrap();
        let h1 = reg.register(AssetPath::Memory("img.raw".into()));
        let h2 = reg.register(AssetPath::Memory("img.raw".into()));

        // Resolve h1 — populates the single cache slot.
        let _ = reg.resolve_image(h1).unwrap();
        // Resolve h2 — evicts h1's entry (cache size = 1).
        let _ = reg.resolve_image(h2).unwrap();
        // Resolve h1 again — should re-decode from source (cache miss + reload).
        let desc = reg.resolve_image(h1).unwrap();
        assert!(!desc.data.is_empty());
    }

    // ── ImageData::Asset variant tests ────────────────────────────────────

    #[test]
    fn image_data_asset_variant_constructs_and_matches() {
        let handle = AssetHandle(1);
        let data = ImageData::Asset(handle);
        // byte_len returns 0 for the Asset variant (not in-memory yet).
        assert_eq!(data.byte_len(), 0);
        assert!(data.is_empty());
        assert!(data.as_bytes().is_none());
        assert!(data.as_color_slice().is_none());

        match &data {
            ImageData::Asset(h) => assert_eq!(h.as_u32(), 1),
            _ => panic!("expected Asset variant"),
        }
    }

    #[test]
    fn existing_image_data_variants_unaffected() {
        // Verify that existing variants still match and behave as before.
        let borrowed_data = ImageData::Borrowed(&[1u8, 2, 3]);
        assert_eq!(borrowed_data.byte_len(), 3);
        assert!(borrowed_data.as_bytes().is_some());

        let owned_data: ImageData<'_> = ImageData::Owned(vec![4, 5]);
        assert_eq!(owned_data.byte_len(), 2);

        // A match that covers all known variants (the `#[non_exhaustive]`
        // attribute requires a wildcard arm only for external crates; within
        // `rlvgl-core` itself all variants are exhaustively known).
        let check = match owned_data {
            ImageData::Borrowed(_) => "borrowed",
            ImageData::BorrowedColors(_) => "colors",
            ImageData::Owned(_) => "owned",
            ImageData::Asset(_) => "asset",
        };
        assert_eq!(check, "owned");
    }

    // ── AssetPath helpers ─────────────────────────────────────────────────

    #[test]
    fn asset_path_helpers() {
        let p = AssetPath::Embedded("icons/ok.raw");
        assert_eq!(p.path_str(), "icons/ok.raw");
        assert_eq!(p.source_kind(), "embedded");

        let p2 = AssetPath::Memory("fonts/mono.bin".into());
        assert_eq!(p2.source_kind(), "memory");
        assert_eq!(p2.path_str(), "fonts/mono.bin");
    }

    // ── SimAssetSource test (std only) ────────────────────────────────────

    #[cfg(all(feature = "sim", not(target_os = "none")))]
    #[test]
    fn sim_source_reads_real_file() {
        use std::io::Write as _;
        let dir = std::env::temp_dir();
        let path = dir.join("rlvgl_sim_test.raw");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(&[0xDE, 0xAD]).unwrap();
        }
        let src = SimAssetSource::new(dir);
        let mut reader = src.open("rlvgl_sim_test.raw").unwrap();
        let mut buf = [0u8; 2];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [0xDE, 0xAD]);
        std::fs::remove_file(path).ok();
    }
}
