// Smiling Buddha — AssemblyScript (Tier 2 seed).
// Contrast with the home-DSL version: here we have const, typed params, an extracted function, for, a real if.
// The output is a standard .wasm; import section = {env.sin,cos,hue,disc,ring,arc,line} (the draw ABI),
// entering the sandbox through the same wasm-jit import audit — the language changed, the fence didn't.

// —— host capabilities (@external pins each import's module::name to env.*, aligned with the draw ABI's grant list) ——
@external("env", "sin")  declare function sin(x: f64): f64;
@external("env", "cos")  declare function cos(x: f64): f64;
@external("env", "hue")  declare function hue(v: f64): void;
@external("env", "disc") declare function disc(x: f64, y: f64, r: f64): void;
@external("env", "ring") declare function ring(x: f64, y: f64, r: f64): void;
@external("env", "arc")  declare function arc(x: f64, y: f64, r: f64, a0: f64, a1: f64): void;
@external("env", "line") declare function line(x1: f64, y1: f64, x2: f64, y2: f64): void;

// What the DSL can't do: extract "one closed eye" into a function and call it once per eye.
function closedEye(cx: f64, cy: f64, r: f64): void {
  arc(cx, cy, r, 3.35, 6.05);
}

// The exported kernel. ABI matches the home-DSL version: run(t, w, h) -> f64.
export function run(t: f64, w: f64, h: f64): f64 {
  const cx: f64 = w * 0.5;
  const cy: f64 = h * 0.56;
  const r: f64 = h * 0.27;

  // halo (breathing)
  hue(0.13);
  ring(cx, cy - r * 0.18, r * 1.55 + sin(t) * 6.0);

  // ushnisha (topknot)
  hue(0.09);
  disc(cx, cy - r * 1.04, r * 0.2);

  // face
  hue(0.1);
  disc(cx, cy, r);

  // long ears (for loop + a real if, showing control flow; the DSL version is line-by-line)
  hue(0.1);
  for (let i: i32 = 0; i < 2; i++) {
    const side: f64 = i == 0 ? -1.0 : 1.0;
    line(cx + side * r * 1.06, cy - r * 0.15, cx + side * r * 1.06, cy + r * 0.5);
  }

  // urna (forehead dot)
  hue(0.99);
  disc(cx, cy - r * 0.28, r * 0.045);

  // closed eyes + smile (breathing)
  hue(0.02);
  closedEye(cx - r * 0.36, cy - r * 0.02, r * 0.17);
  closedEye(cx + r * 0.36, cy - r * 0.02, r * 0.17);
  arc(cx, cy + r * 0.22, r * 0.46 + sin(t * 2.0) * 3.0, 0.45, 2.69);

  return 0.0;
}
