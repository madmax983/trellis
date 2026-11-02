//! # trellis
//!
//! A hexagonal light-cycle game played by vines, and the gardeners who play it.
//! No IO, no async, no clock: the engine is the product, and the app around it
//! is a window.
//!
//! * [`hex`] — the trellis: axial hexagon, neighbour table, rings.
//! * [`garden`] — the rules: simultaneous growth, wilting, outcomes.
//! * [`gardener`] — the bots: flood fill, Voronoi territory, tempers.
//! * [`round`] — seats, a seeded planting, keystrokes, and pressings that
//!   regrow stem for stem.
//! * [`rng`] — mulberry32 and a word-to-seed hash the browser shares.
//!
//! ```
//! use trellis::round::{Round, gardeners};
//! let mut r = Round::new(12, 7, &gardeners(4))?;
//! let outcome = r.play_out()?;
//! assert!(r.garden().over());
//! # let _ = outcome;
//! # Ok::<(), trellis::round::RoundError>(())
//! ```

pub mod garden;
pub mod gardener;
pub mod hex;
pub mod rng;
pub mod round;
