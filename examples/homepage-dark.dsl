// arcana.boo sky (dark theme) — a wasm-jit `draw` seed, compiled LIVE in
// your browser by the wasm-jit compiler (itself Rust compiled to WASM).
// ONE module draws BOTH skies: on a tall canvas (the hero) it composes like
// the original; on a short canvas (this panel) it centers itself.
// Its entire world is 10 drawing primitives — it cannot fetch, cannot hold
// state, cannot read the page. run(t, w, h), ~60 times a second.

// ---- composition: the seed reads its canvas and places itself ----
let hero = 0.0;
if h > 480.0 { hero = 1.0; }
let cx = w * 0.5;
let cy = h * 0.53;
let S = h * 0.36;
if w < h { S = w * 0.36; }
let mx = cx + S * 1.7;
let my = h * 0.19;
let mm = h * 0.14;
if w < h { mm = w * 0.14; }
let N = 13.0;
let D = S * 0.66;
if hero > 0.5 {
  cx = w * 0.785;
  cy = h * 0.35;
  S = h * 0.27;
  if w < h { S = w * 0.27; }
  mx = w * 0.86;
  my = h * 0.16;
  mm = h * 0.115;
  if w < h { mm = w * 0.115; }
  N = w / 120.0;
  N = N - (N % 1.0);
  if N < 9.0 { N = 9.0; }
  if N > 18.0 { N = 18.0; }
  D = S * 0.52;
}
let D2 = D * D;

// ---- the moon and its halo ----
hsl(0.557, 0.68, 0.70);
glow(mx, my, mm * 2.6);
disc(mx, my, mm * 0.45);

// ---- snow: density follows the canvas, each flake its own quiet fall ----
let SN = w * h / 34000.0;
SN = SN - (SN % 1.0);
if SN < 16.0 { SN = 16.0; }
if SN > 60.0 { SN = 60.0; }
let s = 0.0;
while s < SN {
  let px = sin(s * 12.9898) * 437.585;
  px = px % 1.0;
  if px < 0.0 { px = px + 1.0; }
  let vy = 0.016 + (s % 5.0) * 0.005;
  let py = px * 3.7 + t * vy;
  py = py % 1.0;
  if py < 0.0 { py = py + 1.0; }
  hsl(0.58, 0.30, 0.86);
  disc(px * w + sin(t * 0.4 + s) * 6.0, py * h, 0.8 + (s % 3.0) * 0.3);
  s = s + 1.0;
}

// ---- links: transient binding between every near pair (form and break) ----
let i = 0.0;
while i < N {
  let ali = sin(i * 12.9898) * 437.585;
ali = ali % 1.0;
if ali < 0.0 { ali = ali + 1.0; }
let rli = sin(i * 78.233) * 125.432;
rli = rli % 1.0;
if rli < 0.0 { rli = rli + 1.0; }
let dli = (0.05 + 0.75 * rli) * S;
if (i % 2.0) > 0.5 { dli = (0.28 + 0.77 * rli) * S; }
let xi = cx + cos(ali * 6.2832) * dli;
let yi = cy + sin(ali * 6.2832) * dli * 0.82;
let exli = xi - mx;
let eyli = yi - my;
if (exli * exli + eyli * eyli) < mm * mm * 2.1 {
  dli = dli * 0.5;
  xi = cx + cos(ali * 6.2832) * dli;
  yi = cy + sin(ali * 6.2832) * dli * 0.82;
}
xi = xi + sin(i * 1.7 + t * (0.14 + (i % 4.0) * 0.06)) * (7.0 + (i % 3.0) * 4.0);
yi = yi + cos(i * 2.3 + t * (0.11 + (i % 3.0) * 0.05)) * (6.0 + (i % 3.0) * 3.0);
  let j = i + 1.0;
  while j < N {
    let alj = sin(j * 12.9898) * 437.585;
alj = alj % 1.0;
if alj < 0.0 { alj = alj + 1.0; }
let rlj = sin(j * 78.233) * 125.432;
rlj = rlj % 1.0;
if rlj < 0.0 { rlj = rlj + 1.0; }
let dlj = (0.05 + 0.75 * rlj) * S;
if (j % 2.0) > 0.5 { dlj = (0.28 + 0.77 * rlj) * S; }
let xj = cx + cos(alj * 6.2832) * dlj;
let yj = cy + sin(alj * 6.2832) * dlj * 0.82;
let exlj = xj - mx;
let eylj = yj - my;
if (exlj * exlj + eylj * eylj) < mm * mm * 2.1 {
  dlj = dlj * 0.5;
  xj = cx + cos(alj * 6.2832) * dlj;
  yj = cy + sin(alj * 6.2832) * dlj * 0.82;
}
xj = xj + sin(j * 1.7 + t * (0.14 + (j % 4.0) * 0.06)) * (7.0 + (j % 3.0) * 4.0);
yj = yj + cos(j * 2.3 + t * (0.11 + (j % 3.0) * 0.05)) * (6.0 + (j % 3.0) * 3.0);
    let ddx = xi - xj;
    let ddy = yi - yj;
    let q = ddx * ddx + ddy * ddy;
    if q < D2 {
      let cl = 1.0 - q / D2;
      hsl(0.556, 0.35, 0.17 + cl * 0.10);
      line(xi, yi, xj, yj);
    }
    j = j + 1.0;
  }
  i = i + 1.0;
}

