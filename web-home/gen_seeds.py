# -*- coding: utf-8 -*-
"""Generate the two theme seeds (light/dark) for the arcana.boo homepage sky.
One geometry — v1's net, faithfully: hash-random placement, drifting beings
with breathing glow halos, proximity links that form/break, a pulse of light
travelling being to being with an arrival flare, the moon with its halo, snow.
Only the palette differs per theme; the canvas stays transparent so the page's
own themed ground shows through (v1's exact behaviour)."""
import os

WT = os.path.expanduser("~/Documents/projects/ai/wasm-jit/.claude/worktrees/canvas-poc")

# v1 JS tokens → hsl (h,s,l in 0..1)
THEMES = {
    "light": dict(
        BLUE="0.553, 0.65, 0.375",    # rgb(33,122,158)
        BLUE_CORE="0.553, 0.6, 0.42",
        EMB="0.068, 0.47, 0.47",      # rgb(176,111,64)
        MOON="0.551, 0.58, 0.60",     # rgb(95,176,212)
        SNOW="0.576, 0.26, 0.66",     # rgb(120,150,175)-ish, lifted for light ground
        LINK_BASE="0.80", LINK_SPAN="-0.33",   # faint = lighter on light ground
        PULSE="0.56, 0.60, 0.42",
    ),
    "dark": dict(
        BLUE="0.557, 0.68, 0.69",     # rgb(124,198,230)
        BLUE_CORE="0.557, 0.68, 0.78",
        EMB="0.068, 0.55, 0.62",      # rgb(210,154,104)
        MOON="0.557, 0.68, 0.70",
        SNOW="0.58, 0.30, 0.86",
        LINK_BASE="0.20", LINK_SPAN="0.30",    # faint = darker on dark ground
        PULSE="0.575, 0.72, 0.86",
    ),
}

