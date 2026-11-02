// Trellis — the page glue. Wired by data-page; no inline scripts.
(function () {
  "use strict";
  const E = window.TrellisEngine, R = window.TrellisRender;
  const token = (document.querySelector('meta[name="csrf-token"]') || {}).content || "";
  const $ = (id) => document.getElementById(id);
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));

  function post(url, body) {
    return fetch(url, {
      method: "POST",
      headers: { "content-type": "application/x-www-form-urlencoded", "x-csrf-token": token },
      body: new URLSearchParams(body).toString(),
    }).then(async (r) => {
      const text = await r.text();
      let json = null; try { json = JSON.parse(text); } catch (e) { /* not json */ }
      if (!r.ok) throw new Error((json && (json.message || json.error)) || text || r.statusText);
      return json;
    });
  }

  const store = {
    get(k) { try { return window.localStorage.getItem(k); } catch (e) { return null; } },
    set(k, v) { try { window.localStorage.setItem(k, v); } catch (e) { /* private window */ } },
    del(k) { try { window.localStorage.removeItem(k); } catch (e) { /* ignore */ } },
  };

  function epitaph(seats, k, d) {
    const name = (j) => (seats[j] ? seats[j].name : "vine " + j);
    if (d.kind === "wall") return `${name(k)} grew off the wall`;
    if (d.kind === "stem") return d.into === k ? `${name(k)} strangled itself` : `${name(k)} ran into ${name(d.into)}`;
    return `${name(k)} and ${name(d.with)} reached for the same cell`;
  }

  function fitCanvas(canvas) {
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    const w = Math.round(canvas.clientWidth * dpr);
    const h = Math.round(w * 0.91);
    if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
  }

  function steerKeys(onTurn) {
    window.addEventListener("keydown", (e) => {
      if (e.target && (e.target.tagName === "INPUT" || e.target.tagName === "SELECT")) return;
      const k = e.key;
      if (k === "ArrowLeft" || k === "a" || k === "A") { onTurn("left"); e.preventDefault(); }
      if (k === "ArrowRight" || k === "d" || k === "D") { onTurn("right"); e.preventDefault(); }
    });
    document.querySelectorAll(".pad").forEach((b) => b.addEventListener("click", () => onTurn(b.dataset.turn)));
  }

  function banner(html, ms) {
    const b = $("banner"); if (!b) return;
    b.innerHTML = html; b.classList.add("on");
    clearTimeout(banner.t);
    if (ms) banner.t = setTimeout(() => b.classList.remove("on"), ms);
  }
  function unbanner() { const b = $("banner"); if (b) b.classList.remove("on"); }

  // ── The Arena ───────────────────────────────────────────────────────────
  function arena() {
    const canvas = $("wall");
    const S = { round: 0, tick: 0, seats: [], vines: [], board: null, phase: "sprouting", outcome: null, beat: 125, wins: [] };
    let ticket = store.get("trellis.ticket") || null;
    let myHue = Number(store.get("trellis.hue") || 8);
    let myName = store.get("trellis.name") || "";
    $("name").value = myName;

    const hues = document.querySelectorAll(".hue");
    const pressHue = () => hues.forEach((b) => b.setAttribute("aria-pressed", String(Number(b.dataset.hue) === myHue)));
    hues.forEach((b) => b.addEventListener("click", () => { myHue = Number(b.dataset.hue); store.set("trellis.hue", String(myHue)); pressHue(); }));
    pressHue();

    function load(snap) {
      S.round = snap.round; S.tick = snap.tick; S.seats = snap.seats; S.phase = snap.phase;
      S.board = (S.board && S.board.radius === snap.radius) ? S.board : new E.Board(snap.radius);
      S.outcome = snap.outcome; S.wins = snap.wins || [];
      S.vines = snap.vines.map((v, k) => ({
        stem: v.stem.slice(), wilted: v.wilted, hue: snap.seats[k].hue, name: snap.seats[k].name,
        human: snap.seats[k].kind === "human", wiltedAt: -1e9, wonAt: -1e9,
      }));
      if (snap.phase === "sprouting") banner(`Round ${snap.round}<small>sprouting</small>`, 1500);
      cast(); wins();
    }
    function sync() { return fetch("/_arena.json").then((r) => r.json()).then(load).catch(() => {}); }

    function you() {
      return S.vines.findIndex((v, k) => v.human && S.seats[k] && S.seats[k].name === myName && ticket);
    }

    function cast() {
      const me = you();
      $("cast").innerHTML = S.seats.map((s, k) => {
        const v = S.vines[k] || { stem: [], wilted: null };
        const cls = [v.wilted ? "wilted" : "", k === me ? "you" : ""].join(" ");
        const sub = s.temper ? s.temper : "person";
        return `<li class="${cls}"><span class="dot" style="background:hsl(${s.hue} 62% 58%);color:hsl(${s.hue} 62% 58%)"></span>` +
          `<span><span class="name">${esc(s.name)}</span> <span class="temper">${esc(sub)}</span></span>` +
          `<span class="len">${v.stem.length}</span></li>`;
      }).join("");
      $("roundinfo").textContent = `Round ${S.round} · tick ${S.tick} · ${S.vines.filter((v) => !v.wilted).length} growing`;
    }
    function wins() {
      $("wins").innerHTML = (S.wins || []).slice(0, 8).map(([n, w]) => `<tr><td>${esc(n)}</td><td class="num">${w}</td></tr>`).join("") ||
        `<tr><td class="hint">no round finished yet</td></tr>`;
    }
    function log(html) {
      const li = document.createElement("li"); li.innerHTML = html;
      const ul = $("log"); ul.insertBefore(li, ul.firstChild);
      while (ul.children.length > 40) ul.removeChild(ul.lastChild);
    }
    const tag = (k) => { const s = S.seats[k]; return s ? `<b style="color:hsl(${s.hue} 62% 66%)">${esc(s.name)}</b>` : "?"; };

    function onEvent(e) {
      const now = performance.now();
      if (e.t === "round") {
        S.round = e.round; S.tick = 0; S.seats = e.seats; S.phase = "sprouting"; S.outcome = null;
        if (!S.board || S.board.radius !== e.radius) S.board = new E.Board(e.radius);
        S.vines = e.starts.map(([c], k) => ({ stem: [c], wilted: null, hue: e.seats[k].hue, name: e.seats[k].name, human: e.seats[k].kind === "human", wiltedAt: 0 }));
        const me = you();
        if (me >= 0) $("joinstate").textContent = `You are on the wall: the ${colourWord(S.seats[me].hue)} vine, seat ${me + 1}.`;
        else if (ticket) $("joinstate").textContent = "Waiting for a free seat…";
        banner(`Round ${e.round}` + (me >= 0 ? `<small>you are the ${esc(colourWord(S.seats[me].hue))} vine — get ready</small>` : `<small>sprouting</small>`), e.beats * 125);
        cast();
      } else if (e.t === "tick") {
        if (e.round !== S.round || e.tick !== S.tick + 1) { sync(); return; }
        S.phase = "growing"; S.tick = e.tick;
        if (e.tick === 1) unbanner();
        for (const [k, c] of e.grew) { S.vines[k].stem.push(c); S.vines[k].grewAt = now; }
        for (const [k, d] of e.wilted) {
          S.vines[k].wilted = [e.tick, d]; S.vines[k].wiltedAt = now;
          log(`<span class="hint">${e.tick}</span> ${esc(epitaph(S.seats, k, d)).replace(esc(S.seats[k].name), tag(k))}`);
        }
        cast();
      } else if (e.t === "pressed") {
        S.phase = "pressed"; S.outcome = e.outcome;
        if (e.outcome.kind === "won") {
          const v = S.vines[e.outcome.by]; if (v) v.wonAt = now;
          banner(`${esc(S.seats[e.outcome.by].name)} wins the wall<small>${e.ticks} ticks · stem of ${e.lengths[e.outcome.by]}</small>`, 3600);
          log(`${tag(e.outcome.by)} won round ${e.round}`);
        } else {
          banner(`A draw<small>the last vines wilted together</small>`, 3600);
          log(`round ${e.round} was a draw`);
        }
        setTimeout(sync, 300);
      } else if (e.t === "joined") {
        log(`<b style="color:hsl(${e.hue} 62% 66%)">${esc(e.name)}</b> is waiting for a seat`);
      }
    }

    const colourWord = (h) => (h < 20 ? "coral" : h < 40 ? "amber" : h < 70 ? "gold" : h < 170 ? "green" : h < 200 ? "teal" : h < 250 ? "blue" : h < 320 ? "violet" : "rose");

    $("join").addEventListener("click", () => {
      const name = $("name").value.trim();
      if (!name) { $("joinstate").textContent = "a name first"; return; }
      post("/join", { name, hue: myHue }).then((j) => {
        ticket = j.ticket; myName = name;
        store.set("trellis.ticket", ticket); store.set("trellis.name", name);
        $("joinstate").textContent = `You're in. You take a seat when the next round is planted (${j.players} waiting or playing).`;
      }).catch((err) => { $("joinstate").textContent = err.message; });
    });

    steerKeys((turn) => {
      if (!ticket) return;
      post("/steer", { ticket, turn }).catch((err) => {
        if (/not in the garden/.test(err.message)) { ticket = null; store.del("trellis.ticket"); $("joinstate").textContent = "Your seat lapsed — join again."; }
      });
    });

    function frame() {
      fitCanvas(canvas);
      if (S.board) {
        const winner = S.outcome && S.outcome.kind === "won" ? S.outcome.by : null;
        const me = you();
        R.draw(canvas, { board: S.board, vines: S.vines.map((v, k) => Object.assign({}, v, { you: k === me })), now: performance.now(), beat: S.beat, winner });
      }
      requestAnimationFrame(frame);
    }

    sync().then(() => {
      const es = new EventSource("/stream");
      es.onopen = () => $("live").classList.add("on");
      es.onerror = () => $("live").classList.remove("on");
      es.onmessage = (m) => { try { onEvent(JSON.parse(m.data)); } catch (err) { sync(); } };
      requestAnimationFrame(frame);
    });
    window.trellis = { state: S, sync };
  }

  // ── The potting shed ────────────────────────────────────────────────────
  const SEAT_HUES = [350, 135, 285, 90, 45, 200];
  const seatHue = (k, v) => (v === "you" ? 8 : SEAT_HUES[k % SEAT_HUES.length]);
  function shed() {
    const canvas = $("shed");
    let round = null, playing = false, waiting = false, last = 0, selected = null, queue = [], hueOf = [], names = [];
    const speed = () => Number($("speed").value);

    function seatsFromUi() {
      const seats = [];
      hueOf = []; names = [];
      document.querySelectorAll("select[data-seat]").forEach((s) => {
        const v = s.value, k = Number(s.dataset.seat);
        if (v === "none") return;
        if (v === "you") { if (seats.some((x) => x.kind === "human")) return; seats.push({ kind: "human" }); hueOf.push(seatHue(k, v)); names.push("you"); }
        else { seats.push({ kind: "gardener", temper: v }); hueOf.push(seatHue(k, v)); names.push(v); }
      });
      return seats;
    }
    function dots() {
      document.querySelectorAll(".dot[data-seat]").forEach((d) => {
        const v = document.querySelector(`select[data-seat="${d.dataset.seat}"]`).value;
        d.style.background = v === "none" ? "transparent" : `hsl(${seatHue(Number(d.dataset.seat), v)} 62% 58%)`;
      });
    }
    function plant() {
      const seats = seatsFromUi();
      if (seats.length === 0) return;
      const radius = Number($("radius").value);
      round = new E.Round(radius, E.seedOf($("word").value || "trellis"), seats);
      round.wilt = new Array(seats.length).fill(-1e9);
      round.grew = new Array(seats.length).fill(0);
      queue = []; selected = seats.findIndex((s) => s.kind === "gardener"); if (selected < 0) selected = null;
      const human = seats.some((s) => s.kind === "human");
      waiting = human;
      playing = !human; last = performance.now() + 900;
      $("pause").textContent = playing ? "Pause" : "Play";
      banner(human ? "You are the coral vine<small>press ← or → (or Play) to start growing</small>"
        : "Planted<small>" + esc($("word").value || "trellis") + "</small>", human ? 0 : 900);
      looks();
    }
    function stepOnce() {
      if (!round || round.garden.over()) return;
      const human = round.seats.findIndex((s) => s.kind === "human");
      const turns = {};
      if (human >= 0 && queue.length) turns[human] = queue.shift();
      const t = round.tick(turns);
      const now = performance.now();
      for (const [k] of t.grew) round.grew[k] = now;
      for (const [k] of t.wilted) round.wilt[k] = now;
      if (round.garden.over()) {
        const o = round.outcome();
        round.wonAt = now;
        banner(o.kind === "won" ? `${esc(names[o.by])} wins<small>${round.garden.tick} ticks</small>` : "A draw", 2600);
      }
      looks();
    }
    function looks() {
      const tbl = $("looks");
      if (!round || selected === null || !round.lastLooks[selected]) { tbl.innerHTML = `<tr><td class="hint">click a gardener's vine</td></tr>`; return; }
      const ls = round.lastLooks[selected];
      const best = ls.reduce((b, l, i) => (l.score > ls[b].score ? i : b), 0);
      tbl.innerHTML = `<tr><th>${esc(names[selected])}</th><th>reach</th><th>mine</th><th>theirs</th><th>score</th></tr>` +
        ls.map((l, i) => `<tr class="${i === best ? "pick" : ""}"><td>${l.turn}${l.contact ? "" : " ⌂"}</td><td>${l.reach}</td><td>${l.mine}</td><td>${l.theirs}</td><td>${l.score}</td></tr>`).join("") +
        (ls.some((l) => !l.contact) ? `<tr><td colspan="5" class="hint">⌂ sealed off — packing, not fighting</td></tr>` : "");
    }
    canvas.addEventListener("click", (ev) => {
      if (!round) return;
      const r = canvas.getBoundingClientRect();
      const c = R.pick(canvas, round.board, (ev.clientX - r.left) * (canvas.width / r.width), (ev.clientY - r.top) * (canvas.height / r.height));
      let best = null, bd = Infinity;
      round.garden.vines.forEach((v, k) => {
        if (round.seats[k].kind !== "gardener") return;
        for (const s of v.stem) { const d = round.board.distance(s, c); if (d < bd) { bd = d; best = k; } }
      });
      if (best !== null && bd <= 2) { selected = best; looks(); }
    });
    $("plant").addEventListener("click", plant);
    $("pause").addEventListener("click", () => { if (waiting) { go(); return; } playing = !playing; $("pause").textContent = playing ? "Pause" : "Play"; });
    $("stepbtn").addEventListener("click", () => { playing = false; $("pause").textContent = "Play"; stepOnce(); });
    $("radius").addEventListener("input", () => { $("rlabel").textContent = $("radius").value; });
    document.querySelectorAll("select[data-seat]").forEach((s) => s.addEventListener("change", dots));
    $("word").addEventListener("keydown", (e) => { if (e.key === "Enter") plant(); });
    const go = () => { if (waiting) { waiting = false; playing = true; $("pause").textContent = "Pause"; unbanner(); } };
    steerKeys((t) => { if (waiting) { go(); return; } if (queue.length < 3) queue.push(t); });
    dots();

    function frame(now) {
      fitCanvas(canvas);
      if (round) {
        const beat = 1000 / speed();
        if (playing && !round.garden.over() && now - last >= beat) { last = now; stepOnce(); }
        const g = round.garden, o = g.outcome();
        R.draw(canvas, {
          board: round.board, now, beat,
          vines: g.vines.map((v, k) => ({ stem: v.stem, wilted: v.wilted, hue: hueOf[k], name: names[k], human: round.seats[k].kind === "human", you: round.seats[k].kind === "human", grewAt: round.grew[k], wiltedAt: round.wilt[k], wonAt: round.wonAt })),
          territory: $("eye").checked && !g.over() ? R.territory(g) : null,
          selected, winner: o.kind === "won" && g.vines.length > 1 ? o.by : null,
        });
        $("shedinfo").textContent = `tick ${g.tick} · ${g.growing().length} growing · ${g.free()} cells free`;
      }
      requestAnimationFrame(frame);
    }
    plant();
    requestAnimationFrame(frame);
    window.trellis = { get round() { return round; }, plant, stepOnce };
  }

  // ── A pressing ──────────────────────────────────────────────────────────
  function pressing() {
    const svg = document.querySelector(".bigsheet .plate");
    const btn = $("regrow");
    if (!svg || !btn) return;
    btn.addEventListener("click", () => {
      const stems = svg.querySelectorAll(".stem");
      const per = 70; // ms per cell
      let longest = 0;
      svg.classList.add("regrowing");
      stems.forEach((p) => {
        const len = p.getTotalLength();
        const cells = Number(p.dataset.cells) || 1;
        longest = Math.max(longest, cells);
        p.style.transition = "none";
        p.style.strokeDasharray = `${len} ${len}`;
        p.style.strokeDashoffset = String(len);
        p.getBoundingClientRect();
        p.style.transition = `stroke-dashoffset ${cells * per}ms linear`;
        p.style.strokeDashoffset = "0";
      });
      setTimeout(() => svg.classList.remove("regrowing"), longest * per);
    });
  }

  document.addEventListener("DOMContentLoaded", () => {
    const p = document.body.dataset.page;
    if (p === "arena") arena();
    else if (p === "shed") shed();
    else if (p === "pressing") pressing();
  });
})();
