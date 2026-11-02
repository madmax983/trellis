//! The gardeners: small search engines with tempers.
//!
//! Every gardener looks one move ahead. For each turn that does not wilt at
//! once it measures, by breadth-first search over empty cells:
//!
//! * **reach** — how many cells the vine could still grow into from there;
//! * **territory** — a Voronoi split of the trellis: cells strictly closer to
//!   this vine's new head than to any other growing head (ties belong to
//!   nobody), and the same count for each rival;
//! * **contact** — whether any rival's head borders that reachable region.
//!
//! A vine without contact is sealed in a pocket, and every gardener switches
//! to *packing*: maximise reach, hug walls. Otherwise each scores by temper.
//! Everything is integer arithmetic over a fixed visit order, so the browser
//! port makes the same choice on every tick.

use serde::{Deserialize, Serialize};

use crate::garden::{Garden, VineId};
use crate::hex::{Cell, OFF, Turn};
use crate::rng::Rng;

/// A gardener's temper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Temper {
    /// Owns the most territory.
    Ivy,
    /// Closes on the nearest rival and shrinks its territory.
    Kudzu,
    /// Hugs walls and wastes nothing.
    Moss,
    /// Rolls a seeded die.
    Hops,
}

impl Temper {
    /// Every temper, in seating order.
    pub const ALL: [Self; 4] = [Self::Ivy, Self::Kudzu, Self::Moss, Self::Hops];

    /// The temper's name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ivy => "ivy",
            Self::Kudzu => "kudzu",
            Self::Moss => "moss",
            Self::Hops => "hops",
        }
    }

    /// Parse a name.
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == s)
    }

    /// One line on how it plays.
    pub const fn motto(self) -> &'static str {
        match self {
            Self::Ivy => "counts every cell it is closer to than you",
            Self::Kudzu => "closes on the nearest vine and starves it",
            Self::Moss => "hugs the wall and wastes nothing",
            Self::Hops => "rolls a die, mostly sensibly",
        }
    }
}

/// What a gardener saw for one candidate turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Look {
    /// The turn.
    pub turn: Turn,
    /// The cell it reaches.
    pub cell: Cell,
    /// Cells reachable from there (including it).
    pub reach: u32,
    /// Cells strictly closer to this head than to any rival's.
    pub mine: u32,
    /// The same for the nearest rival by hex distance.
    pub nearest_theirs: u32,
    /// Sum of every rival's territory.
    pub theirs: u32,
    /// Hex distance to the nearest growing rival head (0 if none).
    pub nearest: u32,
    /// Off-board or held neighbours of the cell.
    pub walls: u32,
    /// Rival heads that could reach the same cell this tick.
    pub danger: u32,
    /// Does any rival head border the reachable region?
    pub contact: bool,
    /// The score the temper gave it.
    pub score: i64,
}

/// A gardener's choice and what it weighed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    /// The turn it takes.
    pub turn: Turn,
    /// Every turn that did not wilt at once, in [`Turn::ORDER`].
    pub looks: Vec<Look>,
}

const UNSEEN: u32 = u32::MAX;
const TIE: u8 = u8::MAX;

/// Scratch space reused across searches.
#[derive(Debug, Clone, Default)]
pub struct Scratch {
    dist: Vec<u32>,
    who: Vec<u8>,
    queue: Vec<u16>,
}

impl Scratch {
    fn reset(&mut self, n: usize) {
        self.dist.clear();
        self.dist.resize(n, UNSEEN);
        self.who.clear();
        self.who.resize(n, TIE);
        self.queue.clear();
    }
}

/// Is `c` empty and not the hypothetical new head?
fn open(g: &Garden, c: u16, block: Cell) -> bool {
    c != OFF && c != block.0 && g.owners()[c as usize] == 0
}

