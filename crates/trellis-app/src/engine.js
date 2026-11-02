// Trellis — the engine, ported from crates/trellis bit for bit.
//
// Same cell numbering, same rules, same gardeners, same mulberry32 die. The
// parity check in tools/parity.js plays rounds here and in Rust and compares
// every stem, cell for cell.
(function (root) {
  "use strict";

  const DIRS = [[1, 0], [1, -1], [0, -1], [-1, 0], [-1, 1], [0, 1]];
  const OFF = 0xffff;
  const TURNS = ["straight", "left", "right"]; // the order gardeners weigh them
  const DELTA = { left: 1, straight: 0, right: 5 };
  const TEMPERS = ["ivy", "kudzu", "moss", "hops"];
  const DIE_SALT = 0x9e3779b9;

  function hexLen(q, r) {
    return Math.max(Math.abs(q), Math.abs(r), Math.abs(q + r));
  }

  // ── mulberry32 ────────────────────────────────────────────────────────────
  class Rng {
    constructor(seed) { this.s = seed >>> 0; }
    nextU32() {
      this.s = (this.s + 0x6d2b79f5) >>> 0;
      let t = this.s;
      t = Math.imul(t ^ (t >>> 15), t | 1);
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
      return (t ^ (t >>> 14)) >>> 0;
    }
    below(n) { return n === 0 ? 0 : this.nextU32() % n; }
  }

  function seedOf(word) {
    let h = 0x811c9dc5;
    const bytes = new TextEncoder().encode(String(word).trim().toLowerCase());
    for (const b of bytes) {
      h ^= b;
      h = Math.imul(h, 0x01000193) >>> 0;
    }
    return h >>> 0;
  }

  // ── the trellis ───────────────────────────────────────────────────────────
  class Board {
    constructor(radius) {
      if (radius < 3 || radius > 24) throw new Error("radius 3 to 24");
      this.radius = radius;
      const side = 2 * radius + 1;
      this.side = side;
      this.grid = new Uint16Array(side * side).fill(OFF);
      this.coords = [];
      for (let r = -radius; r <= radius; r++) {
        for (let q = -radius; q <= radius; q++) {
          if (hexLen(q, r) <= radius) {
            this.grid[(q + radius) + (r + radius) * side] = this.coords.length;
            this.coords.push([q, r]);
          }
        }
      }
      this.len = this.coords.length;
      this.nbr = new Uint16Array(this.len * 6);
      for (let c = 0; c < this.len; c++) {
        const [q, r] = this.coords[c];
        for (let d = 0; d < 6; d++) {
          const n = this.at(q + DIRS[d][0], r + DIRS[d][1]);
          this.nbr[c * 6 + d] = n === null ? OFF : n;
        }
      }
    }
    at(q, r) {
      if (hexLen(q, r) > this.radius) return null;
      const v = this.grid[(q + this.radius) + (r + this.radius) * this.side];
      return v === OFF ? null : v;
    }
    step(c, d) { const n = this.nbr[c * 6 + d]; return n === OFF ? null : n; }
    distance(a, b) {
      const [aq, ar] = this.coords[a], [bq, br] = this.coords[b];
      return hexLen(aq - bq, ar - br);
    }
    ring(rho) {
      if (rho === 0 || rho > this.radius) return [];
      let q = DIRS[4][0] * rho, r = DIRS[4][1] * rho;
      const out = [];
      for (let d = 0; d < 6; d++) {
        for (let i = 0; i < rho; i++) {
          const c = this.at(q, r);
          if (c !== null) out.push([c, d]);
          q += DIRS[d][0]; r += DIRS[d][1];
        }
      }
      return out;
    }
  }

  const turnDir = (d, t) => (d + DELTA[t]) % 6;

  // ── the rules ─────────────────────────────────────────────────────────────
  class Garden {
    constructor(board, starts) {
      if (starts.length < 1 || starts.length > 8) throw new Error("1 to 8 vines");
      this.board = board;
      this.owner = new Uint8Array(board.len);
      this.vines = [];
      this.tick = 0;
      starts.forEach(([c, d], k) => {
        if (this.owner[c] !== 0) throw new Error("crowded");
        this.owner[c] = k + 1;
        this.vines.push({ head: c, heading: d, stem: [c], wilted: null });
      });
    }
    growing() {
      const out = [];
      this.vines.forEach((v, k) => { if (v.wilted === null) out.push(k); });
      return out;
    }
    outcome() {
      const alive = this.growing();
      if (this.vines.length === 1) return alive.length === 0 ? { kind: "won", by: 0 } : { kind: "growing" };
      if (alive.length === 0) return { kind: "draw" };
      if (alive.length === 1) return { kind: "won", by: alive[0] };
      return { kind: "growing" };
    }
    over() { return this.outcome().kind !== "growing"; }
    target(v, t) {
      const vine = this.vines[v];
      return this.board.step(vine.head, turnDir(vine.heading, t));
    }
    free() { let n = 0; for (const o of this.owner) if (o === 0) n++; return n; }
    step(turns) {
      if (turns.length !== this.vines.length) throw new Error("one turn per vine");
      if (this.over()) return { tick: this.tick, grew: [], wilted: [] };
      const n = this.vines.length;
      const heading = [], target = [];
      for (let k = 0; k < n; k++) {
        const v = this.vines[k];
        const h = turnDir(v.heading, turns[k]);
        heading.push(h);
        target.push(v.wilted === null ? this.board.step(v.head, h) : null);
      }
      const next = this.tick + 1;
      const out = { tick: next, grew: [], wilted: [] };
      const fate = new Array(n).fill(null);
      for (let k = 0; k < n; k++) {
        if (this.vines[k].wilted !== null) continue;
        const t = target[k];
        if (t === null) fate[k] = { kind: "wall" };
        else if (this.owner[t] !== 0) fate[k] = { kind: "stem", into: this.owner[t] - 1 };
        else {
          for (let j = 0; j < n; j++) {
            if (j !== k && target[j] === t) { fate[k] = { kind: "clash", with: j }; break; }
          }
        }
      }
      for (let k = 0; k < n; k++) {
        const v = this.vines[k];
        if (v.wilted !== null) continue;
        if (fate[k] !== null) {
          v.wilted = [next, fate[k]];
          out.wilted.push([k, fate[k]]);
        } else {
          const t = target[k];
          this.owner[t] = k + 1;
          v.head = t; v.heading = heading[k]; v.stem.push(t);
          out.grew.push([k, t]);
        }
      }
      this.tick = next;
      return out;
    }
  }

  // ── the gardeners ─────────────────────────────────────────────────────────
  const UNSEEN = 0xffffffff, TIE = 255;

  function makeScratch(n) {
    return { dist: new Uint32Array(n), who: new Uint8Array(n), queue: new Uint16Array(n + 16) };
  }

  function open(g, c, block) { return c !== OFF && c !== block && g.owner[c] === 0; }

  function flood(g, me, from, s) {
    const nb = g.board.nbr;
    s.dist.fill(UNSEEN);
    const heads = g.growing().filter((v) => v !== me).map((v) => g.vines[v].head);
    let qlen = 0, i = 0, reach = 1, contact = false;
    s.dist[from] = 0; s.queue[qlen++] = from;
    while (i < qlen) {
      const c = s.queue[i++];
      for (let d = 0; d < 6; d++) {
        const x = nb[c * 6 + d];
        if (x === OFF) continue;
        if (heads.includes(x)) contact = true;
        if (open(g, x, from) && s.dist[x] === UNSEEN) {
          s.dist[x] = 0; s.queue[qlen++] = x; reach++;
        }
      }
    }
    return [reach, contact];
  }

  function voronoi(g, me, from, s) {
    const nb = g.board.nbr;
    s.dist.fill(UNSEEN); s.who.fill(TIE);
    const count = new Array(g.vines.length).fill(0);
    let qlen = 0, i = 0;
    s.dist[from] = 0; s.who[from] = me; s.queue[qlen++] = from;
    for (const v of g.growing()) {
      if (v === me) continue;
      const h = g.vines[v].head;
      s.dist[h] = 0; s.who[h] = v; s.queue[qlen++] = h;
    }
    while (i < qlen) {
      const c = s.queue[i++];
      const d = s.dist[c] + 1, w = s.who[c];
      for (let k = 0; k < 6; k++) {
        const x = nb[c * 6 + k];
        if (!open(g, x, from)) continue;
        if (s.dist[x] === UNSEEN) {
          s.dist[x] = d; s.who[x] = w; s.queue[qlen++] = x;
          if (w !== TIE) count[w]++;
        } else if (s.dist[x] === d && s.who[x] !== w && s.who[x] !== TIE) {
          count[s.who[x]]--; s.who[x] = TIE;
        }
      }
    }
    return count;
  }

  function score(temper, l, bestReach, roll) {
    const trap = l.reach * 2 < bestReach ? -1000000 : 0;
    if (!l.contact) return trap + l.reach * 100 + l.walls * 7 - l.danger * 5000;
    let body;
    switch (temper) {
      case "ivy": body = l.mine * 20 - l.theirs * 4 + l.reach - l.danger * 5000; break;
      case "kudzu": body = l.mine * 10 - l.nearestTheirs * 12 - l.nearest * 30 - l.danger * 2000; break;
      case "moss": body = l.reach * 20 + l.walls * 15 + l.mine * 4 - l.danger * 5000; break;
      default: body = l.reach * 6 + l.mine * 6 + roll - l.danger * 3000;
    }
    return trap + body;
  }

  function choose(g, me, temper, rng, s) {
    const rivals = g.growing().filter((v) => v !== me);
    const nb = g.board.nbr;
    const looks = [];
    for (const turn of TURNS) {
      const cell = g.target(me, turn);
      if (cell === null || g.owner[cell] !== 0) continue;
      const [reach, contact] = flood(g, me, cell, s);
      const terr = voronoi(g, me, cell, s);
      const mine = terr[me];
      let theirs = 0;
      for (const v of rivals) theirs += terr[v];
      let nearest = 0, nearestTheirs = 0, best = UNSEEN;
      for (const v of rivals) {
        const d = g.board.distance(cell, g.vines[v].head);
        if (d < best) { best = d; nearest = d; nearestTheirs = terr[v]; }
      }
      let walls = 0;
      for (let d = 0; d < 6; d++) {
        const x = nb[cell * 6 + d];
        if (x === OFF || g.owner[x] !== 0) walls++;
      }
      let danger = 0;
      for (const v of rivals) {
        const h = g.vines[v].head;
        for (let d = 0; d < 6; d++) if (nb[h * 6 + d] === cell) { danger++; break; }
      }
      looks.push({ turn, cell, reach, mine, nearestTheirs, theirs, nearest, walls, danger, contact, score: 0 });
    }
    if (looks.length === 0) return { turn: "straight", looks };
    let bestReach = 0;
    for (const l of looks) bestReach = Math.max(bestReach, l.reach);
    for (const l of looks) {
      const roll = temper === "hops" ? rng.below(600) : 0;
      l.score = score(temper, l, bestReach, roll);
    }
    let pick = 0;
    looks.forEach((l, k) => { if (l.score > looks[pick].score) pick = k; });
    return { turn: looks[pick].turn, looks };
  }

  // ── rounds ────────────────────────────────────────────────────────────────
  function sow(board, n, seed) {
    const rng = new Rng(seed);
    const rho = Math.max(1, Math.floor(board.radius * 2 / 3));
    const ring = board.ring(rho);
    const len = Math.max(1, ring.length);
    const offset = rng.below(len);
    const out = [];
    for (let k = 0; k < n; k++) {
      const at = (Math.floor((k * len) / n) + offset) % len;
      const [cell, along] = ring[at];
      const lean = rng.below(3) === 1 ? "left" : "straight";
      out.push([cell, turnDir(along, lean)]);
    }
    return out;
  }

  function gardeners(n) {
    const out = [];
    for (let k = 0; k < n; k++) out.push({ kind: "gardener", temper: TEMPERS[k % 4] });
    return out;
  }

  class Round {
    constructor(radius, seed, seats) {
      this.seed = seed >>> 0;
      this.seats = seats.slice();
      this.board = new Board(radius);
      this.garden = new Garden(this.board, sow(this.board, seats.length, this.seed));
      this.die = new Rng((this.seed ^ DIE_SALT) >>> 0);
      this.scratch = makeScratch(this.board.len);
      this.keys = [];
      this.lastLooks = new Array(seats.length).fill(null);
    }
    outcome() { return this.garden.outcome(); }
    // human: Map or object { vine: turn }
    tick(human) {
      const g = this.garden;
      if (g.over()) return { tick: g.tick, grew: [], wilted: [] };
      const next = g.tick + 1;
      const turns = new Array(this.seats.length).fill("straight");
      this.seats.forEach((seat, k) => {
        if (g.vines[k].wilted !== null) return;
        if (seat.kind === "gardener") {
          const c = choose(g, k, seat.temper, this.die, this.scratch);
          this.lastLooks[k] = c.looks;
          turns[k] = c.turn;
        } else {
          const t = human && human[k];
          if (t) {
            turns[k] = t;
            if (t !== "straight") this.keys.push({ tick: next, vine: k, turn: t });
          }
        }
      });
      return g.step(turns);
    }
    playOut() {
      const bound = this.board.len + 1;
      for (let i = 0; i < bound && !this.garden.over(); i++) this.tick(null);
      return this.garden.outcome();
    }
  }

  const api = { DIRS, OFF, TURNS, TEMPERS, Rng, seedOf, Board, Garden, Round, sow, gardeners, choose, hexLen };
  root.TrellisEngine = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(typeof globalThis !== "undefined" ? globalThis : this);
