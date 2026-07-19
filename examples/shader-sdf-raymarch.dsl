// L4 landed — the L4-falsification math, now running where it belongs: this
// seed IS the fragment shader. Every pixel raymarches the same SDF the DSL
// proved it could express (sphere on a checkered plane, one light), but at
// full resolution on the GPU instead of a coarse disc grid. Narrowest fence
// of all: math + colour + pointer; a pixel has no memory and no reach.

let u = (x - w * 0.5) / h;
let v = (y - h * 0.5) / h;

// camera at (0, 1.6, 4.2) looking down -z; ray per pixel
let ox = 0.0;
let oy = 1.6;
let oz = 4.2;
let dx = u;
let dy = 0.0 - v;
let dz = 0.0 - 1.2;
let dl = sqrt(dx * dx + dy * dy + dz * dz);
dx = dx / dl;
dy = dy / dl;
dz = dz / dl;

// march: sphere at (0,1,0) r=1, plane y=0
let t2 = 0.0;
let hit = 0.0;
let i = 0.0;
let px = 0.0;
let py = 0.0;
let pz = 0.0;
while i < 64.0 {
  if hit < 0.5 {
    px = ox + dx * t2;
    py = oy + dy * t2;
    pz = oz + dz * t2;
    let ds = sqrt(px * px + (py - 1.0) * (py - 1.0) + pz * pz) - 1.0;
    let dp = py;
    let d = min(ds, dp);
    if d < 0.001 { hit = 1.0; }
    if d >= 0.001 { t2 = t2 + d; }
    if t2 > 40.0 { i = 64.0; }
  }
  i = i + 1.0;
}

// shade
let lx = 0.45;
let ly = 0.8;
let lz = 0.35;
if hit > 0.5 {
  let ds2 = sqrt(px * px + (py - 1.0) * (py - 1.0) + pz * pz) - 1.0;
  let dp2 = py;
  if ds2 < dp2 {
    // sphere: normal = (p - c)
    let nx = px;
    let ny = py - 1.0;
    let nz = pz;
    let lit = max(nx * lx + ny * ly + nz * lz, 0.0);
    rgb(0.2 + 0.75 * lit, 0.06 + 0.2 * lit, 0.08 + 0.15 * lit);
  }
  if ds2 >= dp2 {
    // plane: checker + a soft shadow under the sphere
    let cx = floor(px * 1.2) + floor(pz * 1.2);
    let ck = cx - floor(cx / 2.0) * 2.0;
    let base = 0.28 + ck * 0.34;
    let sh = min(sqrt(px * px + pz * pz), 1.4) / 1.4;
    let k = base * (0.45 + 0.55 * sh);
    rgb(k * 0.9, k, k * 1.05);
  }
}
if hit < 0.5 {
  // sky gradient
  let g = max(0.0 - dy, 0.0);
  rgb(0.05 + g * 0.1, 0.09 + g * 0.14, 0.16 + g * 0.25);
}
0.0
