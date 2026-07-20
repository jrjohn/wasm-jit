let horizon = h * 0.60;
let mxn = mx() / w;
if mxn < 0.0 { mxn = 0.5; }
let wind = (mxn - 0.5) * 2.0;
let y = 0.0;
while y < horizon {
  let f = y / horizon;
  hsl(0.62, 0.45, 0.05 + 0.16 * (1.0 - f));
  line(0.0, y, w, y);
  y = y + 2.0;
}
let ns = 0.0;
while ns < 110.0 {
  let hx = sin(ns * 12.9) * 43758.5;
  let hy = sin(ns * 78.2) * 12345.6;
  let tw = 0.5 + 0.5 * sin(t * 2.0 + ns);
  hsl(0.6, 0.15, 0.6 + 0.35 * tw);
  disc((hx - floor(hx)) * w, (hy - floor(hy)) * horizon * 0.85, 0.7 + tw);
  ns = ns + 1.0;
}
let mnx = w * 0.72;
let mny = horizon * 0.34;
hsl(0.6, 0.25, 0.92);
glow(mnx, mny, 130.0);
disc(mnx, mny, 34.0);
hsl(0.62, 0.3, 0.7);
disc(mnx - 12.0, mny - 8.0, 6.0);
disc(mnx + 10.0, mny + 6.0, 4.0);
let bx = 0.0;
while bx < w {
  let bh = (sin(bx * 0.008) * 0.5 + 0.5) * 90.0 + (sin(bx * 0.021 + 2.0) * 0.5 + 0.5) * 40.0;
  hsl(0.66, 0.35, 0.10);
  line(bx, horizon, bx, horizon - bh);
  bx = bx + 4.0;
}
let gy = horizon;
while gy < h {
  let gf = (gy - horizon) / (h - horizon);
  hsl(0.60, 0.5, 0.04 + 0.10 * (1.0 - gf));
  line(0.0, gy, w, gy);
  gy = gy + 2.0;
}
hsl(0.6, 0.3, 0.85);
let rf = 0.0;
while rf < 40.0 {
  let ry = horizon + rf * ((h - horizon) / 40.0);
  let ph = sin(t * 1.3 + rf * 0.6) * (5.0 + rf * 0.7);
  disc(mnx + ph, ry, (13.0 - rf * 0.28) * 0.5);
  rf = rf + 1.0;
}
let ripy = horizon + 6.0;
while ripy < h {
  hsl(0.58, 0.3, 0.18);
  let amp = 3.0 + 2.0 * sin(ripy * 0.3);
  line(0.0, ripy + sin(t * 0.8 + ripy * 0.2) * amp * 0.2, w, ripy + sin(t * 0.8 + ripy * 0.2 + 3.0) * amp * 0.2);
  ripy = ripy + 16.0;
}
let boatx = w * (0.34 + 0.06 * sin(t * 0.18));
let boaty = horizon + (h - horizon) * 0.42;
hsl(0.08, 0.55, 0.14);
arc(boatx, boaty, 46.0, 0.15, 3.0);
line(boatx - 45.0, boaty, boatx + 45.0, boaty - 3.0);
hsl(0.09, 0.4, 0.08);
disc(boatx + 8.0, boaty - 16.0, 9.0);
line(boatx + 26.0, boaty - 6.0, boatx + 46.0, boaty - 26.0);
hsl(0.11, 0.95, 0.6);
glow(boatx + 30.0, boaty - 20.0, 34.0);
disc(boatx + 30.0, boaty - 20.0, 4.5);
let birdt = (t * 0.06) % 1.0;
let birdx = birdt * (w + 120.0) - 60.0;
let birdy = horizon * (0.5 + 0.12 * sin(t * 0.9));
let flap = sin(t * 7.0) * 9.0;
hsl(0.62, 0.1, 0.28);
line(birdx - 12.0, birdy + flap, birdx, birdy);
line(birdx, birdy, birdx + 12.0, birdy + flap);
let s = 0.0;
while s < 90.0 {
  let sx0 = sin(s * 3.3) * 43758.5;
  let sy0 = sin(s * 9.1) * 12345.6;
  let px = ((sx0 - floor(sx0)) * w + t * (12.0 + wind * 40.0) + sin(t + s) * 12.0) % w;
  let py = ((sy0 - floor(sy0)) * h + t * (24.0 + s)) % h;
  hsl(0.6, 0.1, 0.92);
  disc(px, py, 1.0 + (sx0 - floor(sx0)) * 1.6);
  s = s + 1.0;
}
let cd = down();
let cx = get(0.0);
let cy = get(1.0);
let ct = get(2.0);
if cd > 0.5 {
  set(0.0, mx());
  set(1.0, my());
  set(2.0, t);
  cx = mx();
  cy = my();
  ct = t;
}
let age = t - ct;
if age < 1.6 {
  if cy > horizon {
    hsl(0.58, 0.4, 0.8);
    ring(cx, cy, age * 60.0);
    ring(cx, cy, age * 38.0);
  }
}
0.0
