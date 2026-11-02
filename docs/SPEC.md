# Trellis — specification

## 1. The trellis

A hexagon of hexagons of radius `R` (3 ≤ R ≤ 24) in axial coordinates: the
cells are every `(q, r)` with `max(|q|, |r|, |q + r|) ≤ R`, numbered row by row
(`r` ascending, then `q` ascending). There are `3R(R+1) + 1` of them. The
Arena uses `R = 16`: 817 cells.

Six headings, counter-clockwise from east:
`E (1,0), NE (1,−1), NW (0,−1), W (−1,0), SW (−1,1), SE (0,1)`.
`opp(d) = (d + 3) mod 6`, and `DIRS[opp(d)] = −DIRS[d]`.

A turn is `left (+1)`, `straight (0)` or `right (+5)`, applied mod 6.

## 2. One tick

State: `owner[c] ∈ {0} ∪ {k+1}` per cell; per vine `k` a head, a heading, a
stem (the cells it holds, in order) and an optional `(tick, death)`.

For every growing vine `k`, simultaneously:

1. `h'ₖ = hₖ + turnₖ (mod 6)`;
2. `tₖ = step(headₖ, h'ₖ)` or *off*;
3. vine `k` **wilts** if `tₖ` is off (`wall`), or `owner[tₖ] ≠ 0` (`stem`, naming
   the owner — possibly itself), or some other growing vine `j` has `tⱼ = tₖ`
   (`clash`, naming the lowest such `j`). Fates are decided against the board
   *before* any write;
4. every vine that did not wilt sets `owner[tₖ] = k + 1`, `headₖ = tₖ`,
   `headingₖ = h'ₖ` and appends `tₖ` to its stem.

Two adjacent heads facing each other both hit a stem (each other's head); two
heads one cell apart facing each other clash over the middle and both wilt.

**Outcome.** With one vine: over when it wilts. With two or more: `won by k`
when exactly `k` is growing, `draw` when none is, otherwise growing. A finished
garden does not step.

## 3. The gardeners

For a growing vine `me` and each turn in the order `straight, left, right` whose
target is on the wall and empty, a gardener computes, over the board as it
stands, treating the target as held:

- `reach`: cells reachable from the target through empty cells, counting the
  target;
- `contact`: whether any rival's head borders that region;
- territory: a multi-source BFS from the target (for `me`) and every rival's
  head, in seat order, visiting neighbours in heading order; a cell first
  reached at distance `d` belongs to that source, and a second arrival at the
  same `d` from a different source makes it a tie (owned by nobody, and so are
  cells first reached from it);
- `mine`, `theirs` (sum of rivals), and `nearest`/`nearest_theirs` for the
  rival head nearest by hex distance (lowest seat on ties);
- `walls`: neighbours of the target that are off or held;
- `danger`: rivals whose head neighbours the target (a clash risk).

Score (integers): a turn whose reach is under half the best reach scores
`−1 000 000` extra (a trap). Without contact, every temper packs:
`100·reach + 7·walls − 5000·danger`. With contact:

| temper | score |
|---|---|
| ivy | `20·mine − 4·theirs + reach − 5000·danger` |
| kudzu | `10·mine − 12·nearest_theirs − 30·nearest − 2000·danger` |
| moss | `20·reach + 15·walls + 4·mine − 5000·danger` |
| hops | `6·reach + 6·mine + roll − 3000·danger`, `roll ∈ 0..600` |

The highest score wins; ties go to the earlier turn in the order above. With no
safe turn, go straight (and wilt). Hops draws exactly one `roll` from the
round's die per safe turn; no other temper draws.

## 4. Rounds and pressings

A round is `(radius, seed, seats)`. Planting draws from mulberry32(`seed`): a
ring offset, then per vine one lean (`left` with probability ⅓, else
`straight`). Vines sit evenly on the ring of radius `⌊2R/3⌋`, heading along the
ring's counter-clockwise walk, leaned. The gardeners share one die,
mulberry32(`seed ⊕ 0x9E3779B9`), drawn in seat order each tick.

A **pressing** is `(radius, seed, seats, keystrokes, ticks)` where keystrokes
are the humans' non-straight turns tagged with the tick they produced.
`regrow(press(round)) = round`, stem for stem.

The Arena cycles `sprouting (20 beats) → growing (1 tick per beat) →
pressed (32 beats) → next round`, one beat every 125 ms. People join a queue;
the first six are seated at the next planting; a seated person who sends no
steer for two rounds loses the place.

## 5. What must be true

### Formally (verus/src/trellis.rs)

| # | obligation | confidence it verifies as written |
|---|---|---|
| 1 | `lemma_distinct_targets`: two survivors never share a target | high — immediate from the definition |
| 2–4 | `lemma_apply`: stems are forever; survivors land as `k+1`; untouched cells unchanged | medium — the induction is right; the nested-quantifier triggers in the fourth clause are the likeliest thing to need a hint |
| 5 | `lemma_fills` + `lemma_filled_update`: a tick fills exactly `survivors` cells | medium-high — depends on Z3 unfolding `apply` one level |
| 5′ | `lemma_tick_progress`: something survives ⇒ strictly fuller, and never over-full | high given 5 |
| 6 | `lemma_game_bound`: a strictly-filling run has at most `len` steps | medium-high — `boards[(m−1)+1]` must match the trigger `boards[i+1]` |
| 7 | `lemma_no_reversal` | high — six cases of constant `mod` |
| 8 | `lemma_there_and_back`, `lemma_step_distance` | medium — `lemma_step_distance` asks Z3 to case-split three `abs` and a six-way `dq`/`dr`; it may want an explicit split on `d` |

Not modelled: the neighbour table's agreement with `DIRS` (property-tested),
the gardeners (they are heuristics: what matters is that they are
deterministic, which the parity check and the regrow laws test), and the Arena.

### Observably (tests)

- every tick keeps the laws on arbitrary plantings and turn scripts
  (`crates/trellis/tests/laws.rs`, 400 cases);
- any table of gardeners finishes within `cells` ticks and regrows exactly;
- arbitrary human keystrokes survive a JSON round trip and regrow exactly;
- the browser engine makes the same choice as the Rust engine on every tick
  (`tools/parity.js`, run by `verify.sh`);
- the Arena keeps planting and pressing on Autumn's simulated clock
  (`crates/trellis-app/tests/sim_arena.rs`);
- the HTTP surface: pages, snapshot, join/steer/leave refusals (`tests/http.rs`)
  and, in `verify.sh`, the CSRF layer end to end.