// ---- two lights gliding being to being (picked per time-slot) ----
let sl1 = t * 0.55 + 0.0;
let fs1 = sl1 - (sl1 % 1.0);
let pp1 = sl1 % 1.0;
let h11 = sin(fs1 * 91.17) * 331.73;
h11 = h11 % 1.0;
if h11 < 0.0 { h11 = h11 + 1.0; }
let na1 = h11 * N;
na1 = na1 - (na1 % 1.0);
let h21 = sin(fs1 * 47.53) * 217.31;
h21 = h21 % 1.0;
if h21 < 0.0 { h21 = h21 + 1.0; }
let nb1 = h21 * N;
nb1 = nb1 - (nb1 % 1.0);
let dq1 = nb1 - na1;
if dq1 < 0.5 { if dq1 > 0.0 - 0.5 { nb1 = nb1 + 1.0; if nb1 >= N { nb1 = 0.0; } } }
let sl2 = t * 0.41 + 0.37;
let fs2 = sl2 - (sl2 % 1.0);
let pp2 = sl2 % 1.0;
let h12 = sin(fs2 * 53.29) * 331.73;
h12 = h12 % 1.0;
if h12 < 0.0 { h12 = h12 + 1.0; }
let na2 = h12 * N;
na2 = na2 - (na2 % 1.0);
let h22 = sin(fs2 * 77.91) * 217.31;
h22 = h22 % 1.0;
if h22 < 0.0 { h22 = h22 + 1.0; }
let nb2 = h22 * N;
nb2 = nb2 - (nb2 % 1.0);
let dq2 = nb2 - na2;
if dq2 < 0.5 { if dq2 > 0.0 - 0.5 { nb2 = nb2 + 1.0; if nb2 >= N { nb2 = 0.0; } } }