TEMPLATE = """// arcana.boo homepage sky ({name} theme) — a wasm-jit `draw` seed, compiled
// LIVE in your browser by the wasm-jit compiler (itself Rust compiled to WASM).
// Everything moving on this page is this one module. Its entire world is
// 10 drawing primitives — it cannot fetch, cannot hold state, cannot read
// the page. run(t, w, h), ~60 times a second.

let cx = w * 0.785;
let cy = h * 0.35;
let S  = h * 0.27;
if w < h {{ S = w * 0.27; }}
let D  = S * 0.62;
let D2 = D * D;
let N  = 13.0;

// ---- the moon and its halo ----
let mm = h * 0.115;
if w < h {{ mm = w * 0.115; }}
hsl({MOON});
glow(w * 0.84, h * 0.27, mm * 2.6);
disc(w * 0.84, h * 0.27, mm * 0.45);

// ---- snow: each flake its own quiet fall ----
let s = 0.0;
while s < 30.0 {{
  let px = sin(s * 12.9898) * 437.585;
  px = px % 1.0;
  if px < 0.0 {{ px = px + 1.0; }}
  let vy = 0.016 + (s % 5.0) * 0.005;
  let py = px * 3.7 + t * vy;
  py = py % 1.0;
  if py < 0.0 {{ py = py + 1.0; }}
  hsl({SNOW});
  disc(px * w + sin(t * 0.4 + s) * 6.0, py * h, 0.8 + (s % 3.0) * 0.3);
  s = s + 1.0;
}}

// ---- links: transient binding between every near pair (they form and break) ----
let i = 0.0;
while i < N {{
  let pa = sin(i * 12.9898) * 437.585;
  pa = pa % 1.0;
  if pa < 0.0 {{ pa = pa + 1.0; }}
  let pr = sin(i * 78.233) * 125.432;
  pr = pr % 1.0;
  if pr < 0.0 {{ pr = pr + 1.0; }}
  let xi = cx + cos(pa * 6.2832) * (0.15 + 0.9 * pr) * S + sin(t * 0.22 + i) * 12.0;
  let yi = cy + sin(pa * 6.2832) * (0.15 + 0.9 * pr) * S * 0.82 + cos(t * 0.17 + i * 1.7) * 10.0;
  let j = i + 1.0;
  while j < N {{
    let qa = sin(j * 12.9898) * 437.585;
    qa = qa % 1.0;
    if qa < 0.0 {{ qa = qa + 1.0; }}
    let qr = sin(j * 78.233) * 125.432;
    qr = qr % 1.0;
    if qr < 0.0 {{ qr = qr + 1.0; }}
    let xj = cx + cos(qa * 6.2832) * (0.15 + 0.9 * qr) * S + sin(t * 0.22 + j) * 12.0;
    let yj = cy + sin(qa * 6.2832) * (0.15 + 0.9 * qr) * S * 0.82 + cos(t * 0.17 + j * 1.7) * 10.0;
    let dx = xi - xj;
    let dy = yi - yj;
    let q = dx * dx + dy * dy;
    if q < D2 {{
      let cl = 1.0 - q / D2;
      hsl(0.556, 0.45, {LINK_BASE} + cl * {LINK_SPAN});
      line(xi, yi, xj, yj);
    }}
    j = j + 1.0;
  }}
  i = i + 1.0;
}}

// ---- the light: gliding being to being, flaring what it reaches ----
let slot = t * 0.55;
let fs = slot - (slot % 1.0);
let pp = slot % 1.0;
let ha = sin(fs * 91.17) * 331.73;
ha = ha % 1.0;
if ha < 0.0 {{ ha = ha + 1.0; }}
let na = ha * N;
na = na - (na % 1.0);
let hb = sin(fs * 47.53) * 217.31;
hb = hb % 1.0;
if hb < 0.0 {{ hb = hb + 1.0; }}
let nb = hb * N;
nb = nb - (nb % 1.0);
let dna = nb - na;
if dna < 0.5 {{ if dna > 0.0 - 0.5 {{ nb = nb + 1.0; if nb >= N {{ nb = 0.0; }} }} }}

// ---- beings: water-blue cells and ember souls, breathing halos ----
let k = 0.0;
while k < N {{
  let ka = sin(k * 12.9898) * 437.585;
  ka = ka % 1.0;
  if ka < 0.0 {{ ka = ka + 1.0; }}
  let kr = sin(k * 78.233) * 125.432;
  kr = kr % 1.0;
  if kr < 0.0 {{ kr = kr + 1.0; }}
  let xk = cx + cos(ka * 6.2832) * (0.15 + 0.9 * kr) * S + sin(t * 0.22 + k) * 12.0;
  let yk = cy + sin(ka * 6.2832) * (0.15 + 0.9 * kr) * S * 0.82 + cos(t * 0.17 + k * 1.7) * 10.0;
  let rr = 1.7 + (k % 3.0) * 0.55;
  let fl = 0.0;
  let dkb = k - nb;
  if dkb < 0.5 {{ if dkb > 0.0 - 0.5 {{ if pp > 0.72 {{ fl = (pp - 0.72) * 3.5; }} }} }}
  let sv = k % 5.0;
  if sv > 1.5 {{
    if sv < 2.5 {{
      hsl({EMB});
      glow(xk, yk, (rr + 0.5) * 4.0 + fl * 10.0);
      disc(xk, yk, rr + 0.5 + fl * 2.0);
    }}
  }}
  if sv < 1.5 {{
    hsl({BLUE});
    glow(xk, yk, rr * 4.0 + fl * 10.0);
    hsl({BLUE_CORE});
    disc(xk, yk, rr + fl * 2.0);
  }}
  if sv > 2.5 {{
    hsl({BLUE});
    glow(xk, yk, rr * 4.0 + fl * 10.0);
    hsl({BLUE_CORE});
    disc(xk, yk, rr + fl * 2.0);
  }}
  k = k + 1.0;
}}

// the travelling light itself
let aa = sin(na * 12.9898) * 437.585;
aa = aa % 1.0;
if aa < 0.0 {{ aa = aa + 1.0; }}
let ar = sin(na * 78.233) * 125.432;
ar = ar % 1.0;
if ar < 0.0 {{ ar = ar + 1.0; }}
let xa = cx + cos(aa * 6.2832) * (0.15 + 0.9 * ar) * S + sin(t * 0.22 + na) * 12.0;
let ya = cy + sin(aa * 6.2832) * (0.15 + 0.9 * ar) * S * 0.82 + cos(t * 0.17 + na * 1.7) * 10.0;
let ba = sin(nb * 12.9898) * 437.585;
ba = ba % 1.0;
if ba < 0.0 {{ ba = ba + 1.0; }}
let br = sin(nb * 78.233) * 125.432;
br = br % 1.0;
if br < 0.0 {{ br = br + 1.0; }}
let xb = cx + cos(ba * 6.2832) * (0.15 + 0.9 * br) * S + sin(t * 0.22 + nb) * 12.0;
let yb = cy + sin(ba * 6.2832) * (0.15 + 0.9 * br) * S * 0.82 + cos(t * 0.17 + nb * 1.7) * 10.0;
hsl({PULSE});
glow(xa + (xb - xa) * pp, ya + (yb - ya) * pp, 8.0);
disc(xa + (xb - xa) * pp, ya + (yb - ya) * pp, 1.8);

0.0
"""

for name, c in THEMES.items():
    src = TEMPLATE.format(name=name, **c)
    out = os.path.join(WT, f"examples/homepage-{name}.dsl")
    open(out, "w", encoding="utf-8").write(src)
    print(f"{name}: {len(src)} chars -> {out}")
