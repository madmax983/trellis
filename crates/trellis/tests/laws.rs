//! The laws in verus/src/trellis.rs, checked against the running engine with
//! arbitrary plantings and arbitrary turns.

use proptest::prelude::*;
use trellis::garden::{Garden, Outcome, VineId};
use trellis::hex::{Board, Cell, Dir, Turn};
use trellis::round::{Pressing, Round, Seat, gardeners};

fn turn() -> impl Strategy<Value = Turn> {
    prop_oneof![Just(Turn::Left), Just(Turn::Straight), Just(Turn::Right)]
}

/// A random planting on a small trellis: distinct cells, any headings.
fn planting() -> impl Strategy<Value = (u8, Vec<(u16, u8)>)> {
    (3_u8..=8).prop_flat_map(|r| {
        let n = 3 * usize::from(r) * (usize::from(r) + 1) + 1;
        let cells = proptest::sample::subsequence((0..n).collect::<Vec<_>>(), 1..=6);
        (Just(r), cells, proptest::collection::vec(0_u8..6, 6)).prop_map(|(r, cs, ds)| {
            (
                r,
                cs.into_iter()
                    .zip(ds)
                    .map(|(c, d)| (u16::try_from(c).unwrap(), d))
                    .collect(),
            )
        })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    /// Stems are forever, survivors land, the wall fills by exactly the
    /// number of survivors, and the game ends within `cells` ticks.
    #[test]
    fn every_tick_keeps_the_laws(
        (r, plant) in planting(),
        script in proptest::collection::vec(proptest::collection::vec(turn(), 6), 1..400),
    ) {
        let b = Board::new(r).unwrap();
        let starts: Vec<(Cell, Dir)> = plant.iter().map(|&(c, d)| (Cell(c), Dir::new(d).unwrap())).collect();
        let mut g = Garden::plant(b.clone(), &starts).unwrap();
        let n = starts.len();
        let mut ticks_with_growth = 0;
        for turns in &script {
            if g.over() { break; }
            let before = g.owners().to_vec();
            let free_before = g.free();
            let t = g.step(&turns[..n]).unwrap();
            // Stems are forever.
            for (c, &o) in before.iter().enumerate() {
                if o != 0 { prop_assert_eq!(g.owners()[c], o); }
            }
            // Survivors land, on distinct empty cells.
            let mut cells: Vec<Cell> = t.grew.iter().map(|x| x.1).collect();
            for &(v, c) in &t.grew {
                prop_assert_eq!(before[c.ix()], 0);
                prop_assert_eq!(g.owner(c), Some(v));
                prop_assert_eq!(g.vine(v).head, c);
            }
            cells.sort();
            cells.dedup();
            prop_assert_eq!(cells.len(), t.grew.len());
            // The wall fills by exactly the survivors.
            prop_assert_eq!(free_before - g.free(), t.grew.len());
            if !t.grew.is_empty() { ticks_with_growth += 1; }
            // A wilted vine never grows again.
            for &(v, _) in &t.wilted { prop_assert!(!g.vine(v).alive()); }
        }
        prop_assert!(ticks_with_growth <= b.len());
    }

    /// No turn reverses a heading.
    #[test]
    fn no_turn_reverses(d in 0_u8..6, t in turn()) {
        let d = Dir::new(d).unwrap();
        prop_assert_ne!(d.turn(t), d.opposite());
    }

    /// Any table of gardeners plays to an end within the bound, and its
    /// pressing regrows to the same garden.
    #[test]
    fn gardeners_always_finish_and_regrow(seed in any::<u32>(), r in 5_u8..=14, n in 2_usize..=6) {
        let mut round = Round::new(r, seed, &gardeners(n)).unwrap();
        let outcome = round.play_out().unwrap();
        prop_assert_ne!(outcome, Outcome::Growing);
        prop_assert!(round.garden().tick() as usize <= round.garden().board().len());
        let again = round.press().regrow().unwrap();
        prop_assert_eq!(again.garden(), round.garden());
    }

    /// A human's keystrokes, whatever they are, regrow exactly.
    #[test]
    fn keystrokes_regrow(seed in any::<u32>(), script in proptest::collection::vec(turn(), 1..300)) {
        let mut seats = gardeners(3);
        seats.insert(1, Seat::Human);
        let mut round = Round::new(10, seed, &seats).unwrap();
        for t in &script {
            if round.garden().over() { break; }
            round.tick(&[(VineId(1), *t)]).unwrap();
        }
        let p: Pressing = round.press();
        let json = serde_json::to_string(&p).unwrap();
        let back: Pressing = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&back, &p);
        let regrown = back.regrow().unwrap();
        prop_assert_eq!(regrown.garden(), round.garden());
    }
}