/// Flood from `from` (treated as held) through empty cells. Returns reach
/// including `from`, and whether a rival head borders the region.
fn flood(garden: &Garden, me: VineId, from: Cell, scratch: &mut Scratch) -> (u32, bool) {
    scratch.reset(garden.board().len());
    let heads: Vec<u16> = garden
        .growing()
        .filter(|&v| v != me)
        .map(|v| garden.vine(v).head.0)
        .collect();
    scratch.dist[from.ix()] = 0;
    scratch.queue.push(from.0);
    let mut reach = 1;
    let mut contact = false;
    let mut next = 0;
    while next < scratch.queue.len() {
        let here = scratch.queue[next];
        next += 1;
        for nb in garden.board().neighbours(Cell(here)) {
            if nb == OFF {
                continue;
            }
            if heads.contains(&nb) {
                contact = true;
            }
            if open(garden, nb, from) && scratch.dist[nb as usize] == UNSEEN {
                scratch.dist[nb as usize] = 0;
                scratch.queue.push(nb);
                reach += 1;
            }
        }
    }
    (reach, contact)
}

/// Multi-source Voronoi: `me` from `from`, every rival from its head.
/// Returns territory per seat (index = vine number).
fn voronoi(garden: &Garden, me: VineId, from: Cell, scratch: &mut Scratch) -> Vec<u32> {
    scratch.reset(garden.board().len());
    let mut count = vec![0_u32; garden.vines().len()];
    scratch.dist[from.ix()] = 0;
    scratch.who[from.ix()] = me.0;
    scratch.queue.push(from.0);
    for v in garden.growing() {
        if v == me {
            continue;
        }
        let head = garden.vine(v).head;
        scratch.dist[head.ix()] = 0;
        scratch.who[head.ix()] = v.0;
        scratch.queue.push(head.0);
    }
    let mut next = 0;
    while next < scratch.queue.len() {
        let here = scratch.queue[next];
        next += 1;
        let depth = scratch.dist[here as usize] + 1;
        let source = scratch.who[here as usize];
        for nb in garden.board().neighbours(Cell(here)) {
            if !open(garden, nb, from) {
                continue;
            }
            let at = nb as usize;
            if scratch.dist[at] == UNSEEN {
                scratch.dist[at] = depth;
                scratch.who[at] = source;
                scratch.queue.push(nb);
                if source != TIE {
                    count[source as usize] += 1;
                }
            } else if scratch.dist[at] == depth
                && scratch.who[at] != source
                && scratch.who[at] != TIE
            {
                count[scratch.who[at] as usize] -= 1;
                scratch.who[at] = TIE;
            }
        }
    }
    count
}

/// Weigh every turn and choose. `rng` is drawn from only by [`Temper::Hops`],
/// exactly once per non-wilting turn.
pub fn choose(g: &Garden, me: VineId, temper: Temper, rng: &mut Rng, s: &mut Scratch) -> Choice {
    let rivals: Vec<VineId> = g.growing().filter(|&v| v != me).collect();
    let mut looks = Vec::with_capacity(3);
    for turn in Turn::ORDER {
        let Some(cell) = g.target(me, turn) else {
            continue;
        };
        if !g.empty(cell) {
            continue;
        }
        let (reach, contact) = flood(g, me, cell, s);
        let terr = voronoi(g, me, cell, s);
        let mine = terr[me.ix()];
        let theirs: u32 = rivals.iter().map(|v| terr[v.ix()]).sum();
        let mut nearest = 0;
        let mut nearest_theirs = 0;
        let mut best = u32::MAX;
        for &v in &rivals {
            let d = g.board().distance(cell, g.vine(v).head);
            if d < best {
                best = d;
                nearest = d;
                nearest_theirs = terr[v.ix()];
            }
        }
        let walls = u32::try_from(
            g.board()
                .neighbours(cell)
                .iter()
                .filter(|&&nb| nb == OFF || g.owners()[nb as usize] != 0)
                .count(),
        )
        .unwrap_or(6);
        let danger = u32::try_from(
            rivals
                .iter()
                .filter(|&&v| g.board().neighbours(g.vine(v).head).contains(&cell.0))
                .count(),
        )
        .unwrap_or(0);
        looks.push(Look {
            turn,
            cell,
            reach,
            mine,
            nearest_theirs,
            theirs,
            nearest,
            walls,
            danger,
            contact,
            score: 0,
        });
    }
    if looks.is_empty() {
        return Choice {
            turn: Turn::Straight,
            looks,
        };
    }
    let best_reach = looks.iter().map(|l| l.reach).max().unwrap_or(0);
    for l in &mut looks {
        let roll = if temper == Temper::Hops {
            i64::from(rng.below(600))
        } else {
            0
        };
        l.score = score(temper, l, best_reach, roll);
    }
    let mut pick = 0;
    for (k, l) in looks.iter().enumerate() {
        if l.score > looks[pick].score {
            pick = k;
        }
    }
    Choice {
        turn: looks[pick].turn,
        looks,
    }
}

