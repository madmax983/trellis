//! # trellis-app
//!
//! The thin app around the `trellis` engine: one Arena in memory whose rounds
//! never stop, every tick over live events, a herbarium of pressed rounds
//! regrown from their seeds, and a potting shed where the engine runs in the
//! browser.
//!
//!   Engine       -> `trellis`: the trellis, the rules, the gardeners.
//!   State        -> [`arena::Arena`], an `AppState` extension. No database:
//!                   see docs/adr/0001-the-arena-lives-in-memory.md.
//!   Live events  -> one topic, `garden`. A round, then one small message per
//!                   tick, then the pressing.
//!   Time         -> one beat loop on `tokio::time`, so Autumn's simulation
//!                   harness drives the Arena on virtual time.

pub mod arena;
pub mod routes;
pub mod view;

use std::hash::{BuildHasher, Hasher};
use std::sync::Arc;
use std::time::Duration;

use autumn_web::AppState;

use crate::arena::{Arena, Event, TOPIC};

/// One beat: one tick of a growing round.
pub const BEAT: Duration = Duration::from_millis(125);

/// Publish one event. A publish with nobody listening is not an error.
pub fn publish(state: &AppState, e: &Event) {
    match serde_json::to_string(e) {
        Ok(json) => {
            if let Err(err) = state.channels().publish(TOPIC, json) {
                tracing::debug!(error = %err, "garden event not delivered");
            }
        }
        Err(err) => tracing::warn!(error = %err, "garden event did not serialise"),
    }
}

/// A seed nobody chose: the standard library's per-process random hasher keys.
pub fn fresh_seed() -> u64 {
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(0x0074_7265_6c6c_6973);
    h.finish()
}

/// Install an Arena seeded with `seed`. Synchronous, so tests can call it from
/// a state initializer.
pub fn install(state: &AppState, seed: u64) -> anyhow::Result<()> {
    state.insert_extension(Arena::new(seed)?);
    Ok(())
}

/// The installed Arena.
pub fn arena(state: &AppState) -> Option<Arc<Arena>> {
    state.extension::<Arena>()
}

/// Start the installed Arena's beat loop, once per Arena. Returns whether this
/// call started it. Boot calls it; a test that
/// mounts the app without `on_startup` calls it when it wants time to pass.
pub fn start_beating(state: &AppState) -> bool {
    match arena(state) {
        Some(a) if a.claim_beat() => {}
        _ => return false,
    }
    let state = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(BEAT);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            let Some(a) = arena(&state) else { continue };
            match a.beat() {
                Ok(events) => {
                    for e in &events {
                        publish(&state, e);
                    }
                }
                Err(e) => tracing::warn!(error = %e, "the Arena missed a beat"),
            }
        }
    });
    true
}

/// Boot: plant the Arena and start the beat.
#[allow(clippy::unused_async)] // `on_startup` takes an async hook; this one has nothing to await
pub async fn boot(state: AppState) -> autumn_web::AutumnResult<()> {
    use autumn_web::error::AutumnError;
    install(&state, fresh_seed())
        .map_err(|e| AutumnError::service_unavailable_msg(e.to_string()))?;
    if std::env::var("TRELLIS_ARENA").is_ok_and(|v| v == "off")
        && let Some(a) = arena(&state)
    {
        a.set_running(false);
    }
    start_beating(&state);
    tracing::info!("the Arena is planted");
    Ok(())
}
