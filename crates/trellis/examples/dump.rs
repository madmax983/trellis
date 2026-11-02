//! Dump rounds as JSON lines for the browser port's parity check.
//!
//!   cargo run --release -p trellis --example dump -- <seed>...
//!
//! Each seed plays two rounds: six gardeners on radius 16, and a human in seat
//! 0 on radius 12 whose turns come from `Rng::new(seed)` (the human's script
//! the port replays from the same generator).

use serde_json::json;
use trellis::garden::VineId;
use trellis::hex::Turn;
use trellis::rng::{Rng, seed_of};
use trellis::round::{Round, Seat, gardeners};

fn line(kind: &str, r: &Round) -> serde_json::Value {
    json!({
        "kind": kind,
        "seed": r.seed(),
        "ticks": r.garden().tick(),
        "outcome": r.outcome(),
        "vines": r.garden().vines(),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for word in std::env::args().skip(1) {
        let seed = seed_of(&word);
        let mut r = Round::new(16, seed, &gardeners(6))?;
        r.play_out()?;
        println!("{}", line("gardeners", &r));

        let mut seats = gardeners(5);
        seats.insert(0, Seat::Human);
        let mut r = Round::new(12, seed, &seats)?;
        let mut script = Rng::new(seed);
        while !r.garden().over() {
            let t = match script.below(5) {
                0 => Turn::Left,
                1 => Turn::Right,
                _ => Turn::Straight,
            };
            r.tick(&[(VineId(0), t)])?;
        }
        println!("{}", line("human", &r));
    }
    Ok(())
}