fn score(temper: Temper, l: &Look, best_reach: u32, roll: i64) -> i64 {
    let reach = i64::from(l.reach);
    let mine = i64::from(l.mine);
    let walls = i64::from(l.walls);
    let danger = i64::from(l.danger);
    // A turn into a pocket under half the size of the best one is a trap.
    let trap = if l.reach * 2 < best_reach {
        -1_000_000
    } else {
        0
    };
    if !l.contact {
        return trap + reach * 100 + walls * 7 - danger * 5_000;
    }
    let body = match temper {
        Temper::Ivy => mine * 20 - i64::from(l.theirs) * 4 + reach - danger * 5_000,
        Temper::Kudzu => {
            mine * 10
                - i64::from(l.nearest_theirs) * 12
                - i64::from(l.nearest) * 30
                - danger * 2_000
        }
        Temper::Moss => reach * 20 + walls * 15 + mine * 4 - danger * 5_000,
        Temper::Hops => reach * 6 + mine * 6 + roll - danger * 3_000,
    };
    trap + body
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex::{Board, Dir};

    #[test]
    fn a_gardener_never_walks_into_the_wall_when_it_can_turn() {
        let b = Board::new(4).unwrap();
        let edge = b.at(4, -2).unwrap();
        let g = Garden::plant(b, &[(edge, Dir::ALL[0])]).unwrap();
        let mut s = Scratch::default();
        for t in Temper::ALL {
            let c = choose(&g, VineId(0), t, &mut Rng::new(7), &mut s);
            assert_ne!(c.turn, Turn::Straight, "{t:?}");
            assert!(!c.looks.is_empty());
        }
    }

    #[test]
    fn territory_and_reach_see_the_open_board() {
        let board = Board::new(6).unwrap();
        let a = board.at(-3, 0).unwrap();
        let z = board.at(3, 0).unwrap();
        let g = Garden::plant(board.clone(), &[(a, Dir::ALL[2]), (z, Dir::ALL[5])]).unwrap();
        let mut scratch = Scratch::default();
        let t0 = voronoi(
            &g,
            VineId(0),
            board.step(a, Dir::ALL[2]).unwrap(),
            &mut scratch,
        );
        assert!(t0[0] > 0 && t0[1] > 0);
        let (reach, contact) = flood(
            &g,
            VineId(0),
            board.step(a, Dir::ALL[2]).unwrap(),
            &mut scratch,
        );
        assert!(contact);
        assert_eq!(reach as usize, g.free());
    }

    #[test]
    fn hops_draws_once_per_safe_turn() {
        let board = Board::new(6).unwrap();
        let g = Garden::plant(board.clone(), &[(board.at(0, 0).unwrap(), Dir::ALL[0])]).unwrap();
        let mut scratch = Scratch::default();
        let mut r = Rng::new(3);
        let c = choose(&g, VineId(0), Temper::Hops, &mut r, &mut scratch);
        assert_eq!(c.looks.len(), 3);
        let mut r2 = Rng::new(3);
        for _ in 0..3 {
            r2.next_u32();
        }
        assert_eq!(r, r2);
        let mut r3 = Rng::new(3);
        choose(&g, VineId(0), Temper::Ivy, &mut r3, &mut scratch);
        assert_eq!(r3, Rng::new(3));
    }
}
