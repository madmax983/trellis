// Check the browser engine against the Rust engine.
//
//   cargo run -q --release -p trellis --example dump -- ivy moss ... > dump.jsonl
//   node tools/parity.js dump.jsonl
//
// Every line is one round Rust played. Replay it here and compare every vine:
// head, heading, every stem cell in order, and when and how it wilted.
"use strict";
const fs = require("fs");
const path = require("path");
const E = require(path.join(__dirname, "..", "crates", "trellis-app", "src", "engine.js"));

function canon(x) {
  if (Array.isArray(x)) return "[" + x.map(canon).join(",") + "]";
  if (x && typeof x === "object") {
    return "{" + Object.keys(x).sort().map((k) => JSON.stringify(k) + ":" + canon(x[k])).join(",") + "}";
  }
  return JSON.stringify(x);
}

const lines = fs.readFileSync(process.argv[2], "utf8").split("\n").filter(Boolean);
let rounds = 0, cells = 0, bad = 0;
for (const line of lines) {
  const want = JSON.parse(line);
  let r;
  if (want.kind === "gardeners") {
    r = new E.Round(16, want.seed, E.gardeners(6));
    r.playOut();
  } else {
    const seats = [{ kind: "human" }].concat(E.gardeners(5));
    r = new E.Round(12, want.seed, seats);
    const script = new E.Rng(want.seed);
    while (!r.garden.over()) {
      const roll = script.below(5);
      const t = roll === 0 ? "left" : roll === 1 ? "right" : "straight";
      r.tick({ 0: t });
    }
  }
  const got = { ticks: r.garden.tick, outcome: r.outcome(), vines: r.garden.vines };
  for (const k of ["ticks", "outcome", "vines"]) {
    if (canon(got[k]) !== canon(want[k])) {
      bad++;
      console.error(`MISMATCH ${want.kind} seed ${want.seed} on ${k}`);
      console.error("  rust:", canon(want[k]).slice(0, 400));
      console.error("  js:  ", canon(got[k]).slice(0, 400));
    }
  }
  rounds++;
  for (const v of want.vines) cells += v.stem.length;
}
if (bad) { console.error(`${bad} mismatches over ${rounds} rounds`); process.exit(1); }
console.log(`js parity: ${rounds} rounds, ${cells} stem cells, identical`);
