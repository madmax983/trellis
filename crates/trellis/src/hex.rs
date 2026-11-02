//! The trellis: a hexagon of hexagons in axial coordinates.
//!
//! Cells are numbered row by row (`r` from `-R` to `R`, then `q` ascending),
//! and the browser port numbers them the same way, so a cell index means the
//! same thing on both sides of the wire.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The six axial directions, counter-clockwise from east.
///
/// `DIRS[d]` and `DIRS[(d + 3) % 6]` are negations of each other, which is the
/// whole of [`Dir::opposite`].
pub const DIRS: [(i32, i32); 6] = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];

/// The largest trellis this crate builds. 3·24·25+1 = 1,801 cells.
pub const MAX_RADIUS: u8 = 24;

/// The smallest trellis worth growing on.
pub const MIN_RADIUS: u8 = 3;

/// A cell of the trellis, by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Cell(pub u16);

impl Cell {
    /// The index as a `usize`, for slices.
    pub const fn ix(self) -> usize {
        self.0 as usize
    }
}

/// One of the six headings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Dir(u8);

impl Dir {
    /// Every heading, in order.
    pub const ALL: [Self; 6] = [Self(0), Self(1), Self(2), Self(3), Self(4), Self(5)];

    /// A heading from its number, if it is one.
    pub const fn new(d: u8) -> Option<Self> {
        if d < 6 { Some(Self(d)) } else { None }
    }

    /// The heading's number, `0..6`.
    pub const fn get(self) -> u8 {
        self.0
    }

    /// The heading after a turn. Left is counter-clockwise.
    #[must_use]
    pub const fn turn(self, t: Turn) -> Self {
        Self((self.0 + t.delta()) % 6)
    }

    /// The heading straight back.
    #[must_use]
    pub const fn opposite(self) -> Self {
        Self((self.0 + 3) % 6)
    }

    /// The axial step.
    pub const fn step(self) -> (i32, i32) {
        DIRS[self.0 as usize]
    }
}

/// What a vine does with its heading each tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Turn {
    /// Sixty degrees counter-clockwise.
    Left,
    /// Keep the heading.
    Straight,
    /// Sixty degrees clockwise.
    Right,
}

impl Turn {
    /// The order gardeners consider turns in, and break ties by.
    pub const ORDER: [Self; 3] = [Self::Straight, Self::Left, Self::Right];

    /// The turn as a heading increment modulo 6.
    pub const fn delta(self) -> u8 {
        match self {
            Self::Left => 1,
            Self::Straight => 0,
            Self::Right => 5,
        }
    }

    /// Parse `left` / `straight` / `right`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "left" | "l" => Some(Self::Left),
            "straight" | "s" => Some(Self::Straight),
            "right" | "r" => Some(Self::Right),
            _ => None,
        }
    }

    /// The name [`Self::parse`] reads.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Straight => "straight",
            Self::Right => "right",
        }
    }
}

/// Why a trellis could not be built.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BoardError {
    /// The radius is outside `MIN_RADIUS..=MAX_RADIUS`.
    #[error("a trellis has radius {MIN_RADIUS} to {MAX_RADIUS}, not {0}")]
    Radius(u8),
}

/// The hexagonal board and its neighbour table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    radius: u8,
    coords: Vec<(i32, i32)>,
    /// `(q + R) + (r + R)·(2R + 1)` to a cell index, or `u16::MAX`.
    grid: Vec<u16>,
    /// `nbr[c][d]`, or `u16::MAX` off the edge.
    nbr: Vec<[u16; 6]>,
}

/// The off-the-edge marker in the neighbour table.
pub const OFF: u16 = u16::MAX;

impl Board {
    /// The hexagon of the given radius.
    pub fn new(radius: u8) -> Result<Self, BoardError> {
        if !(MIN_RADIUS..=MAX_RADIUS).contains(&radius) {
            return Err(BoardError::Radius(radius));
        }
        let r = i32::from(radius);
        let side = 2 * r + 1;
        let mut grid = vec![OFF; usize::try_from(side * side).unwrap_or(0)];
        let mut coords = Vec::new();
        for row in -r..=r {
            for q in -r..=r {
                if hex_len(q, row) <= r {
                    let at = usize::try_from((q + r) + (row + r) * side).unwrap_or(0);
                    // At most 1,801 cells: always fits in a u16 below OFF.
                    grid[at] = u16::try_from(coords.len()).unwrap_or(OFF);
                    coords.push((q, row));
                }
            }
        }
        let mut board = Self {
            radius,
            coords,
            grid,
            nbr: Vec::new(),
        };
        board.nbr = (0..board.coords.len())
            .map(|c| {
                let (q, row) = board.coords[c];
                let mut out = [OFF; 6];
                for (d, (dq, dr)) in DIRS.iter().enumerate() {
                    out[d] = board.at(q + dq, row + dr).map_or(OFF, |x| x.0);
                }
                out
            })
            .collect();
        Ok(board)
    }

    /// The radius.
    pub const fn radius(&self) -> u8 {
        self.radius
    }

