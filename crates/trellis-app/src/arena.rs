//! The Arena: one wall, rounds that never stop, and a herbarium of pressings.
//!
//! The Arena is a state machine driven by *beats* — one beat per tick of the
//! growing round, eight a second in production. It has no clock of its own:
//! [`Arena::beat`] advances it one beat and returns the events to publish, so
//! the same code runs under the wall clock, under Autumn's simulated clock in
//! `tests/sim_arena.rs`, and in a plain loop in unit tests.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use trellis::garden::{Death, Outcome, Vine, VineId};
use trellis::gardener::Temper;
use trellis::hex::{Cell, Dir, Turn};
use trellis::rng::Rng;
use trellis::round::{Pressing, Round, RoundError, Seat};

/// The live-events topic.
pub const TOPIC: &str = "garden";
/// Trellis radius in the Arena: 817 cells.
pub const RADIUS: u8 = 16;
/// Vines per round.
pub const SEATS: usize = 6;
/// Beats of countdown before a round grows.
pub const SPROUT_BEATS: u32 = 20;
/// Beats a finished round stays on the wall.
pub const PRESSED_BEATS: u32 = 32;
/// Pressings the herbarium keeps.
pub const HERBARIUM: usize = 36;
/// Turns a person may queue ahead.
pub const QUEUE: usize = 3;
/// Rounds a seated person may sit out without steering before losing the seat.
pub const IDLE_ROUNDS: u32 = 2;

/// The gardeners who fill empty seats, with the colour that is theirs.
pub const CAST: [(&str, Temper, u32); 6] = [
    ("Ivy", Temper::Ivy, 135),
    ("Kudzu", Temper::Kudzu, 285),
    ("Moss", Temper::Moss, 90),
    ("Hops", Temper::Hops, 45),
    ("Clematis", Temper::Ivy, 200),
    ("Bindweed", Temper::Kudzu, 330),
];

/// A person's ticket: proof of a join, sent with every steer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ticket(pub u64);

impl Ticket {
    /// Hex, for the wire.
    pub fn hex(self) -> String {
        format!("{:016x}", self.0)
    }

    /// From hex.
    pub fn parse(s: &str) -> Option<Self> {
        u64::from_str_radix(s.trim(), 16).ok().map(Self)
    }
}

/// A round number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RoundNo(pub u64);

/// Who is in a seat, as the wire sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatView {
    /// Display name.
    pub name: String,
    /// `gardener` or `human`.
    pub kind: String,
    /// The gardener's temper, if any.
    pub temper: Option<Temper>,
    /// Hue, 0–359.
    pub hue: u32,
}

/// Where the Arena is in its cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// Counting down; vines planted, not yet growing.
    Sprouting,
    /// Growing, one tick per beat.
    Growing,
    /// Finished, on display.
    Pressed,
}

/// One event, for every browser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Event {
    /// A new round is planted.
    Round {
        /// Round number.
        round: RoundNo,
        /// Seed.
        seed: u32,
        /// Trellis radius.
        radius: u8,
        /// Seats.
        seats: Vec<SeatView>,
        /// Planting.
        starts: Vec<(Cell, Dir)>,
        /// Beats until it grows.
        beats: u32,
    },
    /// One tick of growth.
    Tick {
        /// Round number.
        round: RoundNo,
        /// Tick number.
        tick: u32,
        /// Who grew where.
        grew: Vec<(VineId, Cell)>,
        /// Who wilted how.
        wilted: Vec<(VineId, Death)>,
    },
    /// The round is over.
    Pressed {
        /// Round number.
        round: RoundNo,
        /// How it ended.
        outcome: Outcome,
        /// Ticks played.
        ticks: u32,
        /// Stem length per seat.
        lengths: Vec<u32>,
    },
    /// Someone joined the queue.
    Joined {
        /// Their name.
        name: String,
        /// Their hue.
        hue: u32,
    },
}

