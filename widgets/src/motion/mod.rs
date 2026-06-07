// SPDX-License-Identifier: MIT
//! UI motion components and the traits that parameterise them.
//!
//! This module is the home of `rlvgl`'s scrolling motion widgets. The
//! first inhabitant is the Star Wars–style text crawl
//! ([`crawl::StarCrawl`]), but the infrastructure is designed to
//! accommodate other stress-test components such as news tickers,
//! parallax scrollers, audio scopes, and so on.
//!
//! The motion infrastructure is split along four axes:
//!
//! - [`Direction`] selects one of four cardinal scroll directions.
//! - [`MotionRate`] / [`FrameRoundedRate`] controls how the scroll
//!   accumulator advances per frame. Sub-pixel rates slot in as
//!   additional trait impls without touching any engine code.
//! - [`JumboBuffer`] wraps an externally-supplied slice that is 2× or
//!   more the visible extent along the scroll axis, so the scroll
//!   cursor can slide cheaply and the background only needs to be
//!   painted once.
//! - [`BackgroundPattern`] / [`StarField`] paints the static
//!   background into a jumbo buffer at activation time.
//!
//! The engines themselves live in the [`crawl`] submodule and compose
//! the above: see [`crawl::Crawl`], [`crawl::TextCrawl`],
//! [`crawl::CrawlWindow`], and the [`crawl::StarCrawl`] preset.
//!
//! ## External buffer contract
//!
//! Every non-trivial buffer used by motion widgets is owned by the
//! host and passed in via a lifetime-bound reference. This lets the
//! STM32H747I-DISCO target place the jumbo background and A8 text
//! source in SDRAM at fixed addresses, while host targets keep them
//! in plain `Vec<u8>` allocated on the heap. The widget crate never
//! allocates motion buffers itself.

pub mod background;
pub mod crawl;
pub mod direction;
pub mod jumbo;
pub mod rate;

pub use background::{BackgroundPattern, StarField};
pub use crawl::{Crawl, CrawlState, CrawlWindow, StarCrawl, TextCrawl};
pub use direction::Direction;
pub use jumbo::{JumboBuffer, JumboOrientation};
pub use rate::{FrameRoundedRate, MotionRate, SubPixelRate};
