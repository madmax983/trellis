//! The rules: vines on a trellis, growing simultaneously.
//!
//! One tick, for every vine still growing, at the same instant:
//!
//! 1. turn the heading by that vine's [`Turn`] (never a reversal);
//! 2. the *target* is the neighbour across the new heading;
//! 3. the vine **wilts** if the target is off the trellis ([`Death::Wall`]),
//!    already holds any stem ([`Death::Stem`], its own included), or is the
//!    target of another growing vine this tick ([`Death::Clash`]);
//! 4. every vine that did not wilt takes its target.
//!
//! Stems never move and never vanish, so the number of empty cells strictly
//! falls on every tick at least one vine survives — the game ends within
//! `cells` ticks. That, and that no cell ever holds two stems, is what
//! `verus/src/trellis.rs` proves.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::hex::{Board, Cell, Dir, Turn};

/// A vine, by seat number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VineId(pub u8);

impl VineId {
    /// The seat as a `usize`.
    pub const fn ix(self) -> usize {
        self.0 as usize
    }
}

/// The largest table.
pub const MAX_VINES: usize = 8;

/// How a vine wilted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Death {
    /// It grew off the edge of the trellis.
    Wall,
    /// It grew into a stem: `into` is whose.
    Stem {
        /// The owner of the stem it hit (maybe itself).
        into: VineId,
    },
    /// It and another vine reached for the same cell at once.
    Clash {
        /// The lowest-numbered other vine that reached for it.
        with: VineId,
    },
}

/// One vine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vine {
    /// Where it is now.
    pub head: Cell,
    /// Which way it faces.
    pub heading: Dir,
    /// Every cell it holds, in the order it took them.
    pub stem: Vec<Cell>,
    /// When and how it wilted, if it has.
    pub wilted: Option<(u32, Death)>,
}

impl Vine {
    /// Still growing.
    pub const fn alive(&self) -> bool {
        self.wilted.is_none()
    }
}

/// What one tick did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tick {
    /// The tick number after the step.
    pub tick: u32,
    /// Vines that grew, and where to.
    pub grew: Vec<(VineId, Cell)>,
    /// Vines that wilted, and how.
    pub wilted: Vec<(VineId, Death)>,
}

/// How the game stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Outcome {
    /// Still growing.
    Growing,
    /// One vine outgrew the rest.
    Won {
        /// The winner.
        by: VineId,
    },
    /// The last vines wilted on the same tick.
    Draw,
}

/// Why a garden could not be planted.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GardenError {
    /// No vines, or more than [`MAX_VINES`].
    #[error("a garden has 1 to {MAX_VINES} vines, not {0}")]
    Vines(usize),
    /// Two vines planted in one cell.
    #[error("two vines cannot be planted in cell {0}")]
    Crowded(u16),
    /// A starting cell that is not on the trellis.
    #[error("cell {0} is not on the trellis")]
    Off(u16),
    /// A turn list that does not name every vine.
    #[error("{got} turns for {want} vines")]
    Turns {
        /// Turns given.
        got: usize,
        /// Vines planted.
        want: usize,
    },
}

/// The trellis and everything growing on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Garden {
    board: Board,
    /// `0` empty, `k + 1` held by vine `k`.
    owner: Vec<u8>,
    vines: Vec<Vine>,
    tick: u32,
}

impl Garden {
    /// Plant vines at given cells with given headings.
    pub fn plant(board: Board, starts: &[(Cell, Dir)]) -> Result<Self, GardenError> {
        if starts.is_empty() || starts.len() > MAX_VINES {
            return Err(GardenError::Vines(starts.len()));
        }
        let mut owner = vec![0_u8; board.len()];
        let mut vines = Vec::with_capacity(starts.len());
        for (k, (c, d)) in starts.iter().enumerate() {
            let slot = owner.get_mut(c.ix()).ok_or(GardenError::Off(c.0))?;
            if *slot != 0 {
                return Err(GardenError::Crowded(c.0));
            }
            // k < MAX_VINES = 8, so k + 1 fits.
            *slot = u8::try_from(k + 1).unwrap_or(u8::MAX);
            vines.push(Vine {
                head: *c,
                heading: *d,
                stem: vec![*c],
                wilted: None,
            });
        }
        Ok(Self {
            board,
            owner,
            vines,
            tick: 0,
        })
    }

    /// The board.
    pub const fn board(&self) -> &Board {
        &self.board
    }

    /// Every vine.
    pub fn vines(&self) -> &[Vine] {
        &self.vines
    }

