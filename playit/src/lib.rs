//! Mini-playwright test driver for rlvgl.
//!
//! `rlvgl-playit` provides input injection, widget queries by tag, and
//! framebuffer pixel inspection over a pluggable transport (serial, in-process,
//! etc.).
//!
//! # Architecture
//!
//! ```text
//! test host                       target / simulator
//! ──────────                      ──────────────────
//! "T100,200\n"  ──► transport ──► PlayitExecutor::poll()
//!                                   ├─ parse_command()
//!                                   ├─ EventSpec → Event
//!                                   ├─ WidgetNode::dispatch_event()
//!                                   └─► "OK\r\n" ──► transport ──► host
//! ```
//!
//! # Modules
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`command`] | Command, EventSpec, KeySpec, QuerySpec, DumpSpec, TouchPointSpec |
//! | [`response`] | Response, StatusData |
//! | [`protocol`] | Text wire codec (parse / format) |
//! | [`transport`] | `PlayitTransport` byte-level I/O trait |
//! | [`executor`] | `PlayitExecutor`, `EventPipeline` — main poll loop + gesture routing |
//! | [`recorder`] | `EventRecorder` — timed input capture for replay |
//! | [`tag`] | `find_by_tag` / `find_by_tag_mut` tree walkers |
//! | [`framebuffer`] | `FramebufferReader` pixel inspection trait |
//! | [`tcp`] | Loopback TCP transport for simulator / host automation (`std`) |
//!
//! # `no_std` support
//!
//! The crate is `no_std`-compatible by default with no allocator required.
//! Enable `alloc` for heap-backed helpers or `std` for standard-library
//! integrations.
#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![deny(missing_docs)]

pub mod command;
pub mod executor;
pub mod framebuffer;
pub mod protocol;
pub mod recorder;
pub mod response;
pub mod tag;
#[cfg(feature = "std")]
pub mod tcp;
pub mod transport;

pub use command::{
    Command, DumpSpec, EventSpec, KeySpec, QuerySpec, TouchPointSpec, TouchStateSpec,
};
pub use executor::{EventPipeline, PlayitExecutor};
pub use framebuffer::FramebufferReader;
pub use recorder::{EventRecorder, RecordEntry};
pub use response::{Response, StatusData};
pub use tag::{find_by_tag, find_by_tag_mut};
#[cfg(feature = "std")]
pub use tcp::TcpServerTransport;
pub use transport::PlayitTransport;
