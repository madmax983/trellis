// Trellis — the verified core.
//
//   verus verus/src/trellis.rs
//
// A pure model of one tick of `Garden::step` (crates/trellis/src/garden.rs),
// and what makes the game a game rather than a bug:
//
//   1. distinct writes   — two survivors never reach for the same cell;
//   2. stems are forever — every cell held before a tick is held by the same
//                          vine after it;
//   3. survivors land    — each surviving vine k holds its target afterwards,
//                          as k + 1;
//   4. nothing else moves — a cell no survivor reached for is unchanged;
//   5. the wall fills    — a tick in which s vines survive fills exactly s
//                          more cells;
//   6. the game ends     — any run of ticks in each of which something grows
//                          is no longer than the wall has cells;
//   7. no reversal       — a turned heading is never the opposite heading;
//   8. the trellis       — stepping across heading d then across opp(d)
//                          returns to the start, and moves hex distance one.
//
// This file is standalone: no dependency on the `trellis` crate and no IO.
//
// ── The modelling assumptions ───────────────────────────────────────────────
//
// * The board is `owner: Seq<int>`: 0 empty, k + 1 held by vine k —
//   `Garden::owner` exactly.
// * `target[k]` is vine k's reach this tick, any int outside 0..len for "off
//   the wall" (`None` at runtime). Dead vines are filtered by `alive`.
// * Fates are decided against the board *before* any write, then survivors
//   write in seat order: `apply` below is that loop, seat by seat. This is the
//   two-loop structure of `Garden::step`.
// * The neighbour table's correspondence to `DIRS` is property-tested in
//   crates/trellis (hex::tests::neighbours_are_symmetric), not modelled; the
//   arithmetic it rests on is lemma 8.

use vstd::prelude::*;

