// Indra's net — a wasm-jit `draw` seed. Pure run(t,w,h): beings drift, links
// form and break by proximity (繫緣), a light glides from being to being.
// Its entire world is 9 primitives — it cannot fetch, cannot hold state.

let cx = w * 0.785;
let cy = h * 0.35;
let R  = w * 0.135;
let am = w * 0.0075;
let dd = w * 0.086;
let D2 = dd * dd;
let N  = 13.0;
let GA = 2.39996;

// ---- links: transient binding between every near pair ----
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
      hsl(0.56, 0.5, 0.2 + cl * 0.34);
      line(xi, yi, xj, yj);
    }
    j = j + 1.0;
  }
  i = i + 1.0;
}

// ---- beings: cells (water-blue) and souls (ember) ----
let k = 0.0;
while k < N {
  let ak = k * GA;
  let rk = R * (0.26 + 0.7 * k / N);
  let xk = cx + cos(ak) * rk + sin(t * 0.3 + k) * am;
  let yk = cy + sin(ak) * rk * 0.82 + cos(t * 0.24 + k * 1.7) * am * 0.82;
  let sv = k % 5.0;
  let nh = 0.56;
  let ns = 0.46;
  if sv > 1.5 {
    if sv < 2.5 {
      nh = 0.09;
      ns = 0.55;
    }
  }
  hsl(nh, ns, 0.34);
  ring(xk, yk, 6.5);
  hsl(nh, ns, 0.68);
  disc(xk, yk, 2.5);
  k = k + 1.0;
}

// ---- a light gliding being to being (beholding through shared light) ----
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
hsl(0.58, 0.65, 0.82);
disc(xa + (xb - xa) * pp, ya + (yb - ya) * pp, 2.4);

0.0
