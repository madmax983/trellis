# ADR 0001 — The Arena lives in memory, and a pressing is its causes

Date: 2026-11-02 · Status: accepted

## Context

Trellis is one shared, always-running game. Its state is a 817-cell owner
table, six stems, a queue of people, and a herbarium of finished rounds. Rounds
last seconds. Nothing about a round in progress is worth surviving a restart:
the next one is planted within three seconds of boot.

The herbarium is the only thing people might want back, and a finished round
is fully determined by its seed, its seats and the humans' non-straight turns:
the gardeners are deterministic integer searches over a fixed visit order.

## Decision

1. The Arena is an `AppState` extension behind one `Mutex`. No database, no
   Docker, no job queue. `verify.sh` has no services gate to bring up.
2. Time enters through one `tokio::time::interval` beat loop that calls
   `Arena::beat()`. The Arena itself never reads a clock, so the same code runs
   under the wall clock and under Autumn's `#[sim_test]` virtual clock.
3. A pressing stores `(radius, seed, seats, keystrokes, ticks)` — tens of bytes
   for an all-gardener round — and every plate in the herbarium is regrown from
   it on each request. Regrowing is the test that the record is complete.
4. Live events carry one small message per tick (`grew`, `wilted`); a browser
   that sees a gap in tick numbers refetches `/_arena.json`.

## Consequences

- A restart empties the herbarium. If that ever matters, persisting pressings
  is one table of JSON blobs — the record is already the minimal one.
- One process, one Arena: horizontal scaling would need the beat loop on one
  owner and the channel backend on Redis. Not needed for a toy.
- The browser engine (`engine.js`) must stay bit-identical to the Rust engine
  for the potting shed to mean anything; `tools/parity.js` guards that.