// ---- beings: water-blue cells and ember souls, breathing halos; a light's
// ---- arrival makes its target flare ----
let k = 0.0;
while k < N {
  let abk = sin(k * 12.9898) * 437.585;
abk = abk % 1.0;
if abk < 0.0 { abk = abk + 1.0; }
let rbk = sin(k * 78.233) * 125.432;
rbk = rbk % 1.0;
if rbk < 0.0 { rbk = rbk + 1.0; }
let dbk = (0.05 + 0.75 * rbk) * S;
if (k % 2.0) > 0.5 { dbk = (0.28 + 0.77 * rbk) * S; }
let xk = cx + cos(abk * 6.2832) * dbk;
let yk = cy + sin(abk * 6.2832) * dbk * 0.82;
let exbk = xk - mx;
let eybk = yk - my;
if (exbk * exbk + eybk * eybk) < mm * mm * 2.1 {
  dbk = dbk * 0.5;
  xk = cx + cos(abk * 6.2832) * dbk;
  yk = cy + sin(abk * 6.2832) * dbk * 0.82;
}
xk = xk + sin(k * 1.7 + t * (0.14 + (k % 4.0) * 0.06)) * (7.0 + (k % 3.0) * 4.0);
yk = yk + cos(k * 2.3 + t * (0.11 + (k % 3.0) * 0.05)) * (6.0 + (k % 3.0) * 3.0);
  let rr = 1.7 + (k % 3.0) * 0.55;
  let fl = 0.0;
  let e1 = k - nb1;
  if e1 < 0.5 { if e1 > 0.0 - 0.5 { if pp1 > 0.72 { fl = (pp1 - 0.72) * 3.5; } } }
  let e2 = k - nb2;
  if e2 < 0.5 { if e2 > 0.0 - 0.5 { if pp2 > 0.72 { fl = fl + (pp2 - 0.72) * 3.5; } } }
  let sv = k % 5.0;
  if sv > 1.5 {
    if sv < 2.5 {
      hsl(0.068, 0.55, 0.62);
      glow(xk, yk, (rr + 0.5) * 4.0 + fl * 10.0);
      disc(xk, yk, rr + 0.5 + fl * 2.0);
    }
  }
  if sv < 1.5 {
    hsl(0.557, 0.68, 0.69);
    glow(xk, yk, rr * 4.0 + fl * 10.0);
    hsl(0.557, 0.68, 0.78);
    disc(xk, yk, rr + fl * 2.0);
  }
  if sv > 2.5 {
    hsl(0.557, 0.68, 0.69);
    glow(xk, yk, rr * 4.0 + fl * 10.0);
    hsl(0.557, 0.68, 0.78);
    disc(xk, yk, rr + fl * 2.0);
  }
  k = k + 1.0;
}