/// A pressed round, kept in the herbarium.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pressed {
    /// Round number.
    pub round: RoundNo,
    /// The causes: seed, seats, keystrokes.
    pub pressing: Pressing,
    /// Who sat where.
    pub seats: Vec<SeatView>,
    /// How it ended.
    pub outcome: Outcome,
}

/// Everything a late browser needs.
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    /// Round number.
    pub round: RoundNo,
    /// Phase.
    pub phase: Phase,
    /// Beats left in the phase (sprouting / pressed).
    pub beats: u32,
    /// Seed.
    pub seed: u32,
    /// Radius.
    pub radius: u8,
    /// Seats.
    pub seats: Vec<SeatView>,
    /// Ticks so far.
    pub tick: u32,
    /// Every vine.
    pub vines: Vec<Vine>,
    /// Outcome so far.
    pub outcome: Outcome,
    /// People waiting or seated.
    pub players: usize,
    /// Rounds in the herbarium.
    pub pressed: usize,
    /// Whether the Arena is running.
    pub running: bool,
    /// Wins per name since boot.
    pub wins: Vec<(String, u32)>,
}

/// Why a person's request was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ArenaError {
    /// A ticket the Arena never issued, or one that has left.
    #[error("that ticket is not in the garden")]
    UnknownTicket,
    /// The ticket's vine is not growing in this round.
    #[error("your vine is not growing this round")]
    NotGrowing,
    /// The steering queue is full.
    #[error("you have {QUEUE} turns queued already")]
    QueueFull,
    /// A name that is empty or too long.
    #[error("a name is 1 to 16 letters")]
    Name,
    /// The engine refused.
    #[error(transparent)]
    Round(#[from] RoundError),
}

#[derive(Debug, Clone)]
struct Player {
    name: String,
    hue: u32,
    idle: u32,
}

#[derive(Debug)]
struct Inner {
    no: RoundNo,
    phase: Phase,
    beats: u32,
    round: Round,
    seats: Vec<SeatView>,
    /// Which ticket holds which seat this round.
    holders: Vec<Option<Ticket>>,
    steered: Vec<bool>,
    queues: Vec<VecDeque<Turn>>,
    /// Join order matters: first joined, first seated.
    players: Vec<(Ticket, Player)>,
    herbarium: VecDeque<Pressed>,
    wins: HashMap<String, u32>,
    seeds: Rng,
    tickets: Rng,
    running: bool,
}

/// The Arena.
#[derive(Debug)]
pub struct Arena {
    inner: Mutex<Inner>,
    beating: AtomicBool,
}

fn clean_name(s: &str) -> Result<String, ArenaError> {
    let name: String = s
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(16)
        .collect();
    if name.is_empty() {
        Err(ArenaError::Name)
    } else {
        Ok(name)
    }
}

impl Inner {
    /// Plant the next round with whoever is waiting.
    fn plant(&mut self) -> Result<Event, ArenaError> {
        self.no = RoundNo(self.no.0 + 1);
        let mut seats = Vec::with_capacity(SEATS);
        let mut views = Vec::with_capacity(SEATS);
        let mut holders = Vec::with_capacity(SEATS);
        for (ticket, p) in self.players.iter().take(SEATS) {
            seats.push(Seat::Human);
            views.push(SeatView {
                name: p.name.clone(),
                kind: "human".into(),
                temper: None,
                hue: p.hue,
            });
            holders.push(Some(*ticket));
        }
        let shift = usize::try_from(self.no.0 % 6).unwrap_or(0);
        let mut k = 0;
        while seats.len() < SEATS {
            let (name, temper, hue) = CAST[(k + shift) % CAST.len()];
            seats.push(Seat::Gardener { temper });
            views.push(SeatView {
                name: name.into(),
                kind: "gardener".into(),
                temper: Some(temper),
                hue,
            });
            holders.push(None);
            k += 1;
        }
        let seed = self.seeds.next_u32();
        self.round = Round::new(RADIUS, seed, &seats)?;
        self.seats = views;
        self.holders = holders;
        self.steered = vec![false; SEATS];
        self.queues = vec![VecDeque::new(); SEATS];
        self.phase = Phase::Sprouting;
        self.beats = SPROUT_BEATS;
        Ok(Event::Round {
            round: self.no,
            seed,
            radius: RADIUS,
            seats: self.seats.clone(),
            starts: self
                .round
                .garden()
                .vines()
                .iter()
                .map(|v| (v.head, v.heading))
                .collect(),
            beats: self.beats,
        })
    }

