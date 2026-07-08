// fisherman — the default SOUL of the fisherman package (Tier 2).
// Compiled by asc to a standard .wasm; enters the world through the entity
// import audit (env.{sin,cos,get,set,fr,mv} only). Stillness is the point:
// while riding a boat his mv is ignored anyway (the carrier carries); free-
// standing, he only shifts his weight, almost imperceptibly.

@external("env", "sin") declare function sin(x: f64): f64;
@external("env", "cos") declare function cos(x: f64): f64;
@external("env", "get") declare function get(i: f64): f64;
@external("env", "set") declare function set(i: f64, v: f64): void;
@external("env", "fr")  declare function fr(c: f64, x: f64, y: f64): f64;
@external("env", "mv")  declare function mv(dx: f64, dy: f64): void;

export function run(t: f64, ex: f64, ey: f64): f64 {
  // beholding the water is the whole occupation
  const water: f64 = fr(1.0, ex, ey);
  if (get(0.0) == 0.0) {
    set(0.0, t); // remember when he sat down
  }
  // a weight-shift measured in millimeters
  mv(sin(t * 0.9) * 0.002, 0.0);
  return water;
}
