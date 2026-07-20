let temp = get(0.0);
let day = get(1.0);
let precip = get(2.0);
let cloud = get(3.0);
let wind = get(4.0);
let horizon = h * 0.64;
let warm = min(max((temp - 8.0) / 24.0, 0.0), 1.0);
let y = 0.0;
while y < horizon {
  let f = y / horizon;
  let sky = 0.60 - day * 0.02 + warm * 0.03;
  let lit = day * (0.34 + 0.44 * (1.0 - f)) + (1.0 - day) * (0.05 + 0.12 * (1.0 - f));
  hsl(sky, 0.3 + 0.25 * day - 0.15 * warm, lit);
  line(0.0, y, w, y);
  y = y + 2.0;
}
let sx = w * (0.5 + 0.18 * (mx() / w - 0.5) * 2.0);
let sy = horizon * (0.34 + 0.10 * sin(t * 0.15));
hsl(0.13 - (1.0 - day) * 0.04, 0.75, 0.55 + 0.3 * day);
glow(sx, sy, 140.0 + 40.0 * day);
disc(sx, sy, 30.0 - day * 4.0);
let ns = 0.0;
while ns < 80.0 * (1.0 - day) {
  let hx = sin(ns * 12.9) * 43758.5;
  let hy = sin(ns * 78.2) * 12345.6;
  hsl(0.6, 0.2, 0.92);
  disc((hx - floor(hx)) * w, (hy - floor(hy)) * horizon * 0.85, 1.0 + (hx - floor(hx)));
  ns = ns + 1.0;
}
let nc = floor(cloud / 100.0 * 8.0) + 1.0;
let c = 0.0;
while c < nc {
  let cx = ((c * 137.0) % w) + sin(t * (0.04 + wind * 0.002) + c) * 40.0;
  let cy = horizon * (0.16 + 0.40 * ((c * 41.0) % 100.0) / 100.0);
  let cs = 30.0 + (c % 4.0) * 12.0;
  hsl(0.6, 0.06, 0.45 + 0.32 * day);
  glow(cx, cy, cs * 1.7);
  disc(cx, cy, cs);
  disc(cx + cs * 0.7, cy + 5.0, cs * 0.72);
  disc(cx - cs * 0.7, cy + 5.0, cs * 0.70);
  c = c + 1.0;
}
let gy = horizon;
while gy < h {
  let gf = (gy - horizon) / (h - horizon);
  hsl(0.58, 0.35, (0.10 + 0.06 * (1.0 - gf)) * (0.5 + 0.5 * day));
  line(0.0, gy, w, gy);
  gy = gy + 2.0;
}
hsl(0.13, 0.7, (0.55 + 0.3 * day) * 0.5);
let rr = 0.0;
while rr < 26.0 {
  let ry = horizon + rr * 3.0;
  disc(sx + sin(t * 1.6 + rr) * (4.0 + rr * 0.5), ry, (14.0 - rr * 0.4) * 0.5);
  rr = rr + 1.0;
}
let bx = 0.0;
while bx < w {
  let bh = (sin(bx * 0.06) * 0.5 + 0.5) * 44.0 + (sin(bx * 0.017) * 0.5 + 0.5) * 30.0;
  hsl(0.62, 0.15, 0.07 + day * 0.05);
  line(bx, horizon + 1.0, bx, horizon - bh);
  bx = bx + 5.0;
}
let r = 0.0;
while r < precip * 4.0 {
  let rx = ((r * 137.0 + t * 60.0) % w);
  let ry0 = ((r * 53.0 + t * (360.0 + wind * 5.0)) % h);
  hsl(0.58, 0.35, 0.75);
  line(rx, ry0, rx - 1.5 - wind * 0.2, ry0 + 11.0);
  r = r + 1.0;
}
0.0
