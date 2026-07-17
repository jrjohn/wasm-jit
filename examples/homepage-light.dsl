// arcana.boo homepage sky (light theme) — a wasm-jit `draw` seed, compiled
// LIVE in your browser by the wasm-jit compiler (itself Rust compiled to WASM).
// Everything moving on this page is this one module. Its entire world is
// 10 drawing primitives — it cannot fetch, cannot hold state, cannot read
// the page. run(t, w, h), ~60 times a second.

let cx = w * 0.5;
let cy = h * 0.53;
let S  = h * 0.36;
if w < h { S = w * 0.36; }
let D  = S * 0.72;
let D2 = D * D;
let N  = 13.0;

// ---- the moon and its halo ----
let mm = h * 0.14;
if w < h { mm = w * 0.14; }
hsl(0.551, 0.58, 0.60);
glow(cx + S * 1.7, h * 0.24, mm * 2.4);
disc(cx + S * 1.7, h * 0.24, mm * 0.45);

// ---- snow: each flake its own quiet fall ----
let s = 0.0;
while s < 30.0 {
  let px = sin(s * 12.9898) * 437.585;
  px = px % 1.0;
  if px < 0.0 { px = px + 1.0; }
  let vy = 0.016 + (s % 5.0) * 0.005;
  let py = px * 3.7 + t * vy;
  py = py % 1.0;
  if py < 0.0 { py = py + 1.0; }
  hsl(0.576, 0.26, 0.66);
  disc(px * w + sin(t * 0.4 + s) * 6.0, py * h, 0.8 + (s % 3.0) * 0.3);
  s = s + 1.0;
}

// ---- links: transient binding between every near pair (they form and break) ----
let i = 0.0;
while i < N {
  let pa = sin(i * 12.9898) * 437.585;
  pa = pa % 1.0;
  if pa < 0.0 { pa = pa + 1.0; }
  let pr = sin(i * 78.233) * 125.432;
  pr = pr % 1.0;
  if pr < 0.0 { pr = pr + 1.0; }
  let xi = cx + cos(pa * 6.2832) * (0.15 + 0.9 * pr) * S + sin(t * 0.22 + i) * 8.0;
  let yi = cy + sin(pa * 6.2832) * (0.15 + 0.9 * pr) * S * 0.82 + cos(t * 0.17 + i * 1.7) * 7.0;
  let j = i + 1.0;
  while j < N {
    let qa = sin(j * 12.9898) * 437.585;
    qa = qa % 1.0;
    if qa < 0.0 { qa = qa + 1.0; }
    let qr = sin(j * 78.233) * 125.432;
    qr = qr % 1.0;
    if qr < 0.0 { qr = qr + 1.0; }
    let xj = cx + cos(qa * 6.2832) * (0.15 + 0.9 * qr) * S + sin(t * 0.22 + j) * 8.0;
    let yj = cy + sin(qa * 6.2832) * (0.15 + 0.9 * qr) * S * 0.82 + cos(t * 0.17 + j * 1.7) * 7.0;
    let dx = xi - xj;
    let dy = yi - yj;
    let q = dx * dx + dy * dy;
    if q < D2 {
      let cl = 1.0 - q / D2;
      hsl(0.556, 0.45, 0.80 + cl * -0.33);
      line(xi, yi, xj, yj);
    }
    j = j + 1.0;
  }
  i = i + 1.0;
}

// ---- the light: gliding being to being, flaring what it reaches ----
let slot = t * 0.55;
let fs = slot - (slot % 1.0);
let pp = slot % 1.0;
let ha = sin(fs * 91.17) * 331.73;
ha = ha % 1.0;
if ha < 0.0 { ha = ha + 1.0; }
let na = ha * N;
na = na - (na % 1.0);
let hb = sin(fs * 47.53) * 217.31;
hb = hb % 1.0;
if hb < 0.0 { hb = hb + 1.0; }
let nb = hb * N;
nb = nb - (nb % 1.0);
let dna = nb - na;
if dna < 0.5 { if dna > 0.0 - 0.5 { nb = nb + 1.0; if nb >= N { nb = 0.0; } } }

// ---- beings: water-blue cells and ember souls, breathing halos ----
let k = 0.0;
while k < N {
  let ka = sin(k * 12.9898) * 437.585;
  ka = ka % 1.0;
  if ka < 0.0 { ka = ka + 1.0; }
  let kr = sin(k * 78.233) * 125.432;
  kr = kr % 1.0;
  if kr < 0.0 { kr = kr + 1.0; }
  let xk = cx + cos(ka * 6.2832) * (0.15 + 0.9 * kr) * S + sin(t * 0.22 + k) * 8.0;
  let yk = cy + sin(ka * 6.2832) * (0.15 + 0.9 * kr) * S * 0.82 + cos(t * 0.17 + k * 1.7) * 7.0;
  let rr = 1.7 + (k % 3.0) * 0.55;
  let fl = 0.0;
  let dkb = k - nb;
  if dkb < 0.5 { if dkb > 0.0 - 0.5 { if pp > 0.72 { fl = (pp - 0.72) * 3.5; } } }
  let sv = k % 5.0;
  if sv > 1.5 {
    if sv < 2.5 {
      hsl(0.068, 0.47, 0.47);
      glow(xk, yk, (rr + 0.5) * 4.0 + fl * 10.0);
      disc(xk, yk, rr + 0.5 + fl * 2.0);
    }
  }
  if sv < 1.5 {
    hsl(0.553, 0.65, 0.375);
    glow(xk, yk, rr * 4.0 + fl * 10.0);
    hsl(0.553, 0.6, 0.42);
    disc(xk, yk, rr + fl * 2.0);
  }
  if sv > 2.5 {
    hsl(0.553, 0.65, 0.375);
    glow(xk, yk, rr * 4.0 + fl * 10.0);
    hsl(0.553, 0.6, 0.42);
    disc(xk, yk, rr + fl * 2.0);
  }
  k = k + 1.0;
}

// the travelling light itself
let aa = sin(na * 12.9898) * 437.585;
aa = aa % 1.0;
if aa < 0.0 { aa = aa + 1.0; }
let ar = sin(na * 78.233) * 125.432;
ar = ar % 1.0;
if ar < 0.0 { ar = ar + 1.0; }
let xa = cx + cos(aa * 6.2832) * (0.15 + 0.9 * ar) * S + sin(t * 0.22 + na) * 8.0;
let ya = cy + sin(aa * 6.2832) * (0.15 + 0.9 * ar) * S * 0.82 + cos(t * 0.17 + na * 1.7) * 7.0;
let ba = sin(nb * 12.9898) * 437.585;
ba = ba % 1.0;
if ba < 0.0 { ba = ba + 1.0; }
let br = sin(nb * 78.233) * 125.432;
br = br % 1.0;
if br < 0.0 { br = br + 1.0; }
let xb = cx + cos(ba * 6.2832) * (0.15 + 0.9 * br) * S + sin(t * 0.22 + nb) * 8.0;
let yb = cy + sin(ba * 6.2832) * (0.15 + 0.9 * br) * S * 0.82 + cos(t * 0.17 + nb * 1.7) * 7.0;
hsl(0.56, 0.60, 0.42);
glow(xa + (xb - xa) * pp, ya + (yb - ya) * pp, 8.0);
disc(xa + (xb - xa) * pp, ya + (yb - ya) * pp, 1.8);

0.0
