// Trellis — the renderer. One canvas, a night garden wall.
//
// view = {
//   board,                      // TrellisEngine.Board
//   vines: [{ stem, wilted, hue, name, human, you, grewAt }],
//   beat,                       // ms per tick, for easing the newest segment
//   now,                        // performance.now()
//   territory,                  // optional Uint8Array: owner per cell, 255 none
//   winner,                     // optional vine index
//   selected,                   // optional vine index
// }
(function (root) {
  "use strict";
  const S3 = Math.sqrt(3);

  function layout(canvas, radius) {
    const w = canvas.width, h = canvas.height;
    const size = Math.min(w / ((2 * radius + 2.3) * S3), h / ((2 * radius + 1.8) * 1.5 + 0.5));
    return { w, h, size, cx: w / 2, cy: h / 2, radius };
  }

  function centre(L, board, c) {
    const [q, r] = board.coords[c];
    return [L.cx + L.size * (S3 * q + (S3 / 2) * r), L.cy + L.size * 1.5 * r];
  }

  function hexPath(ctx, x, y, s) {
    ctx.moveTo(x + s * Math.cos(Math.PI / 6), y + s * Math.sin(Math.PI / 6));
    for (let i = 1; i < 6; i++) {
      const a = Math.PI / 6 + (i * Math.PI) / 3;
      ctx.lineTo(x + s * Math.cos(a), y + s * Math.sin(a));
    }
    ctx.closePath();
  }

  const backdrops = new WeakMap();
  function backdrop(canvas, board, L) {
    const key = board.radius + ":" + canvas.width + "x" + canvas.height;
    const have = backdrops.get(canvas);
    if (have && have.key === key) return have.img;
    const off = document.createElement("canvas");
    off.width = canvas.width; off.height = canvas.height;
    const ctx = off.getContext("2d");
    const g = ctx.createRadialGradient(L.cx, L.cy * 0.9, L.size, L.cx, L.cy, Math.max(L.w, L.h) * 0.7);
    g.addColorStop(0, "#142017"); g.addColorStop(1, "#050806");
    ctx.fillStyle = g; ctx.fillRect(0, 0, off.width, off.height);
    // The wall: every cell a faint hex, the lattice a diagonal weave.
    ctx.beginPath();
    for (let c = 0; c < board.len; c++) {
      const [x, y] = centre(L, board, c);
      hexPath(ctx, x, y, L.size * 0.96);
    }
    ctx.fillStyle = "rgba(120,150,120,0.035)"; ctx.fill();
    ctx.strokeStyle = "rgba(140,170,140,0.07)"; ctx.lineWidth = 1; ctx.stroke();
    ctx.fillStyle = "rgba(190,210,170,0.16)";
    for (let c = 0; c < board.len; c++) {
      const [x, y] = centre(L, board, c);
      ctx.beginPath(); ctx.arc(x, y, Math.max(0.8, L.size * 0.09), 0, Math.PI * 2); ctx.fill();
    }
    // The edge of the wall.
    ctx.beginPath();
    // Pointy-top cells make a hexagon whose corners sit at axial (R, 0) and
    // its rotations: angle 0, 60, … at distance sqrt(3)·R·size.
    const R = board.radius + 0.75;
    for (let i = 0; i < 6; i++) {
      const a = (i * Math.PI) / 3;
      const x = L.cx + L.size * S3 * R * Math.cos(a), y = L.cy + L.size * S3 * R * Math.sin(a);
      if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
    }
    ctx.closePath(); ctx.strokeStyle = "rgba(160,190,150,0.28)"; ctx.lineWidth = 2; ctx.stroke();
    backdrops.set(canvas, { key, img: off });
    return off;
  }

  const col = (hue, l, a) => `hsla(${hue}, 62%, ${l}%, ${a === undefined ? 1 : a})`;

  function bloom(ctx, x, y, hue, s, t) {
    // t: 0..1 unfurling
    const e = 1 - Math.pow(1 - Math.min(1, t), 3);
    const r = s * e;
    if (r <= 0.2) return;
    ctx.save(); ctx.translate(x, y);
    for (let i = 0; i < 5; i++) {
      ctx.rotate((Math.PI * 2) / 5);
      ctx.beginPath();
      ctx.ellipse(0, -r * 0.62, r * 0.42, r * 0.66, 0, 0, Math.PI * 2);
      ctx.fillStyle = col(hue, 68, 0.92); ctx.fill();
    }
    ctx.beginPath(); ctx.arc(0, 0, r * 0.3, 0, Math.PI * 2); ctx.fillStyle = "#f4d35e"; ctx.fill();
    ctx.restore();
  }

  function draw(canvas, view) {
    const ctx = canvas.getContext("2d");
    const board = view.board;
    const L = layout(canvas, board.radius);
    ctx.drawImage(backdrop(canvas, board, L), 0, 0);
    const now = view.now || 0, beat = view.beat || 125;

    if (view.territory) {
      for (let c = 0; c < board.len; c++) {
        const o = view.territory[c];
        if (o === 255 || !view.vines[o]) continue;
        const [x, y] = centre(L, board, c);
        ctx.beginPath(); hexPath(ctx, x, y, L.size * 0.93);
        ctx.fillStyle = col(view.vines[o].hue, 45, view.selected === o ? 0.24 : 0.11); ctx.fill();
      }
    }

    const lw = Math.max(2, L.size * 0.62);
    view.vines.forEach((v, k) => {
      if (!v.stem || v.stem.length === 0) return;
      const alive = v.wilted === null || v.wilted === undefined;
      const pts = v.stem.map((c) => centre(L, board, c));
      // Ease the newest segment in over the beat.
      let frac = 1;
      if (alive && v.grewAt !== undefined && pts.length > 1) frac = Math.max(0, Math.min(1, (now - v.grewAt) / beat));
      const n = pts.length;
      const last = n > 1 ? [pts[n - 2][0] + (pts[n - 1][0] - pts[n - 2][0]) * frac, pts[n - 2][1] + (pts[n - 1][1] - pts[n - 2][1]) * frac] : pts[0];
      const trace = () => {
        ctx.beginPath(); ctx.moveTo(pts[0][0], pts[0][1]);
        for (let i = 1; i < n - 1; i++) ctx.lineTo(pts[i][0], pts[i][1]);
        if (n > 1) ctx.lineTo(last[0], last[1]);
      };
      ctx.lineCap = "round"; ctx.lineJoin = "round";
      const light = alive ? 52 : 30;
      if (alive) {
        ctx.save(); ctx.shadowColor = col(v.hue, 55, 0.9); ctx.shadowBlur = L.size * 1.4;
        trace(); ctx.strokeStyle = col(v.hue, 40, 0.55); ctx.lineWidth = lw * 1.25; ctx.stroke(); ctx.restore();
      }
      trace(); ctx.strokeStyle = col(v.hue, light, alive ? 1 : 0.75); ctx.lineWidth = lw; ctx.stroke();
      trace(); ctx.strokeStyle = col(v.hue, alive ? 75 : 45, alive ? 0.55 : 0.3); ctx.lineWidth = lw * 0.28; ctx.stroke();
      // Leaves every third segment, alternating sides.
      for (let i = 1; i < n; i++) {
        if (i % 3 !== 2) continue;
        const a = pts[i - 1], b = i === n - 1 ? last : pts[i];
        const ang = Math.atan2(b[1] - a[1], b[0] - a[0]) + (i % 2 ? 1 : -1) * 0.95;
        const grow = i === n - 1 ? frac : 1;
        const mx = (a[0] + b[0]) / 2, my = (a[1] + b[1]) / 2;
        ctx.save(); ctx.translate(mx, my); ctx.rotate(ang); ctx.translate(L.size * 0.42 * grow, 0);
        ctx.beginPath(); ctx.ellipse(0, 0, L.size * 0.46 * grow, L.size * 0.21 * grow, 0, 0, Math.PI * 2);
        ctx.fillStyle = col(v.hue, alive ? 60 : 34, alive ? 0.85 : 0.6); ctx.fill(); ctx.restore();
      }
      const [hx, hy] = alive ? last : pts[n - 1];
      if (alive) {
        const pulse = 0.85 + 0.15 * Math.sin(now / 160 + k);
        ctx.beginPath(); ctx.arc(hx, hy, L.size * 0.5 * pulse, 0, Math.PI * 2);
        ctx.fillStyle = col(v.hue, 82); ctx.fill();
        if (v.you || view.selected === k) {
          ctx.beginPath(); ctx.arc(hx, hy, L.size * 1.15 + 2 * Math.sin(now / 200), 0, Math.PI * 2);
          ctx.strokeStyle = col(v.hue, 80, 0.8); ctx.lineWidth = 1.5; ctx.stroke();
        }
      } else {
        const since = v.wiltedAt !== undefined ? (now - v.wiltedAt) / 700 : 1;
        bloom(ctx, hx, hy, v.hue, L.size * (view.winner === k ? 1.6 : 0.95), since);
      }
      if ((v.human || view.selected === k) && v.name) {
        ctx.font = `${Math.max(11, L.size * 0.95)}px Palatino, Georgia, serif`;
        ctx.fillStyle = col(v.hue, 80, alive ? 0.95 : 0.5);
        ctx.fillText(v.name, hx + L.size * 0.9, hy - L.size * 0.9);
      }
    });
    if (view.winner !== undefined && view.winner !== null && view.vines[view.winner]) {
      const v = view.vines[view.winner];
      const n = v.stem.length;
      if (n) {
        const [x, y] = centre(L, board, v.stem[n - 1]);
        const since = v.wonAt !== undefined ? (now - v.wonAt) / 900 : 1;
        bloom(ctx, x, y, v.hue, L.size * 1.8, since);
      }
    }
  }

  // Which cell is under a point, by nearest centre.
  function pick(canvas, board, px, py) {
    const L = layout(canvas, board.radius);
    let best = -1, bd = Infinity;
    for (let c = 0; c < board.len; c++) {
      const [x, y] = centre(L, board, c);
      const d = (x - px) * (x - px) + (y - py) * (y - py);
      if (d < bd) { bd = d; best = c; }
    }
    return best;
  }

  // Whole-wall Voronoi over every growing head: owner per cell, 255 for none
  // or a tie. For display only (the gardeners use their own, per turn).
  function territory(garden) {
    const b = garden.board, n = b.len;
    const dist = new Int32Array(n).fill(-1), who = new Uint8Array(n).fill(255);
    const q = [];
    for (const k of garden.growing()) { const h = garden.vines[k].head; dist[h] = 0; who[h] = k; q.push(h); }
    for (let i = 0; i < q.length; i++) {
      const c = q[i];
      for (let d = 0; d < 6; d++) {
        const x = b.nbr[c * 6 + d];
        if (x === 0xffff || garden.owner[x] !== 0) continue;
        if (dist[x] === -1) { dist[x] = dist[c] + 1; who[x] = who[c]; q.push(x); }
        else if (dist[x] === dist[c] + 1 && who[x] !== who[c]) who[x] = 255;
      }
    }
    for (const k of garden.growing()) who[garden.vines[k].head] = 255;
    return who;
  }

  root.TrellisRender = { draw, pick, layout, centre, territory };
})(typeof globalThis !== "undefined" ? globalThis : this);
