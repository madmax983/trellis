//! A round: a seeded planting, a table of seats, and the humans' keystrokes.
//!
//! Gardeners are deterministic given the garden and one round-wide [`Rng`], so
//! a whole round is fixed by its seed, its seats and the turns humans made.
//! That triple is a [`Pressing`], and [`Pressing::regrow`] replays it to the
//! same garden, stem for stem.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::garden::{Garden, GardenError, MAX_VINES, Outcome, Tick, VineId};
use crate::gardener::{Scratch, Temper, choose};
use crate::hex::{Board, BoardError, Cell, Dir, Turn};
use crate::rng::Rng;

/// Who grows a vine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Seat {
    /// A gardener.
    Gardener {
        /// Its temper.
        temper: Temper,
    },
    /// A person, steering by keystrokes.
    Human,
}

/// One recorded keystroke: vine `vine` turned `turn` on the step that made
/// tick `tick`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keystroke {
    /// The tick the turn was applied to produce.
    pub tick: u32,
    /// Whose vine.
    pub vine: VineId,
    /// Which way.
    pub turn: Turn,
}

/// Why a round could not start or go on.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RoundError {
    /// A bad trellis.
    #[error(transparent)]
    Board(#[from] BoardError),
    /// A bad garden.
    #[error(transparent)]
    Garden(#[from] GardenError),
    /// A keystroke for a seat that is not human.
    #[error("vine {0} is not steered by a person")]
    NotHuman(u8),
}

/// The salt that separates the gardeners' die from the planting's.
const DIE_SALT: u32 = 0x9E37_79B9;

/// Plant `n` vines evenly on the ring two thirds of the way out, rotated by
/// the seed, each heading along the ring or a sixty degrees inward of it.
pub fn sow(board: &Board, n: usize, seed: u32) -> Vec<(Cell, Dir)> {
    let mut rng = Rng::new(seed);
    let rho = (u32::from(board.radius()) * 2 / 3).max(1);
    let spots = board.ring(u8::try_from(rho).unwrap_or(1));
    let len = u32::try_from(spots.len()).unwrap_or(1).max(1);
    let count = u32::try_from(n).unwrap_or(1).max(1);
    let offset = rng.below(len);
    (0..count)
        .map(|k| {
            let at = ((k * len) / count + offset) % len;
            let (cell, along) = spots[at as usize];
            let bend = if rng.below(3) == 1 {
                Turn::Left
            } else {
                Turn::Straight
            };
            (cell, along.turn(bend))
        })
        .collect()
}

/// A round in progress.
#[derive(Debug, Clone)]
pub struct Round {
    seed: u32,
    seats: Vec<Seat>,
    garden: Garden,
    die: Rng,
    scratch: Scratch,
    keys: Vec<Keystroke>,
}

impl Round {
    /// Plant a round.
    pub fn new(radius: u8, seed: u32, seats: &[Seat]) -> Result<Self, RoundError> {
        if seats.is_empty() || seats.len() > MAX_VINES {
            return Err(GardenError::Vines(seats.len()).into());
        }
        let board = Board::new(radius)?;
        let starts = sow(&board, seats.len(), seed);
        let garden = Garden::plant(board, &starts)?;
        Ok(Self {
            seed,
            seats: seats.to_vec(),
            garden,
            die: Rng::new(seed ^ DIE_SALT),
            scratch: Scratch::default(),
            keys: Vec::new(),
        })
    }

    /// The seed.
    pub const fn seed(&self) -> u32 {
        self.seed
    }

    /// The seats.
    pub fn seats(&self) -> &[Seat] {
        &self.seats
    }

    /// The garden.
    pub const fn garden(&self) -> &Garden {
        &self.garden
    }

    /// Every keystroke so far.
    pub fn keys(&self) -> &[Keystroke] {
        &self.keys
    }

    /// The outcome so far.
    pub fn outcome(&self) -> Outcome {
        self.garden.outcome()
    }

    /// One tick. Gardeners choose (in seat order, each drawing from the
    /// round's die as its temper does); each human seat takes the turn given
    /// for it in `human`, or goes straight.
    pub fn tick(&mut self, human: &[(VineId, Turn)]) -> Result<Tick, RoundError> {
        for (v, _) in human {
            if self.seats.get(v.ix()) != Some(&Seat::Human) {
                return Err(RoundError::NotHuman(v.0));
            }
        }
        if self.garden.over() {
            return Ok(Tick {
                tick: self.garden.tick(),
                ..Tick::default()
            });
        }
        let next = self.garden.tick() + 1;
        let mut turns = vec![Turn::Straight; self.seats.len()];
        for (k, seat) in self.seats.iter().enumerate() {
            let v = VineId(u8::try_from(k).unwrap_or(u8::MAX));
            if !self.garden.vine(v).alive() {
                continue;
            }
            match seat {
                Seat::Gardener { temper } => {
                    turns[k] =
                        choose(&self.garden, v, *temper, &mut self.die, &mut self.scratch).turn;
                }
                Seat::Human => {
                    if let Some((_, t)) = human.iter().rev().find(|(h, _)| *h == v) {
                        turns[k] = *t;
                        if *t != Turn::Straight {
                            self.keys.push(Keystroke {
                                tick: next,
                                vine: v,
                                turn: *t,
                            });
                        }
                    }
                }
            }
        }
        Ok(self.garden.step(&turns)?)
    }

    /// Play to the end with no human input. Bounded by the board size: every
    /// tick before the end fills at least one cell.
    pub fn play_out(&mut self) -> Result<Outcome, RoundError> {
        let bound = self.garden.board().len() + 1;
        for _ in 0..bound {
            if self.garden.over() {
                break;
            }
            self.tick(&[])?;
        }
        Ok(self.garden.outcome())
    }

    /// Press the round: everything needed to regrow it.
    pub fn press(&self) -> Pressing {
        Pressing {
            radius: self.garden.board().radius(),
            seed: self.seed,
            seats: self.seats.clone(),
            keys: self.keys.clone(),
            ticks: self.garden.tick(),
        }
    }
}

/// A finished (or interrupted) round, stored as its causes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pressing {
    /// The trellis radius.
    pub radius: u8,
    /// The planting seed.
    pub seed: u32,
    /// The seats.
    pub seats: Vec<Seat>,
    /// The humans' non-straight turns.
    pub keys: Vec<Keystroke>,
    /// How many ticks it ran.
    pub ticks: u32,
}