verus! {

// ── One tick ────────────────────────────────────────────────────────────────

/// Vine k survives the tick: it is growing, it reaches for a cell on the
/// wall, that cell is empty, and no other growing vine reaches for it.
pub open spec fn survives(owner: Seq<int>, alive: Seq<bool>, target: Seq<int>, k: int) -> bool {
    &&& 0 <= k < alive.len()
    &&& alive[k]
    &&& 0 <= target[k] < owner.len()
    &&& owner[target[k]] == 0
    &&& forall|j: int| 0 <= j < alive.len() && j != k && alive[j] ==> target[j] != target[k]
}

/// The board after survivors 0..k have written.
pub open spec fn apply(owner: Seq<int>, alive: Seq<bool>, target: Seq<int>, k: nat) -> Seq<int>
    decreases k,
{
    if k == 0 {
        owner
    } else {
        let prev = apply(owner, alive, target, (k - 1) as nat);
        if survives(owner, alive, target, k - 1) {
            prev.update(target[k - 1], k as int)
        } else {
            prev
        }
    }
}

/// How many of vines 0..k survive.
pub open spec fn survivors(owner: Seq<int>, alive: Seq<bool>, target: Seq<int>, k: nat) -> nat
    decreases k,
{
    if k == 0 {
        0
    } else {
        survivors(owner, alive, target, (k - 1) as nat) + if survives(owner, alive, target, k - 1) {
            1nat
        } else {
            0nat
        }
    }
}

/// Held cells among the first i.
pub open spec fn filled(s: Seq<int>, i: nat) -> nat
    decreases i,
{
    if i == 0 {
        0
    } else {
        filled(s, (i - 1) as nat) + if s[i - 1] != 0 {
            1nat
        } else {
            0nat
        }
    }
}

/// The well-formedness every tick starts from: one alive flag and one target
/// per vine, and every held cell names a vine.
pub open spec fn well_formed(owner: Seq<int>, alive: Seq<bool>, target: Seq<int>) -> bool {
    &&& alive.len() == target.len()
    &&& forall|c: int| 0 <= c < owner.len() ==> owner[c] >= 0
}

// ── 1. distinct writes ──────────────────────────────────────────────────────

pub proof fn lemma_distinct_targets(
    owner: Seq<int>,
    alive: Seq<bool>,
    target: Seq<int>,
    j: int,
    k: int,
)
    requires
        survives(owner, alive, target, j),
        survives(owner, alive, target, k),
        j != k,
    ensures
        target[j] != target[k],
{
}

// ── 2–4. what a tick writes ─────────────────────────────────────────────────

pub proof fn lemma_apply(owner: Seq<int>, alive: Seq<bool>, target: Seq<int>, k: nat)
    requires
        well_formed(owner, alive, target),
        k <= alive.len(),
    ensures
        apply(owner, alive, target, k).len() == owner.len(),
        // 2. stems are forever
        forall|c: int|
            0 <= c < owner.len() && owner[c] != 0 ==> #[trigger] apply(owner, alive, target, k)[c]
                == owner[c],
        // 3. survivors land
        forall|j: int|
            0 <= j < k && #[trigger] survives(owner, alive, target, j) ==> apply(
                owner,
                alive,
                target,
                k,
            )[target[j]] == j + 1,
        // 4. nothing else moves
        forall|c: int|
            0 <= c < owner.len() && (forall|j: int|
                0 <= j < k && #[trigger] survives(owner, alive, target, j) ==> target[j] != c)
                ==> apply(owner, alive, target, k)[c] == owner[c],
    decreases k,
{
    if k > 0 {
        let km = (k - 1) as nat;
        lemma_apply(owner, alive, target, km);
        let prev = apply(owner, alive, target, km);
        let cur = apply(owner, alive, target, k);
        if survives(owner, alive, target, k - 1) {
            let t = target[k - 1];
            assert(cur == prev.update(t, k as int));
            assert forall|c: int| 0 <= c < owner.len() && owner[c] != 0 implies cur[c]
                == owner[c] by {
                assert(c != t);
            }
            assert forall|j: int|
                0 <= j < k && #[trigger] survives(owner, alive, target, j) implies cur[target[j]]
                == j + 1 by {
                if j < k - 1 {
                    lemma_distinct_targets(owner, alive, target, j, k - 1);
                    assert(target[j] != t);
                }
            }
            assert forall|c: int|
                0 <= c < owner.len() && (forall|j: int|
                    0 <= j < k && #[trigger] survives(owner, alive, target, j) ==> target[j]
                        != c) implies cur[c] == owner[c] by {
                assert(survives(owner, alive, target, k - 1));
                assert(t != c);
                assert forall|j: int|
                    0 <= j < km && #[trigger] survives(owner, alive, target, j) implies target[j]
                    != c by {}
            }
        } else {
            assert(cur == prev);
            assert forall|c: int|
                0 <= c < owner.len() && (forall|j: int|
                    0 <= j < k && #[trigger] survives(owner, alive, target, j) ==> target[j]
                        != c) implies cur[c] == owner[c] by {
                assert forall|j: int|
                    0 <= j < km && #[trigger] survives(owner, alive, target, j) implies target[j]
                    != c by {}
            }
        }
    }
}

// ── 5. the wall fills ───────────────────────────────────────────────────────

/// Writing a non-zero value into an empty cell adds one to the count of every
/// prefix that contains it.
pub proof fn lemma_filled_update(s: Seq<int>, t: int, v: int, i: nat)
    requires
        0 <= t < s.len(),
        s[t] == 0,
        v != 0,
        i <= s.len(),
    ensures
        filled(s.update(t, v), i) == filled(s, i) + if t < i {
            1nat
        } else {
            0nat
        },
    decreases i,
{
    if i > 0 {
        lemma_filled_update(s, t, v, (i - 1) as nat);
    }
}

pub proof fn lemma_filled_bound(s: Seq<int>, i: nat)
    requires
        i <= s.len(),
    ensures
        filled(s, i) <= i,
    decreases i,
{
    if i > 0 {
        lemma_filled_bound(s, (i - 1) as nat);
    }
}

pub proof fn lemma_fills(owner: Seq<int>, alive: Seq<bool>, target: Seq<int>, k: nat)
    requires
        well_formed(owner, alive, target),
        k <= alive.len(),
    ensures
        filled(apply(owner, alive, target, k), owner.len()) == filled(owner, owner.len())
            + survivors(owner, alive, target, k),
    decreases k,
{
    if k > 0 {
        let km = (k - 1) as nat;
        lemma_fills(owner, alive, target, km);
        lemma_apply(owner, alive, target, km);
        let prev = apply(owner, alive, target, km);
        if survives(owner, alive, target, k - 1) {
            let t = target[k - 1];
            // The target was empty before the tick and no earlier survivor
            // reached for it, so it is still empty in `prev`.
            assert forall|j: int|
                0 <= j < km && #[trigger] survives(owner, alive, target, j) implies target[j]
                != t by {
                lemma_distinct_targets(owner, alive, target, j, k - 1);
            }
            assert(prev[t] == owner[t]);
            assert(prev[t] == 0);
            lemma_filled_update(prev, t, k as int, owner.len());
        }
    }
}

/// The headline: if anything survives, the wall strictly fills — and it can
/// never be more than full.
pub proof fn lemma_tick_progress(owner: Seq<int>, alive: Seq<bool>, target: Seq<int>)
    requires
        well_formed(owner, alive, target),
        survivors(owner, alive, target, alive.len() as nat) > 0,
    ensures
        filled(apply(owner, alive, target, alive.len() as nat), owner.len()) > filled(
            owner,
            owner.len(),
        ),
        filled(apply(owner, alive, target, alive.len() as nat), owner.len()) <= owner.len(),
{
    lemma_fills(owner, alive, target, alive.len() as nat);
    lemma_apply(owner, alive, target, alive.len() as nat);
    lemma_filled_bound(apply(owner, alive, target, alive.len() as nat), owner.len());
}

// ── 6. the game ends ────────────────────────────────────────────────────────

/// A run of boards on one wall, each strictly fuller than the last, has at
/// most `len` steps: every round ends within as many ticks as there are cells.
pub proof fn lemma_game_bound(boards: Seq<Seq<int>>, len: nat)
    requires
        boards.len() >= 1,
        forall|i: int| 0 <= i < boards.len() ==> #[trigger] boards[i].len() == len,
        forall|i: int|
            0 <= i < boards.len() - 1 ==> filled(#[trigger] boards[i + 1], len) > filled(
                boards[i],
                len,
            ),
    ensures
        boards.len() - 1 <= len,
{
    lemma_run_fills(boards, len, (boards.len() - 1) as nat);
    lemma_filled_bound(boards[boards.len() - 1], len);
}

proof fn lemma_run_fills(boards: Seq<Seq<int>>, len: nat, m: nat)
    requires
        m < boards.len(),
        forall|i: int| 0 <= i < boards.len() ==> #[trigger] boards[i].len() == len,
        forall|i: int|
            0 <= i < boards.len() - 1 ==> filled(#[trigger] boards[i + 1], len) > filled(
                boards[i],
                len,
            ),
    ensures
        filled(boards[m as int], len) >= m,
    decreases m,
{
    if m > 0 {
        lemma_run_fills(boards, len, (m - 1) as nat);
        assert(filled(boards[(m - 1) + 1], len) > filled(boards[m - 1], len));
    }
}

// ── 7. no reversal ──────────────────────────────────────────────────────────

/// `Turn::delta`: left 1, straight 0, right 5.
pub open spec fn is_turn(t: int) -> bool {
    t == 0 || t == 1 || t == 5
}

pub proof fn lemma_no_reversal(h: int, t: int)
    requires
        0 <= h < 6,
        is_turn(t),
    ensures
        (h + t) % 6 != (h + 3) % 6,
        0 <= (h + t) % 6 < 6,
{
    if h == 0 {
    } else if h == 1 {
    } else if h == 2 {
    } else if h == 3 {
    } else if h == 4 {
    } else {
        assert(h == 5);
    }
}

// ── 8. the trellis ──────────────────────────────────────────────────────────

/// `DIRS` in hex.rs.
pub open spec fn dq(d: int) -> int {
    if d == 0 {
        1
    } else if d == 1 {
        1
    } else if d == 2 {
        0
    } else if d == 3 {
        -1
    } else if d == 4 {
        -1
    } else {
        0
    }
}

pub open spec fn dr(d: int) -> int {
    if d == 0 {
        0
    } else if d == 1 {
        -1
    } else if d == 2 {
        -1
    } else if d == 3 {
        0
    } else if d == 4 {
        1
    } else {
        1
    }
}

pub open spec fn opp(d: int) -> int {
    (d + 3) % 6
}

pub open spec fn abs(x: int) -> int {
    if x < 0 {
        -x
    } else {
        x
    }
}

/// `hex_len`: max(|q|, |r|, |q + r|).
pub open spec fn hex_len(q: int, r: int) -> int {
    let a = abs(q);
    let b = abs(r);
    let c = abs(q + r);
    let m = if a > b {
        a
    } else {
        b
    };
    if m > c {
        m
    } else {
        c
    }
}

pub proof fn lemma_there_and_back(q: int, r: int, d: int)
    requires
        0 <= d < 6,
    ensures
        0 <= opp(d) < 6,
        q + dq(d) + dq(opp(d)) == q,
        r + dr(d) + dr(opp(d)) == r,
        hex_len(dq(d), dr(d)) == 1,
{
    if d == 0 {
        assert(opp(d) == 3);
    } else if d == 1 {
        assert(opp(d) == 4);
    } else if d == 2 {
        assert(opp(d) == 5);
    } else if d == 3 {
        assert(opp(d) == 0);
    } else if d == 4 {
        assert(opp(d) == 1);
    } else {
        assert(opp(d) == 2);
    }
}

/// One step changes the distance from the centre by at most one: a vine on
/// the wall can only leave it from the edge ring.
pub proof fn lemma_step_distance(q: int, r: int, d: int)
    requires
        0 <= d < 6,
    ensures
        hex_len(q + dq(d), r + dr(d)) <= hex_len(q, r) + 1,
        hex_len(q, r) <= hex_len(q + dq(d), r + dr(d)) + 1,
{
}

fn main() {
}

} // verus!