    fn press(&mut self) -> Event {
        let outcome = self.round.outcome();
        let name = match outcome {
            Outcome::Won { by } => self.seats.get(by.ix()).map(|s| s.name.clone()),
            _ => None,
        };
        if let Some(n) = name {
            *self.wins.entry(n).or_insert(0) += 1;
        }
        // People who held a seat and never steered lose it after a while.
        for (k, holder) in self.holders.iter().enumerate() {
            if let Some(t) = holder
                && let Some((_, p)) = self.players.iter_mut().find(|(x, _)| x == t)
            {
                p.idle = if self.steered[k] { 0 } else { p.idle + 1 };
            }
        }
        self.players.retain(|(_, p)| p.idle < IDLE_ROUNDS);
        self.herbarium.push_front(Pressed {
            round: self.no,
            pressing: self.round.press(),
            seats: self.seats.clone(),
            outcome,
        });
        self.herbarium.truncate(HERBARIUM);
        self.phase = Phase::Pressed;
        self.beats = PRESSED_BEATS;
        Event::Pressed {
            round: self.no,
            outcome,
            ticks: self.round.garden().tick(),
            lengths: self
                .round
                .garden()
                .vines()
                .iter()
                .map(|v| u32::try_from(v.stem.len()).unwrap_or(u32::MAX))
                .collect(),
        }
    }
}

impl Arena {
    /// A new Arena whose round seeds come from `seed`. The first round is
    /// planted at once and starts sprouting.
    pub fn new(seed: u64) -> Result<Self, ArenaError> {
        let lo = u32::try_from(seed & 0xFFFF_FFFF).unwrap_or(0);
        let hi = u32::try_from(seed >> 32).unwrap_or(0);
        let mut inner = Inner {
            no: RoundNo(0),
            phase: Phase::Sprouting,
            beats: SPROUT_BEATS,
            round: Round::new(RADIUS, lo, &trellis::round::gardeners(SEATS))?,
            seats: Vec::new(),
            holders: Vec::new(),
            steered: Vec::new(),
            queues: Vec::new(),
            players: Vec::new(),
            herbarium: VecDeque::new(),
            wins: HashMap::new(),
            seeds: Rng::new(lo),
            tickets: Rng::new(hi ^ 0xA5A5_5A5A),
            running: true,
        };
        inner.plant()?;
        Ok(Self {
            inner: Mutex::new(inner),
            beating: AtomicBool::new(false),
        })
    }