impl Pressing {
    /// Regrow it, tick for tick.
    pub fn regrow(&self) -> Result<Round, RoundError> {
        let mut round = Round::new(self.radius, self.seed, &self.seats)?;
        let mut k = 0;
        for t in 1..=self.ticks {
            let mut human = Vec::new();
            while k < self.keys.len() && self.keys[k].tick == t {
                human.push((self.keys[k].vine, self.keys[k].turn));
                k += 1;
            }
            round.tick(&human)?;
        }
        Ok(round)
    }
}

/// The standard table of `n` gardeners, tempers in rotation.
pub fn gardeners(n: usize) -> Vec<Seat> {
    (0..n)
        .map(|k| Seat::Gardener {
            temper: Temper::ALL[k % Temper::ALL.len()],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sowing_spreads_vines_on_distinct_cells() {
        let b = Board::new(16).unwrap();
        for seed in 0..50 {
            for n in 1..=MAX_VINES {
                let s = sow(&b, n, seed);
                assert_eq!(s.len(), n);
                let mut cells: Vec<_> = s.iter().map(|x| x.0).collect();
                cells.sort();
                cells.dedup();
                assert_eq!(cells.len(), n);
                for (c, d) in s {
                    assert!(b.step(c, d).is_some());
                }
            }
        }
    }

    #[test]
    fn a_table_of_gardeners_plays_to_an_end() {
        for seed in 0..12 {
            let mut r = Round::new(12, seed, &gardeners(6)).unwrap();
            let o = r.play_out().unwrap();
            assert_ne!(o, Outcome::Growing);
            assert!(r.garden().tick() as usize <= r.garden().board().len());
        }
    }

    #[test]
    fn a_pressing_regrows_with_human_keystrokes() {
        let seats = [
            Seat::Human,
            Seat::Gardener {
                temper: Temper::Kudzu,
            },
            Seat::Gardener {
                temper: Temper::Moss,
            },
        ];
        let mut r = Round::new(10, 99, &seats).unwrap();
        let script = [
            Turn::Left,
            Turn::Straight,
            Turn::Right,
            Turn::Right,
            Turn::Left,
        ];
        let mut i = 0;
        while !r.garden().over() {
            let t = script[i % script.len()];
            i += 1;
            r.tick(&[(VineId(0), t)]).unwrap();
        }
        let p = r.press();
        let again = p.regrow().unwrap();
        assert_eq!(again.garden(), r.garden());
        assert!(!p.keys.is_empty());
    }

    #[test]
    fn a_gardener_cannot_be_steered() {
        let mut r = Round::new(8, 1, &gardeners(2)).unwrap();
        assert_eq!(
            r.tick(&[(VineId(1), Turn::Left)]),
            Err(RoundError::NotHuman(1))
        );
    }
}
