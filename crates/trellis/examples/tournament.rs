//! Play many all-gardener rounds and tally who wins.
//!
//!   cargo run --release -p trellis --example tournament -- [rounds] [radius] [vines]

use trellis::garden::Outcome;
use trellis::round::{Round, Seat, gardeners};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rounds: u32 = args.first().map_or(Ok(200), |s| s.parse())?;
    let radius: u8 = args.get(1).map_or(Ok(16), |s| s.parse())?;
    let vines: usize = args.get(2).map_or(Ok(6), |s| s.parse())?;
    let seats = gardeners(vines);
    let mut wins = vec![0_u32; vines];
    let mut draws = 0;
    let mut ticks = Vec::new();
    for seed in 0..rounds {
        let mut r = Round::new(radius, seed, &seats)?;
        match r.play_out()? {
            Outcome::Won { by } => wins[by.ix()] += 1,
            _ => draws += 1,
        }
        ticks.push(r.garden().tick());
    }
    ticks.sort_unstable();
    for (k, seat) in seats.iter().enumerate() {
        if let Seat::Gardener { temper } = seat {
            println!("seat {k} {:<6} {:>4} wins", temper.name(), wins[k]);
        }
    }
    println!("draws {draws}");
    println!(
        "ticks: min {} median {} max {} (board {} cells)",
        ticks[0],
        ticks[ticks.len() / 2],
        ticks[ticks.len() - 1],
        3 * usize::from(radius) * (usize::from(radius) + 1) + 1
    );
    Ok(())
}
