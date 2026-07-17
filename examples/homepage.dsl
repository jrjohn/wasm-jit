// arcana.boo homepage sky — a wasm-jit `draw` seed, compiled LIVE in your
// browser by the wasm-jit compiler (itself Rust compiled to WASM).
// Everything moving on this page is this one module. Its entire world is
// 10 drawing primitives — it cannot fetch, cannot hold state, cannot read
// the page. run(t, w, h), ~60 times a second.

// the night itself — the seed paints its own sky, one world in every theme
hsl(0.62, 0.42, 0.055);
disc(w * 0.5, h * 0.5, w + h);

let cx = w * 0.785;
let cy = h * 0.38;
let R  = w * 0.135;
let am = w * 0.0075;
let dd = w * 0.083;
let D2 = dd * dd;
let N  = 13.0;
let GA = 2.39996;

// ---- the moon: one luminary, water-blue, with its halo (glow) ----
let mx = w * 0.84;
let my = h * 0.16;
let mr = h * 0.115;
hsl(0.565, 0.6, 0.66);
glow(mx, my, mr * 2.4);
glow(mx, my, mr * 1.2);
hsl(0.56, 0.55, 0.62);
disc(mx, my, mr * 0.42);

// ---- snow: a deterministic field, each flake its own fall ----
let s = 0.0;
while s < 34.0 {
  let px = sin(s * 12.9898) * 437.585;
  px = px % 1.0;
  if px < 0.0 { px = px + 1.0; }
  let vy = 0.018 + (s % 5.0) * 0.006;
  let py = px * 3.7 + t * vy;
  py = py % 1.0;
  if py < 0.0 { py = py + 1.0; }
  let sx = px * w + sin(t * 0.4 + s) * 6.0;
  hsl(0.58, 0.25, 0.6);
  disc(sx, py * h, 0.9 + (s % 3.0) * 0.35);
  s = s + 1.0;
}

// ---- links: transient binding between every near pair (繫緣) ----
let i = 0.0;
while i < N {
  let ai = i * GA;
  let ri = R * (0.26 + 0.7 * i / N);
  let xi = cx + cos(ai) * ri + sin(t * 0.3 + i) * am;
  let yi = cy + sin(ai) * ri * 0.82 + cos(t * 0.24 + i * 1.7) * am * 0.82;
  let j = i + 1.0;
  while j < N {
    let aj = j * GA;
    let rj = R * (0.26 + 0.7 * j / N);
    let xj = cx + cos(aj) * rj + sin(t * 0.3 + j) * am;
    let yj = cy + sin(aj) * rj * 0.82 + cos(t * 0.24 + j * 1.7) * am * 0.82;
    let dx = xi - xj;
    let dy = yi - yj;
    let q = dx * dx + dy * dy;
    if q < D2 {
      let cl = 1.0 - q / D2;
      hsl(0.56, 0.5, 0.18 + cl * 0.3);
      line(xi, yi, xj, yj);
    }
    j = j + 1.0;
  }
  i = i + 1.0;
}

// ---- beings: water-blue cells and ember souls, each with its halo ----
let k = 0.0;
while k < N {
  let ak = k * GA;
  let rk = R * (0.26 + 0.7 * k / N);
  let xk = cx + cos(ak) * rk + sin(t * 0.3 + k) * am;
  let yk = cy + sin(ak) * rk * 0.82 + cos(t * 0.24 + k * 1.7) * am * 0.82;
  let nh = 0.56;
  let ns = 0.5;
  let sv = k % 5.0;
  if sv > 1.5 {
    if sv < 2.5 {
      nh = 0.07;
      ns = 0.6;
    }
  }
  let breath = 1.0 + 0.18 * sin(t * 0.9 + k * 2.1);
  hsl(nh, ns, 0.55);
  glow(xk, yk, 11.0 * breath);
  hsl(nh, ns, 0.72);
  disc(xk, yk, 2.4);
  k = k + 1.0;
}

// ---- a light gliding being to being: beholding through shared light ----
let hop = t * 0.5;
let seg = hop % N;
let fs = seg - (seg % 1.0);
let pp = seg % 1.0;
let ga = fs * GA;
let ra = R * (0.26 + 0.7 * fs / N);
let xa = cx + cos(ga) * ra + sin(t * 0.3 + fs) * am;
let ya = cy + sin(ga) * ra * 0.82 + cos(t * 0.24 + fs * 1.7) * am * 0.82;
let fb = fs + 1.0;
let gb = fb * GA;
let rb = R * (0.26 + 0.7 * fb / N);
let xb = cx + cos(gb) * rb + sin(t * 0.3 + fb) * am;
let yb = cy + sin(gb) * rb * 0.82 + cos(t * 0.24 + fb * 1.7) * am * 0.82;
let lx = xa + (xb - xa) * pp;
let ly = ya + (yb - ya) * pp;
hsl(0.58, 0.7, 0.85);
glow(lx, ly, 10.0);
disc(lx, ly, 2.0);

0.0
