//! The Arena on Autumn's simulated clock (`#[sim_test]`).
//!
//! The beat loop is ordinary `tokio::time::interval` code. Under the sim it
//! runs on a paused runtime, so `sim.advance` plays ten virtual minutes of
//! rounds (dozens of them) in a moment, through the real loop, the real
//! live-events publish and the real HTTP surface.
//!
//! ```text
//! cargo test -p trellis-app --test sim_arena
//! AUTUMN_SIM_SEED=0x9f3a cargo test -p trellis-app --test sim_arena
//! ```

use std::sync::{Arc, Mutex};
use std::time::Duration;

use autumn_web::sim::{Sim, assert_all_sometimes_satisfied};
use autumn_web::test::TestApp;
use autumn_web::{AppState, always, sim_test, sometimes};

#[sim_test]
async fn rounds_keep_finishing_on_virtual_time(mut sim: Sim) {
    let seed = sim.rng().next_u64();
    let held: Arc<Mutex<Option<AppState>>> = Arc::new(Mutex::new(None));
    let keep = Arc::clone(&held);
    sim.build(
        TestApp::new()
            .routes(trellis_app::all_routes!())
            .state_initializer(move |state| {
                trellis_app::install(state, seed).expect("the Arena plants");
                *keep.lock().expect("not poisoned") = Some(state.clone());
            }),
    );
    let state = held
        .lock()
        .expect("not poisoned")
        .clone()
        .expect("the initializer ran");
    always!(
        trellis_app::start_beating(&state),
        "the first start claims the beat (seed={:#x})",
        sim.seed
    );
    always!(
        !trellis_app::start_beating(&state),
        "a second start is refused: one loop per Arena (seed={:#x})",
        sim.seed
    );

    let client = sim.client();
    let snap = |r: autumn_web::test::TestResponse| -> serde_json::Value {
        serde_json::from_str(&r.text()).expect("json")
    };
    let first = snap(client.get("/_arena.json").send().await);
    always!(first["round"] == 1, "the Arena starts at round 1");

    let mut last_round = 1;
    let mut last_pressed = 0;
    for minute in 1..=10 {
        // `tokio::time::advance` jumps the paused clock in one step, and an
        // interval fires once per jump, so walk the minute beat by beat — the
        // way the wall clock would deliver it.
        let beats = Duration::from_mins(1).as_millis() / trellis_app::BEAT.as_millis();
        for _ in 0..beats {
            sim.advance(trellis_app::BEAT).await;
        }
        let s = snap(client.get("/_arena.json").send().await);
        let round = s["round"].as_u64().expect("round");
        let pressed = s["pressed"].as_u64().expect("pressed");
        always!(
            round >= last_round,
            "round numbers never go back: minute {minute}, round {round} (seed={:#x})",
            sim.seed
        );
        sometimes!(
            round > last_round + 2,
            "a virtual minute planted several rounds"
        );
        always!(
            pressed >= last_pressed.min(36),
            "the herbarium never loses pressings it has room for"
        );
        always!(
            s["tick"].as_u64().expect("tick") <= 817,
            "no round runs longer than the wall has cells"
        );
        last_round = round;
        last_pressed = pressed;
    }

    // A round is at most 817 ticks plus 52 beats of ceremony, under 109 s,
    // so ten minutes must have planted at least five more.
    always!(
        last_round >= 6,
        "ten virtual minutes planted only {last_round} rounds (seed={:#x})",
        sim.seed
    );

    // Every pressing regrows to the outcome it was pressed with, and the
    // herbarium page renders every plate.
    let h = client.get("/herbarium").send().await;
    h.assert_ok();
    let a = trellis_app::arena(&state).expect("installed");
    let pressed = a.herbarium();
    always!(!pressed.is_empty(), "ten minutes press something");
    for p in &pressed {
        let again = p.pressing.regrow().expect("regrows");
        always!(
            again.outcome() == p.outcome,
            "pressing {} regrew to a different outcome (seed={:#x})",
            p.round.0,
            sim.seed
        );
        sometimes!(
            matches!(p.outcome, trellis::garden::Outcome::Won { .. }),
            "some round was won outright"
        );
        let page = client.get(&format!("/pressing/{}", p.round.0)).send().await;
        page.assert_ok();
    }
    assert_all_sometimes_satisfied();
}