    /// How many cells: `3R(R+1) + 1`.
    pub const fn len(&self) -> usize {
        self.coords.len()
    }

    /// Never: the smallest trellis has 37 cells.
    pub const fn is_empty(&self) -> bool {
        self.coords.is_empty()
    }

    /// The cell at axial `(q, r)`, if it is on the trellis.
    pub fn at(&self, q: i32, r: i32) -> Option<Cell> {
        let rad = i32::from(self.radius);
        if hex_len(q, r) > rad {
            return None;
        }
        let side = 2 * rad + 1;
        let at = usize::try_from((q + rad) + (r + rad) * side).ok()?;
        match self.grid.get(at) {
            Some(&c) if c != OFF => Some(Cell(c)),
            _ => None,
        }
    }

    /// A cell's axial coordinates.
    pub fn coords(&self, c: Cell) -> (i32, i32) {
        self.coords[c.ix()]
    }

    /// The neighbour across heading `d`, if it is on the trellis.
    pub fn step(&self, c: Cell, d: Dir) -> Option<Cell> {
        let n = self.nbr[c.ix()][d.get() as usize];
        if n == OFF { None } else { Some(Cell(n)) }
    }

    /// The raw neighbour row, `OFF` for off the edge.
    pub fn neighbours(&self, c: Cell) -> [u16; 6] {
        self.nbr[c.ix()]
    }

    /// Hex distance between two cells.
    pub fn distance(&self, a: Cell, b: Cell) -> u32 {
        let (aq, ar) = self.coords(a);
        let (bq, br) = self.coords(b);
        hex_len(aq - bq, ar - br).unsigned_abs()
    }

    /// The ring of radius `rho` around the centre, walked counter-clockwise,
    /// with the heading of the walk at each cell. Empty for `rho == 0` or a
    /// ring that would leave the trellis.
    pub fn ring(&self, rho: u8) -> Vec<(Cell, Dir)> {
        if rho == 0 || rho > self.radius {
            return Vec::new();
        }
        let rho_i = i32::from(rho);
        let (sq, sr) = DIRS[4];
        let mut q = sq * rho_i;
        let mut r = sr * rho_i;
        let mut out = Vec::with_capacity(6 * rho as usize);
        for d in Dir::ALL {
            for _ in 0..rho {
                if let Some(c) = self.at(q, r) {
                    out.push((c, d));
                }
                let (dq, dr) = d.step();
                q += dq;
                r += dr;
            }
        }
        out
    }

    /// Every cell, in index order.
    pub fn cells(&self) -> impl Iterator<Item = Cell> + '_ {
        (0..self.coords.len()).map(|i| Cell(u16::try_from(i).unwrap_or(OFF)))
    }
}

/// Hex length of an axial vector: `max(|q|, |r|, |q + r|)`.
pub const fn hex_len(q: i32, r: i32) -> i32 {
    let along_q = q.abs();
    let along_r = r.abs();
    let along_s = (q + r).abs();
    let most = if along_q > along_r { along_q } else { along_r };
    if most > along_s { most } else { along_s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_centred_hexagonal_numbers() {
        for r in MIN_RADIUS..=MAX_RADIUS {
            let b = Board::new(r).unwrap();
            let r = usize::from(r);
            assert_eq!(b.len(), 3 * r * (r + 1) + 1);
        }
        assert!(Board::new(2).is_err());
        assert!(Board::new(25).is_err());
    }

    #[test]
    fn neighbours_are_symmetric() {
        let b = Board::new(9).unwrap();
        for c in b.cells() {
            for d in Dir::ALL {
                if let Some(n) = b.step(c, d) {
                    assert_eq!(b.step(n, d.opposite()), Some(c));
                    assert_eq!(b.distance(c, n), 1);
                }
            }
        }
    }

    #[test]
    fn turning_never_reverses() {
        for d in Dir::ALL {
            for t in Turn::ORDER {
                assert_ne!(d.turn(t), d.opposite());
            }
            assert_eq!(d.turn(Turn::Left).turn(Turn::Right), d);
        }
    }

    #[test]
    fn rings_have_six_rho_cells_at_distance_rho() {
        let b = Board::new(12).unwrap();
        let centre = b.at(0, 0).unwrap();
        for rho in 1..=12 {
            let ring = b.ring(rho);
            assert_eq!(ring.len(), 6 * usize::from(rho));
            for (c, d) in &ring {
                assert_eq!(b.distance(centre, *c), u32::from(rho));
                // Walking on along the ring's heading stays on the trellis.
                assert!(b.step(*c, *d).is_some());
            }
        }
    }

    #[test]
    fn edge_cells_have_off_neighbours() {
        let b = Board::new(4).unwrap();
        let corner = b.at(4, 0).unwrap();
        assert_eq!(b.step(corner, Dir::ALL[0]), None);
        let centre = b.at(0, 0).unwrap();
        assert!(Dir::ALL.iter().all(|d| b.step(centre, *d).is_some()));
    }
}