// ---- the travelling lights themselves ----
let apa1 = sin(na1 * 12.9898) * 437.585;
apa1 = apa1 % 1.0;
if apa1 < 0.0 { apa1 = apa1 + 1.0; }
let rpa1 = sin(na1 * 78.233) * 125.432;
rpa1 = rpa1 % 1.0;
if rpa1 < 0.0 { rpa1 = rpa1 + 1.0; }
let dpa1 = (0.05 + 0.75 * rpa1) * S;
if (na1 % 2.0) > 0.5 { dpa1 = (0.28 + 0.77 * rpa1) * S; }
let xa1 = cx + cos(apa1 * 6.2832) * dpa1;
let ya1 = cy + sin(apa1 * 6.2832) * dpa1 * 0.82;
let expa1 = xa1 - mx;
let eypa1 = ya1 - my;
if (expa1 * expa1 + eypa1 * eypa1) < mm * mm * 2.1 {
  dpa1 = dpa1 * 0.5;
  xa1 = cx + cos(apa1 * 6.2832) * dpa1;
  ya1 = cy + sin(apa1 * 6.2832) * dpa1 * 0.82;
}
xa1 = xa1 + sin(na1 * 1.7 + t * (0.14 + (na1 % 4.0) * 0.06)) * (7.0 + (na1 % 3.0) * 4.0);
ya1 = ya1 + cos(na1 * 2.3 + t * (0.11 + (na1 % 3.0) * 0.05)) * (6.0 + (na1 % 3.0) * 3.0);
let apb1 = sin(nb1 * 12.9898) * 437.585;
apb1 = apb1 % 1.0;
if apb1 < 0.0 { apb1 = apb1 + 1.0; }
let rpb1 = sin(nb1 * 78.233) * 125.432;
rpb1 = rpb1 % 1.0;
if rpb1 < 0.0 { rpb1 = rpb1 + 1.0; }
let dpb1 = (0.05 + 0.75 * rpb1) * S;
if (nb1 % 2.0) > 0.5 { dpb1 = (0.28 + 0.77 * rpb1) * S; }
let xb1 = cx + cos(apb1 * 6.2832) * dpb1;
let yb1 = cy + sin(apb1 * 6.2832) * dpb1 * 0.82;
let expb1 = xb1 - mx;
let eypb1 = yb1 - my;
if (expb1 * expb1 + eypb1 * eypb1) < mm * mm * 2.1 {
  dpb1 = dpb1 * 0.5;
  xb1 = cx + cos(apb1 * 6.2832) * dpb1;
  yb1 = cy + sin(apb1 * 6.2832) * dpb1 * 0.82;
}
xb1 = xb1 + sin(nb1 * 1.7 + t * (0.14 + (nb1 % 4.0) * 0.06)) * (7.0 + (nb1 % 3.0) * 4.0);
yb1 = yb1 + cos(nb1 * 2.3 + t * (0.11 + (nb1 % 3.0) * 0.05)) * (6.0 + (nb1 % 3.0) * 3.0);
let vx1 = xb1 - xa1;
let vy1 = yb1 - ya1;
if (vx1 * vx1 + vy1 * vy1) < D2 * 1.44 {
  hsl(0.575, 0.72, 0.86);
  glow(xa1 + vx1 * pp1, ya1 + vy1 * pp1, 8.0);
  disc(xa1 + vx1 * pp1, ya1 + vy1 * pp1, 1.8);
}
let apa2 = sin(na2 * 12.9898) * 437.585;
apa2 = apa2 % 1.0;
if apa2 < 0.0 { apa2 = apa2 + 1.0; }
let rpa2 = sin(na2 * 78.233) * 125.432;
rpa2 = rpa2 % 1.0;
if rpa2 < 0.0 { rpa2 = rpa2 + 1.0; }
let dpa2 = (0.05 + 0.75 * rpa2) * S;
if (na2 % 2.0) > 0.5 { dpa2 = (0.28 + 0.77 * rpa2) * S; }
let xa2 = cx + cos(apa2 * 6.2832) * dpa2;
let ya2 = cy + sin(apa2 * 6.2832) * dpa2 * 0.82;
let expa2 = xa2 - mx;
let eypa2 = ya2 - my;
if (expa2 * expa2 + eypa2 * eypa2) < mm * mm * 2.1 {
  dpa2 = dpa2 * 0.5;
  xa2 = cx + cos(apa2 * 6.2832) * dpa2;
  ya2 = cy + sin(apa2 * 6.2832) * dpa2 * 0.82;
}
xa2 = xa2 + sin(na2 * 1.7 + t * (0.14 + (na2 % 4.0) * 0.06)) * (7.0 + (na2 % 3.0) * 4.0);
ya2 = ya2 + cos(na2 * 2.3 + t * (0.11 + (na2 % 3.0) * 0.05)) * (6.0 + (na2 % 3.0) * 3.0);
let apb2 = sin(nb2 * 12.9898) * 437.585;
apb2 = apb2 % 1.0;
if apb2 < 0.0 { apb2 = apb2 + 1.0; }
let rpb2 = sin(nb2 * 78.233) * 125.432;
rpb2 = rpb2 % 1.0;
if rpb2 < 0.0 { rpb2 = rpb2 + 1.0; }
let dpb2 = (0.05 + 0.75 * rpb2) * S;
if (nb2 % 2.0) > 0.5 { dpb2 = (0.28 + 0.77 * rpb2) * S; }
let xb2 = cx + cos(apb2 * 6.2832) * dpb2;
let yb2 = cy + sin(apb2 * 6.2832) * dpb2 * 0.82;
let expb2 = xb2 - mx;
let eypb2 = yb2 - my;
if (expb2 * expb2 + eypb2 * eypb2) < mm * mm * 2.1 {
  dpb2 = dpb2 * 0.5;
  xb2 = cx + cos(apb2 * 6.2832) * dpb2;
  yb2 = cy + sin(apb2 * 6.2832) * dpb2 * 0.82;
}
xb2 = xb2 + sin(nb2 * 1.7 + t * (0.14 + (nb2 % 4.0) * 0.06)) * (7.0 + (nb2 % 3.0) * 4.0);
yb2 = yb2 + cos(nb2 * 2.3 + t * (0.11 + (nb2 % 3.0) * 0.05)) * (6.0 + (nb2 % 3.0) * 3.0);
let vx2 = xb2 - xa2;
let vy2 = yb2 - ya2;
if (vx2 * vx2 + vy2 * vy2) < D2 * 1.44 {
  hsl(0.575, 0.72, 0.86);
  glow(xa2 + vx2 * pp2, ya2 + vy2 * pp2, 8.0);
  disc(xa2 + vx2 * pp2, ya2 + vy2 * pp2, 1.8);
}

0.0