    /// Claim the right to run this Arena's beat loop. True exactly once.
    pub fn claim_beat(&self) -> bool {
        !self.beating.swap(true, Ordering::SeqCst)
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A panic while holding the lock leaves a garden that is still
        // internally consistent (every mutation is a whole tick), so a
        // poisoned lock is safe to keep using.
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Advance one beat. Returns what to publish.
    pub fn beat(&self) -> Result<Vec<Event>, ArenaError> {
        let mut g = self.lock();
        if !g.running {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        match g.phase {
            Phase::Sprouting => {
                g.beats = g.beats.saturating_sub(1);
                if g.beats == 0 {
                    g.phase = Phase::Growing;
                }
            }
            Phase::Growing => {
                let mut human = Vec::new();
                for k in 0..SEATS {
                    if g.holders.get(k).copied().flatten().is_some()
                        && let Some(t) = g.queues[k].pop_front()
                    {
                        human.push((VineId(u8::try_from(k).unwrap_or(u8::MAX)), t));
                    }
                }
                let tick = g.round.tick(&human)?;
                out.push(Event::Tick {
                    round: g.no,
                    tick: tick.tick,
                    grew: tick.grew,
                    wilted: tick.wilted,
                });
                if g.round.garden().over() {
                    out.push(g.press());
                }
            }
            Phase::Pressed => {
                g.beats = g.beats.saturating_sub(1);
                if g.beats == 0 {
                    out.push(g.plant()?);
                }
            }
        }
        drop(g);
        Ok(out)
    }

    /// Join the queue for a seat. Returns the ticket.
    pub fn join(&self, name: &str, hue: u32) -> Result<(Ticket, Event), ArenaError> {
        let name = clean_name(name)?;
        let mut g = self.lock();
        let ticket =
            Ticket((u64::from(g.tickets.next_u32()) << 32) | u64::from(g.tickets.next_u32()));
        let hue = hue % 360;
        g.players.push((
            ticket,
            Player {
                name: name.clone(),
                hue,
                idle: 0,
            },
        ));
        g.running = true;
        drop(g);
        Ok((ticket, Event::Joined { name, hue }))
    }

    /// Leave.
    pub fn leave(&self, t: Ticket) -> Result<(), ArenaError> {
        let mut g = self.lock();
        let before = g.players.len();
        g.players.retain(|(x, _)| *x != t);
        let gone = g.players.len() == before;
        drop(g);
        if gone {
            return Err(ArenaError::UnknownTicket);
        }
        Ok(())
    }

    /// Queue a turn for the ticket's vine.
    pub fn steer(&self, t: Ticket, turn: Turn) -> Result<VineId, ArenaError> {
        let mut g = self.lock();
        if !g.players.iter().any(|(x, _)| *x == t) {
            return Err(ArenaError::UnknownTicket);
        }
        let k = g
            .holders
            .iter()
            .position(|h| *h == Some(t))
            .ok_or(ArenaError::NotGrowing)?;
        let v = VineId(u8::try_from(k).unwrap_or(u8::MAX));
        if g.phase == Phase::Pressed || !g.round.garden().vine(v).alive() {
            return Err(ArenaError::NotGrowing);
        }
        if g.queues[k].len() >= QUEUE {
            return Err(ArenaError::QueueFull);
        }
        g.queues[k].push_back(turn);
        g.steered[k] = true;
        drop(g);
        Ok(v)
    }

    /// Which seat a ticket holds this round, if any.
    pub fn seat_of(&self, t: Ticket) -> Option<VineId> {
        let g = self.lock();
        g.holders
            .iter()
            .position(|h| *h == Some(t))
            .and_then(|k| u8::try_from(k).ok())
            .map(VineId)
    }

    /// Pause or resume rounds.
    pub fn set_running(&self, on: bool) {
        self.lock().running = on;
    }

    /// The whole Arena.
    pub fn snapshot(&self) -> Snapshot {
        let g = self.lock();
        let mut wins: Vec<(String, u32)> = g.wins.iter().map(|(k, v)| (k.clone(), *v)).collect();
        wins.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        Snapshot {
            round: g.no,
            phase: g.phase,
            beats: g.beats,
            seed: g.round.seed(),
            radius: RADIUS,
            seats: g.seats.clone(),
            tick: g.round.garden().tick(),
            vines: g.round.garden().vines().to_vec(),
            outcome: g.round.outcome(),
            players: g.players.len(),
            pressed: g.herbarium.len(),
            running: g.running,
            wins,
        }
    }

    /// Every pressing, newest first.
    pub fn herbarium(&self) -> Vec<Pressed> {
        self.lock().herbarium.iter().cloned().collect()
    }

    /// One pressing.
    pub fn pressing(&self, round: RoundNo) -> Option<Pressed> {
        self.lock()
            .herbarium
            .iter()
            .find(|p| p.round == round)
            .cloned()
    }
}

/// Words that name how a vine wilted.
pub fn epitaph(seats: &[SeatView], who: VineId, d: Death) -> String {
    let name = |v: VineId| {
        seats
            .get(v.ix())
            .map_or_else(|| format!("vine {}", v.0), |s| s.name.clone())
    };
    match d {
        Death::Wall => format!("{} grew off the wall", name(who)),
        Death::Stem { into } if into == who => format!("{} strangled itself", name(who)),
        Death::Stem { into } => format!("{} ran into {}", name(who), name(into)),
        Death::Clash { with } => {
            format!("{} and {} reached for the same cell", name(who), name(with))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_until(a: &Arena, mut f: impl FnMut(&Event) -> bool, max: usize) -> bool {
        for _ in 0..max {
            for e in a.beat().unwrap() {
                if f(&e) {
                    return true;
                }
            }
        }
        false
    }

    #[test]
    fn rounds_cycle_on_their_own() {
        let a = Arena::new(42).unwrap();
        assert_eq!(a.snapshot().round, RoundNo(1));
        let mut pressed = 0;
        assert!(run_until(
            &a,
            |e| {
                if matches!(e, Event::Pressed { .. }) {
                    pressed += 1;
                }
                pressed == 3
            },
            5_000
        ));
        assert_eq!(a.herbarium().len(), 3);
        // Every pressing regrows to the outcome it was pressed with.
        for p in a.herbarium() {
            let again = p.pressing.regrow().unwrap();
            assert_eq!(again.outcome(), p.outcome);
        }
    }

    #[test]
    fn a_person_is_seated_next_round_and_can_steer() {
        let a = Arena::new(7).unwrap();
        let (t, _) = a.join("Mark", 12).unwrap();
        assert_eq!(a.seat_of(t), None);
        assert_eq!(a.steer(t, Turn::Left), Err(ArenaError::NotGrowing));
        assert!(run_until(&a, |e| matches!(e, Event::Round { .. }), 5_000));
        assert_eq!(a.seat_of(t), Some(VineId(0)));
        let s = a.snapshot();
        assert_eq!(s.seats[0].name, "Mark");
        assert_eq!(s.seats[0].kind, "human");
        assert_eq!(a.steer(t, Turn::Left), Ok(VineId(0)));
        a.steer(t, Turn::Left).unwrap();
        a.steer(t, Turn::Left).unwrap();
        assert_eq!(a.steer(t, Turn::Left), Err(ArenaError::QueueFull));
        assert_eq!(
            a.steer(Ticket(1), Turn::Left),
            Err(ArenaError::UnknownTicket)
        );
    }

    #[test]
    fn idle_people_lose_their_seat() {
        let a = Arena::new(9).unwrap();
        let (t, _) = a.join("Idle", 0).unwrap();
        let mut rounds = 0;
        run_until(
            &a,
            |e| {
                if matches!(e, Event::Round { .. }) {
                    rounds += 1;
                }
                rounds == IDLE_ROUNDS + 1
            },
            20_000,
        );
        assert_eq!(a.seat_of(t), None);
        assert_eq!(a.snapshot().players, 0);
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  Mark "), Ok("Mark".into()));
        assert_eq!(clean_name(""), Err(ArenaError::Name));
        assert_eq!(clean_name("abcdefghijklmnopqrstuvwxyz").unwrap().len(), 16);
    }

    #[test]
    fn tickets_round_trip_as_hex() {
        let t = Ticket(0xDEAD_BEEF_0123_4567);
        assert_eq!(Ticket::parse(&t.hex()), Some(t));
        assert_eq!(Ticket::parse("zz"), None);
    }
}
