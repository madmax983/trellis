# Trellis — notes for agents

A hexagonal light-cycle game played by vines, on autumn-web 0.9. See README.md.

## Layout
- `crates/trellis` — pure engine (no IO/async/clock). Change rules here first.
- `crates/trellis-app` — Arena state machine, routes, Maud views, and the
  browser assets (`engine.js`, `render.js`, `trellis.js`, `trellis.css`),
  served via `include_str!`.
- `verus/src/trellis.rs` — standalone Verus model; keep it in sync with
  `Garden::step` (see docs/SPEC.md §2 and §5).
- `tools/parity.js` — the browser engine must match Rust exactly.

## Rules of the road
- Any change to `garden.rs`, `gardener.rs`, `round.rs`, `hex.rs` or `rng.rs`
  must be mirrored in `engine.js`, then run
  `cargo run -q --release -p trellis --example dump -- ivy moss > /tmp/d && node tools/parity.js /tmp/d`.
- The Arena never reads a clock; time is the beat loop in `lib.rs`. Keep it
  that way so `tests/sim_arena.rs` stays meaningful.
- Autumn's CSP is `script-src 'self'`: no inline scripts; wire via `data-`.
- No `unwrap()`/`expect()` outside tests.
- `./verify.sh` is the gate; it needs node for the parity step and Verus.
