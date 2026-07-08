// boat — the default SOUL of the boat package (Tier 2).
// Reads the water around itself and drifts toward the deeper side (a boat
// follows the current it sits in), with a slow sway. It never sees the whole
// river — only its four neighbors. The host clamps every step.

@external("env", "sin") declare function sin(x: f64): f64;
@external("env", "cos") declare function cos(x: f64): f64;
@external("env", "get") declare function get(i: f64): f64;
@external("env", "set") declare function set(i: f64, v: f64): void;
@external("env", "fr")  declare function fr(c: f64, x: f64, y: f64): f64;
@external("env", "mv")  declare function mv(dx: f64, dy: f64): void;

export function run(t: f64, ex: f64, ey: f64): f64 {
  const here: f64 = fr(1.0, ex, ey);
  const east: f64 = fr(1.0, ex + 1.0, ey);
  const west: f64 = fr(1.0, ex - 1.0, ey);
  const north: f64 = fr(1.0, ex, ey - 1.0);
  const south: f64 = fr(1.0, ex, ey + 1.0);

  let dx: f64 = 0.0;
  let dy: f64 = 0.0;
  if (east > west + 0.02) { dx = 0.012; } else if (west > east + 0.02) { dx = -0.012; }
  if (south > north + 0.02) { dy = 0.012; } else if (north > south + 0.02) { dy = -0.012; }

  // the slow sway of a moored hull
  dx += sin(t * 0.4) * 0.014;
  dy += cos(t * 0.31) * 0.006;
  mv(dx, dy);
  return here;
}