    /// One vine.
    pub fn vine(&self, v: VineId) -> &Vine {
        &self.vines[v.ix()]
    }

    /// Ticks so far.
    pub const fn tick(&self) -> u32 {
        self.tick
    }

    /// Who holds a cell.
    pub fn owner(&self, c: Cell) -> Option<VineId> {
        match self.owner[c.ix()] {
            0 => None,
            k => Some(VineId(k - 1)),
        }
    }

    /// The raw owner table: `0` empty, `k + 1` vine `k`.
    pub fn owners(&self) -> &[u8] {
        &self.owner
    }

    /// Is a cell empty?
    pub fn empty(&self, c: Cell) -> bool {
        self.owner[c.ix()] == 0
    }

    /// How many cells hold no stem.
    pub fn free(&self) -> usize {
        self.owner
            .iter()
            .fold(0, |free, &o| free + usize::from(o == 0))
    }

    /// Vines still growing.
    pub fn growing(&self) -> impl Iterator<Item = VineId> + '_ {
        self.vines
            .iter()
            .enumerate()
            .filter(|(_, v)| v.alive())
            .map(|(k, _)| VineId(u8::try_from(k).unwrap_or(u8::MAX)))
    }

    /// How many vines are still growing.
    pub fn alive(&self) -> usize {
        self.vines.iter().filter(|v| v.alive()).count()
    }

    /// How the game stands. A solo garden ends when its vine wilts; any other
    /// ends when at most one vine grows.
    pub fn outcome(&self) -> Outcome {
        let alive: Vec<VineId> = self.growing().collect();
        if self.vines.len() == 1 {
            return if alive.is_empty() {
                Outcome::Won { by: VineId(0) }
            } else {
                Outcome::Growing
            };
        }
        match alive.as_slice() {
            [] => Outcome::Draw,
            [one] => Outcome::Won { by: *one },
            _ => Outcome::Growing,
        }
    }

    /// Is the game over?
    pub fn over(&self) -> bool {
        self.outcome() != Outcome::Growing
    }

    /// Where a growing vine would reach with a turn: `None` off the edge.
    pub fn target(&self, v: VineId, t: Turn) -> Option<Cell> {
        let vine = &self.vines[v.ix()];
        self.board.step(vine.head, vine.heading.turn(t))
    }

    /// One simultaneous tick. `turns[k]` is vine `k`'s turn; wilted vines'
    /// turns are ignored. A finished game does not step.
    pub fn step(&mut self, turns: &[Turn]) -> Result<Tick, GardenError> {
        if turns.len() != self.vines.len() {
            return Err(GardenError::Turns {
                got: turns.len(),
                want: self.vines.len(),
            });
        }
        if self.over() {
            return Ok(Tick {
                tick: self.tick,
                ..Tick::default()
            });
        }
        let n = self.vines.len();
        let mut heading = Vec::with_capacity(n);
        let mut target: Vec<Option<Cell>> = Vec::with_capacity(n);
        for (k, vine) in self.vines.iter().enumerate() {
            let h = vine.heading.turn(turns[k]);
            heading.push(h);
            target.push(if vine.alive() {
                self.board.step(vine.head, h)
            } else {
                None
            });
        }
        let next = self.tick + 1;
        let mut out = Tick {
            tick: next,
            ..Tick::default()
        };
        let mut fate: Vec<Option<Death>> = vec![None; n];
        for k in 0..n {
            if !self.vines[k].alive() {
                continue;
            }
            fate[k] = match target[k] {
                None => Some(Death::Wall),
                Some(t) if self.owner[t.ix()] != 0 => Some(Death::Stem {
                    into: VineId(self.owner[t.ix()] - 1),
                }),
                Some(t) => (0..n)
                    .find(|&j| j != k && target[j] == Some(t))
                    .map(|j| Death::Clash {
                        with: VineId(u8::try_from(j).unwrap_or(u8::MAX)),
                    }),
            };
        }
        for k in 0..n {
            if !self.vines[k].alive() {
                continue;
            }
            let id = VineId(u8::try_from(k).unwrap_or(u8::MAX));
            match (fate[k], target[k]) {
                (Some(d), _) => {
                    self.vines[k].wilted = Some((next, d));
                    out.wilted.push((id, d));
                }
                (None, Some(t)) => {
                    // Survivors' targets are empty and pairwise distinct (a
                    // shared target is a clash for both), so no write lands on
                    // a stem or on another survivor's write.
                    self.owner[t.ix()] = id.0 + 1;
                    let vine = &mut self.vines[k];
                    vine.head = t;
                    vine.heading = heading[k];
                    vine.stem.push(t);
                    out.grew.push((id, t));
                }
                (None, None) => {
                    // Unreachable: a vine with no target is fated to the wall.
                }
            }
        }
        self.tick = next;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> Board {
        Board::new(5).unwrap()
    }

    fn at(b: &Board, q: i32, r: i32) -> Cell {
        b.at(q, r).unwrap()
    }

    const E: Dir = Dir::ALL[0];
    const W: Dir = Dir::ALL[3];

    #[test]
    fn a_straight_vine_runs_into_the_wall() {
        let b = board();
        let mut g = Garden::plant(b.clone(), &[(at(&b, 0, 0), E)]).unwrap();
        for _ in 0..5 {
            let t = g.step(&[Turn::Straight]).unwrap();
            assert_eq!(t.grew.len(), 1);
        }
        let t = g.step(&[Turn::Straight]).unwrap();
        assert_eq!(t.wilted, vec![(VineId(0), Death::Wall)]);
        assert_eq!(g.outcome(), Outcome::Won { by: VineId(0) });
        assert_eq!(g.vine(VineId(0)).stem.len(), 6);
    }

    #[test]
    fn head_to_head_is_a_clash_and_a_draw() {
        let b = board();
        // Two cells apart, facing: both reach for the middle.
        let mut g = Garden::plant(b.clone(), &[(at(&b, -1, 0), E), (at(&b, 1, 0), W)]).unwrap();
        let t = g.step(&[Turn::Straight, Turn::Straight]).unwrap();
        assert_eq!(t.grew, vec![]);
        assert_eq!(
            t.wilted,
            vec![
                (VineId(0), Death::Clash { with: VineId(1) }),
                (VineId(1), Death::Clash { with: VineId(0) })
            ]
        );
        assert_eq!(g.outcome(), Outcome::Draw);
        assert!(g.empty(at(&b, 0, 0)));
    }

    #[test]
    fn adjacent_heads_facing_each_other_both_hit_stems() {
        let b = board();
        let mut g = Garden::plant(b.clone(), &[(at(&b, 0, 0), E), (at(&b, 1, 0), W)]).unwrap();
        let t = g.step(&[Turn::Straight, Turn::Straight]).unwrap();
        assert_eq!(
            t.wilted,
            vec![
                (VineId(0), Death::Stem { into: VineId(1) }),
                (VineId(1), Death::Stem { into: VineId(0) })
            ]
        );
    }

    #[test]
    fn a_vine_can_strangle_itself() {
        let b = board();
        let mut g = Garden::plant(b.clone(), &[(at(&b, 0, 0), E)]).unwrap();
        // Six lefts walk a hexagon around a cell and come back to the start.
        let mut last = None;
        for _ in 0..6 {
            last = Some(g.step(&[Turn::Left]).unwrap());
        }
        let last = last.unwrap();
        assert_eq!(
            last.wilted,
            vec![(VineId(0), Death::Stem { into: VineId(0) })]
        );
        assert_eq!(g.vine(VineId(0)).stem.len(), 6);
    }

    #[test]
    fn the_game_stops_stepping_when_over() {
        let b = board();
        let mut g = Garden::plant(b.clone(), &[(at(&b, -1, 0), E), (at(&b, 1, 0), W)]).unwrap();
        g.step(&[Turn::Straight, Turn::Straight]).unwrap();
        let free = g.free();
        let t = g.step(&[Turn::Straight, Turn::Straight]).unwrap();
        assert!(t.grew.is_empty() && t.wilted.is_empty());
        assert_eq!(g.free(), free);
        assert_eq!(g.tick(), 1);
    }

    #[test]
    fn bad_plantings_are_refused() {
        let b = board();
        let c = at(&b, 0, 0);
        assert!(matches!(
            Garden::plant(b.clone(), &[(c, E), (c, W)]),
            Err(GardenError::Crowded(_))
        ));
        assert!(matches!(
            Garden::plant(b.clone(), &[]),
            Err(GardenError::Vines(0))
        ));
        assert!(matches!(
            Garden::plant(b.clone(), &[(Cell(9999), E)]),
            Err(GardenError::Off(9999))
        ));
        let mut g = Garden::plant(b, &[(c, E)]).unwrap();
        assert!(matches!(
            g.step(&[Turn::Left, Turn::Left]),
            Err(GardenError::Turns { got: 2, want: 1 })
        ));
    }
}
