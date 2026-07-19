//! wasm-jit — runtime script→WASM codegen; the browser engine JITs it, a
//! capability sandbox contains it.
//!
//! Lineage: the idea began from wanting a *sandboxed* runtime scripting
//! language and finding that a tree-walking interpreter (we prototyped
//! against Rhai — thank you to that project for the spark) trades native
//! speed for its sandbox. wasm-jit keeps both: generate a tiny WASM module,
//! let the browser engine JIT it to native speed, and gate it with a
//! capability import table.
//!
//! Exposed to JS (benchmark page, index.html):
//! - `compile_to_wasm(src)`   : DSL source → .wasm module bytes, `run(n)->f64`, no imports
//! - `transpile_to_js(src)`   : same AST → JS function body (V8 JS-JIT reference lane)
//! - `native_kernel(n)`       : default kernel hand-written in Rust (AOT ceiling lane)
//!
//! Exposed to JS (canvas page, canvas.html):
//! - `compile_kernel_wasm(src)`: DSL → .wasm cell, `run(t,i,hx,hy)->hue`,
//!    capabilities (imports): env.sin, env.cos, env.out(x,y)

pub mod audit;
pub mod codegen;
pub mod parser;

#[cfg(feature = "js-api")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_to_wasm(src: &str) -> Result<Vec<u8>, JsError> {
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile(&prog).map_err(|e| JsError::new(&e))
}

/// Canvas kernel: `run(t, i, hx, hy) -> hue`, imports env.{sin, cos, out}.
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_kernel_wasm(src: &str) -> Result<Vec<u8>, JsError> {
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile_kernel(&prog).map_err(|e| JsError::new(&e))
}

/// Free-drawing kernel: `run(t, w, h)`, capabilities = 2D drawing primitives
/// PLUS the interaction loop (docs §21). No widgets required — the primitive
/// vocabulary is complete for 2D (SVG's ~10 path commands can express any
/// shape); any shape is just the generated script composing those primitives.
///
/// The interaction faculties turn a drawing into a live app without widening
/// its reach: `mx`/`my`/`down` read the pointer (the host owns the mouse; the
/// cell only SEES a position, it can't capture events elsewhere), and
/// `get`/`set` are a host-owned 32-slot f64 data root the cell attenuates into
/// — the host keeps it across a hot-patch, so a patched cell remembers what the
/// interaction accumulated (a trail, a click count). Richness unbounded, reach
/// fixed: the cell still cannot fetch, read the page, or touch any other state.
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_draw_wasm(src: &str) -> Result<Vec<u8>, JsError> {
    codegen::compile_with(&parser::parse(src).map_err(|e| JsError::new(&e))?, &DRAW_PARAMS, &DRAW_IMPORTS)
        .map_err(|e| JsError::new(&e))
}

/// The draw ABI, shared so the native validator (gen-server) and the browser
/// compiler mint byte-identical modules. `run(t, w, h)`; imports below.
pub const DRAW_PARAMS: [&str; 3] = ["t", "w", "h"];
pub const DRAW_IMPORTS: [codegen::HostFn; 15] = [
    codegen::HostFn { name: "sin", n_args: 1, returns: true },
    codegen::HostFn { name: "cos", n_args: 1, returns: true },
    codegen::HostFn { name: "hue", n_args: 1, returns: false },   // set colour by hue (fixed sat/light)
    codegen::HostFn { name: "rgb", n_args: 3, returns: false },   // set colour by r,g,b (0..1 each)
    codegen::HostFn { name: "hsl", n_args: 3, returns: false },   // set colour by hue,sat,light (0..1) — natural tones, shadows
    codegen::HostFn { name: "disc", n_args: 3, returns: false },  // filled circle (x,y,r)
    codegen::HostFn { name: "ring", n_args: 3, returns: false },  // outlined circle (x,y,r)
    codegen::HostFn { name: "arc", n_args: 5, returns: false },   // arc (x,y,r,a0,a1)
    codegen::HostFn { name: "line", n_args: 4, returns: false },  // line (x1,y1,x2,y2)
    codegen::HostFn { name: "glow", n_args: 3, returns: false },  // soft radial halo (x,y,r) in the current colour
    // ── the interaction loop (§21): events in, host-owned state ──
    codegen::HostFn { name: "mx", n_args: 0, returns: true },     // pointer x in canvas px (-1 when the pointer is away)
    codegen::HostFn { name: "my", n_args: 0, returns: true },     // pointer y in canvas px (-1 when away)
    codegen::HostFn { name: "down", n_args: 0, returns: true },   // 1.0 while the pointer is pressed, else 0.0
    codegen::HostFn { name: "get", n_args: 1, returns: true },    // read the host data root, slot 0..31 (survives a hot-patch)
    codegen::HostFn { name: "set", n_args: 2, returns: false },   // write the host data root, slot 0..31
];

/// UI-logic cell for the live-generation demo: `run(x) -> f64`, capabilities
/// env.{sin, cos, get, set} — get/set is a host-granted 32-slot f64 store so
/// multi-input logic works (input cells persist to slots, computed cells read
/// them). Fuel-metered at 200k. The same contract the gen-server validates
/// against natively — browser and server compile identical modules.
/// The UI-cell ABI, shared crate-side. get/set = 32 scalar slots (the fast
/// shared store); ld/sd = a host-owned 4096-slot f64 array — the COLLECTION
/// store (lists, queues, tables), same closure pattern as slots, so the fence
/// and the persistence story extend without touching the memory-audit rule.
pub const UI_PARAMS: [&str; 1] = ["x"];
pub const UI_IMPORTS: [codegen::HostFn; 6] = [
    codegen::HostFn { name: "sin", n_args: 1, returns: true },
    codegen::HostFn { name: "cos", n_args: 1, returns: true },
    codegen::HostFn { name: "get", n_args: 1, returns: true },
    codegen::HostFn { name: "set", n_args: 2, returns: false },
    codegen::HostFn { name: "ld", n_args: 1, returns: true },  // read the 4096-slot collection store
    codegen::HostFn { name: "sd", n_args: 2, returns: false }, // write it
];

#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_ui_cell_wasm(src: &str) -> Result<Vec<u8>, JsError> {
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile_with_opts(
        &prog,
        &UI_PARAMS,
        &UI_IMPORTS,
        codegen::CompileOpts { fuel: Some(200_000), memory_pages: None },
    )
    .map_err(|e| JsError::new(&e))
}

/// World cell for the Field (docs §19): `run(t, gw, gh) -> f64`, capabilities
/// env.{sin, cos, get, set, fr, fw} — fr/fw are the shared-field (collective
/// karma) read/write pair; region scoping happens in the host closures.
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_field_cell_wasm(src: &str) -> Result<Vec<u8>, JsError> {
    use codegen::HostFn;
    const PARAMS: [&str; 3] = ["t", "gw", "gh"];
    const IMPORTS: [HostFn; 6] = [
        HostFn { name: "sin", n_args: 1, returns: true },
        HostFn { name: "cos", n_args: 1, returns: true },
        HostFn { name: "get", n_args: 1, returns: true },
        HostFn { name: "set", n_args: 2, returns: false },
        HostFn { name: "fr", n_args: 3, returns: true },
        HostFn { name: "fw", n_args: 4, returns: false },
    ];
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile_with_opts(
        &prog,
        &PARAMS,
        &IMPORTS,
        codegen::CompileOpts { fuel: Some(2_000_000), memory_pages: None },
    )
    .map_err(|e| JsError::new(&e))
}

/// The entity ABI, shared so the native validator (gen-server) and the browser
/// compiler agree on the same capability set. `run(t, ex, ey) -> f64`; imports
/// below. `bind`/`unbind` are §19's paired faculties: ENTER a condition — ride
/// the i-th nearest being (host clamps reach, forbids ride cycles) — and LEAVE
/// it. A rider's own mv() is ignored; the carrier carries.
pub const ENTITY_PARAMS: [&str; 3] = ["t", "ex", "ey"];
pub const ENTITY_IMPORTS: [codegen::HostFn; 10] = [
    codegen::HostFn { name: "sin", n_args: 1, returns: true },
    codegen::HostFn { name: "cos", n_args: 1, returns: true },
    codegen::HostFn { name: "get", n_args: 1, returns: true },
    codegen::HostFn { name: "set", n_args: 2, returns: false },
    codegen::HostFn { name: "fr", n_args: 3, returns: true },
    codegen::HostFn { name: "mv", n_args: 2, returns: false },
    codegen::HostFn { name: "unbind", n_args: 0, returns: false }, // §19: leave the condition
    codegen::HostFn { name: "bind", n_args: 1, returns: true },    // §19: enter one — ride the i-th nearest being (1.0 if boarded, 0.0 if refused)
    codegen::HostFn { name: "rise", n_args: 1, returns: false },   // the vertical faculty: request a change in altitude (host clamps)
    codegen::HostFn { name: "other", n_args: 2, returns: true },   // sense the i-th nearest being: other(i,0)=dist, (i,1)=dx, (i,2)=dy
];

/// Inhabitant (entity) behavior for the Field: `run(t, ex, ey) -> f64`.
/// Capabilities env.{sin, cos, get, set, fr, mv, unbind, bind, rise, other} — fr
/// reads the shared field, mv REQUESTS movement (host clamps speed/bounds;
/// position is host-owned), bind()/unbind() enter and leave a riding condition
/// (§19: the freedom to become one with a thing, and to leave it).
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_entity_wasm(src: &str) -> Result<Vec<u8>, JsError> {
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile_with_opts(
        &prog,
        &ENTITY_PARAMS,
        &ENTITY_IMPORTS,
        codegen::CompileOpts { fuel: Some(200_000), memory_pages: None },
    )
    .map_err(|e| JsError::new(&e))
}

/// The skin ABI, shared crate-side so the native validator and browser compiler
/// agree. `run(px, py, s, t, nx, ny) -> f64`; nx,ny (-1..1) point to the nearest
/// other being. Capabilities = the 2D drawing primitives PLUS `st(i)` — a
/// READ-ONLY view of the being's published state (docs §20.2): the soul writes
/// its slots via set(), the skin reads them via st(), so intent (the mind)
/// reaches form (the body). The skin still cannot fetch, read the page, touch
/// any other being, or write anything — richness up, reach fixed.
pub const SKIN_PARAMS: [&str; 6] = ["px", "py", "s", "t", "nx", "ny"];
pub const SKIN_IMPORTS: [codegen::HostFn; 10] = [
    codegen::HostFn { name: "sin", n_args: 1, returns: true },
    codegen::HostFn { name: "cos", n_args: 1, returns: true },
    codegen::HostFn { name: "hue", n_args: 1, returns: false },
    codegen::HostFn { name: "rgb", n_args: 3, returns: false }, // colour by r,g,b (0..1)
    codegen::HostFn { name: "hsl", n_args: 3, returns: false }, // colour by hue,sat,light (0..1) — skin tones, shading
    codegen::HostFn { name: "disc", n_args: 3, returns: false },
    codegen::HostFn { name: "ring", n_args: 3, returns: false },
    codegen::HostFn { name: "arc", n_args: 5, returns: false },
    codegen::HostFn { name: "line", n_args: 4, returns: false },
    codegen::HostFn { name: "st", n_args: 1, returns: true },   // read the being's published state slot (soul writes, skin reads)
];

/// Runtime-generated SKIN: a novel inhabitant's *look* (a lotus, a deer, a tent)
/// generated at runtime, entering through the same drawing-primitive fence as
/// everything else (docs §20.1). px,py = the entity's canvas center; s = canvas
/// px per grid unit; t = seconds. See `SKIN_IMPORTS` for the capability set.
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_skin_wasm(src: &str) -> Result<Vec<u8>, JsError> {
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile_with_opts(
        &prog,
        &SKIN_PARAMS,
        &SKIN_IMPORTS,
        codegen::CompileOpts { fuel: Some(300_000), memory_pages: None },
    )
    .map_err(|e| JsError::new(&e))
}

/// The DRAW3D ABI (§22 — the seed writes the SCENE): 3D composed the way 2D is,
/// one dimension up. `run(t, w, h)` every frame; the cell places primitives in
/// WORLD coordinates and the host owns everything geometric that could go wrong
/// — camera matrices, depth, projection, lighting are host law, so a seed can
/// never write a matrix and the model never has to. y is up.
pub const DRAW3D_PARAMS: [&str; 3] = ["t", "w", "h"];
pub const DRAW3D_IMPORTS: [codegen::HostFn; 30] = [
    codegen::HostFn { name: "sin", n_args: 1, returns: true },
    codegen::HostFn { name: "cos", n_args: 1, returns: true },
    codegen::HostFn { name: "hue", n_args: 1, returns: false },  // colour verbs — same names as 2D
    codegen::HostFn { name: "rgb", n_args: 3, returns: false },
    codegen::HostFn { name: "hsl", n_args: 3, returns: false },
    codegen::HostFn { name: "cam", n_args: 6, returns: false },  // eye (x,y,z) looking at (tx,ty,tz); omit → host orbit camera
    codegen::HostFn { name: "light", n_args: 3, returns: false },// directional light (dx,dy,dz)
    // ── primitives, drawn in the CURRENT FRAME (see the transform stack) ──
    codegen::HostFn { name: "sphere", n_args: 4, returns: false },// (x,y,z,r)
    codegen::HostFn { name: "box", n_args: 6, returns: false },  // (x,y,z, sx,sy,sz) full sizes
    codegen::HostFn { name: "tri", n_args: 9, returns: false },  // arbitrary triangle — the escape hatch
    codegen::HostFn { name: "cyl", n_args: 2, returns: false },  // (r,h) standing at the frame origin, +y up
    codegen::HostFn { name: "cone", n_args: 2, returns: false }, // (r,h) same stance
    // ── the transform stack (3D-1): hierarchy without matrices — the host
    //    composes frames; a seed can only push/move/turn/scale and pop back.
    //    The stack resets every frame; overflow is ignored; pop on empty is a
    //    no-op (an unbalanced seed cannot corrupt the world). depth ≤ 64. ──
    codegen::HostFn { name: "push", n_args: 0, returns: false },
    codegen::HostFn { name: "pop", n_args: 0, returns: false },
    codegen::HostFn { name: "move", n_args: 3, returns: false },
    codegen::HostFn { name: "rotx", n_args: 1, returns: false }, // radians
    codegen::HostFn { name: "roty", n_args: 1, returns: false },
    codegen::HostFn { name: "rotz", n_args: 1, returns: false },
    codegen::HostFn { name: "scale", n_args: 1, returns: false },// uniform
    // ── matter (3D-2): how the current colour meets light ──
    codegen::HostFn { name: "shine", n_args: 1, returns: false },// 0..1 specular strength
    codegen::HostFn { name: "lum", n_args: 1, returns: false },  // 0..1 self-luminous (a sun, a lantern)
    codegen::HostFn { name: "pat", n_args: 1, returns: false },  // 0 solid · 1 checker · 2 stripes · 3 speckle
    // ── the interaction loop (3D-2), same faculties the 2D draw has ──
    codegen::HostFn { name: "mx", n_args: 0, returns: true },
    codegen::HostFn { name: "my", n_args: 0, returns: true },
    codegen::HostFn { name: "down", n_args: 0, returns: true },
    codegen::HostFn { name: "get", n_args: 1, returns: true },
    codegen::HostFn { name: "set", n_args: 2, returns: false },
    // ── the app wires (3D-3): a 3D scene as a LIVE PANEL inside a UI. In a
    //    scene3d node bv(i) reads bound cell outputs and emit(v) fires the
    //    node's on_input; standalone, bv reads 0 and emit is a no-op. ──
    codegen::HostFn { name: "bv", n_args: 1, returns: true },
    codegen::HostFn { name: "emit", n_args: 1, returns: false },
    // pick(): the ordinal (draw order, 0-based) of the primitive under the
    // pointer LAST frame, -1 if none — hover/click selection as host law (the
    // host ray-casts; the seed only reads a number).
    codegen::HostFn { name: "pick", n_args: 0, returns: true },
];

/// Compile a 3D scene seed against the draw3d ABI.
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_draw3d_wasm(src: &str) -> Result<Vec<u8>, JsError> {
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile_with_opts(
        &prog,
        &DRAW3D_PARAMS,
        &DRAW3D_IMPORTS,
        codegen::CompileOpts { fuel: Some(5_000_000), memory_pages: None },
    )
    .map_err(|e| JsError::new(&e))
}


/// The SOUND ABI — the audio shader (§24, the ear's skin): where surface
/// "shader" runs the seed once per PIXEL, surface "sound" runs it once per
/// AUDIO SAMPLE (44100×/s in an AudioWorklet). run(t) -> one sample in -1..1.
/// The narrowest fence with a memory: pure math + 32 slots for envelopes and
/// sequencing. No pointer, no drawing, no reach — and the master volume is
/// host law, outside the cell's world entirely.
pub const SOUND_PARAMS: [&str; 1] = ["t"];
pub const SOUND_IMPORTS: [codegen::HostFn; 5] = [
    codegen::HostFn { name: "sin", n_args: 1, returns: true },
    codegen::HostFn { name: "cos", n_args: 1, returns: true },
    codegen::HostFn { name: "get", n_args: 1, returns: true },
    codegen::HostFn { name: "set", n_args: 2, returns: false },
    codegen::HostFn { name: "noise", n_args: 0, returns: true }, // white noise in -1..1 — filter it (get/set) for rain/wind/waterfall/snow
];

/// Compile a sound seed. Fuel is small and per-call: at 44.1kHz a runaway loop
/// inside one sample would hang the audio thread — the meter traps it instead.
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_sound_wasm(src: &str) -> Result<Vec<u8>, JsError> {
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile_with_opts(
        &prog,
        &SOUND_PARAMS,
        &SOUND_IMPORTS,
        codegen::CompileOpts { fuel: Some(4096), memory_pages: None },
    )
    .map_err(|e| JsError::new(&e))
}

/// The GROWN-WIDGET ABI (詞彙自生成): a widget the fixed vocabulary lacks — a
/// knob, a clock, a heatmap cell — enters as a fenced cell through the same
/// drawing-primitive gate as a grown skin. `run(t, w, h)` every frame on its
/// OWN small canvas. Beyond the drawing/pointer/slot faculties it has exactly
/// two wires into the app, both host-mediated:
///   bv(i)   — READ the i-th bound value (the host feeds outputs of bind/bind_values)
///   emit(v) — RAISE its voice: the host fires the widget's on_input cell with v
///             (coalesced to one emission per frame — one voice per instant)
/// It still cannot touch the DOM, the page, the network, or any other widget.
pub const WIDGET_PARAMS: [&str; 3] = ["t", "w", "h"];
pub const WIDGET_IMPORTS: [codegen::HostFn; 17] = [
    codegen::HostFn { name: "sin", n_args: 1, returns: true },
    codegen::HostFn { name: "cos", n_args: 1, returns: true },
    codegen::HostFn { name: "hue", n_args: 1, returns: false },
    codegen::HostFn { name: "rgb", n_args: 3, returns: false },
    codegen::HostFn { name: "hsl", n_args: 3, returns: false },
    codegen::HostFn { name: "disc", n_args: 3, returns: false },
    codegen::HostFn { name: "ring", n_args: 3, returns: false },
    codegen::HostFn { name: "arc", n_args: 5, returns: false },
    codegen::HostFn { name: "line", n_args: 4, returns: false },
    codegen::HostFn { name: "glow", n_args: 3, returns: false },
    codegen::HostFn { name: "mx", n_args: 0, returns: true },   // pointer x in ITS canvas px (-1 away)
    codegen::HostFn { name: "my", n_args: 0, returns: true },
    codegen::HostFn { name: "down", n_args: 0, returns: true },
    codegen::HostFn { name: "get", n_args: 1, returns: true },  // private 32-slot root (drag state, animation phase)
    codegen::HostFn { name: "set", n_args: 2, returns: false },
    codegen::HostFn { name: "bv", n_args: 1, returns: true },   // bound value in (host-fed)
    codegen::HostFn { name: "emit", n_args: 1, returns: false },// event out (host-routed, 1/frame)
];

/// Compile a grown widget's seed against the widget ABI.
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_widget_wasm(src: &str) -> Result<Vec<u8>, JsError> {
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile_with_opts(
        &prog,
        &WIDGET_PARAMS,
        &WIDGET_IMPORTS,
        codegen::CompileOpts { fuel: Some(300_000), memory_pages: None },
    )
    .map_err(|e| JsError::new(&e))
}

/// Browser-side Tier-2 fence for inhabitant souls: audit that an externally
/// compiled behavior module's imports ⊆ the entity grant list (env.{sin, cos,
/// get, set, fr, mv}) BEFORE instantiating it. The soul of a packaged
/// inhabitant enters through this gate — plugin without trust.
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn audit_entity_bytes(bytes: &[u8]) -> Result<(), JsError> {
    use audit::Grant;
    const GRANTS: [Grant; 10] = [
        Grant { module: "env", name: "sin" },
        Grant { module: "env", name: "cos" },
        Grant { module: "env", name: "get" },
        Grant { module: "env", name: "set" },
        Grant { module: "env", name: "fr" },
        Grant { module: "env", name: "mv" },
        Grant { module: "env", name: "unbind" },
        Grant { module: "env", name: "bind" },
        Grant { module: "env", name: "rise" },
        Grant { module: "env", name: "other" },
    ];
    audit::audit(bytes, &GRANTS).map_err(|e| JsError::new(&e))
}

/// Recursive begetting (docs §20.1/§21): compile a BEGOTTEN child's soul with
/// only a SUBSET of the entity capabilities — the ones its parent grants it.
/// sin/cos are pure math (always available); get/set/fr/mv/unbind/rise are the
/// grantable, world-touching capabilities. A child soul that calls a capability
/// its parent did not pass down is rejected at codegen — the same fence, one
/// generation deeper. Permissions are monotonically non-increasing by
/// construction: the compiler will not emit an import the grant list omits.
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_entity_wasm_grants(src: &str, grants: Vec<String>) -> Result<Vec<u8>, JsError> {
    use codegen::HostFn;
    const PARAMS: [&str; 3] = ["t", "ex", "ey"];
    let g = |n: &str| grants.iter().any(|x| x == n);
    let mut imports = vec![
        HostFn { name: "sin", n_args: 1, returns: true },
        HostFn { name: "cos", n_args: 1, returns: true },
    ];
    if g("get") { imports.push(HostFn { name: "get", n_args: 1, returns: true }); }
    if g("set") { imports.push(HostFn { name: "set", n_args: 2, returns: false }); }
    if g("fr") { imports.push(HostFn { name: "fr", n_args: 3, returns: true }); }
    if g("mv") { imports.push(HostFn { name: "mv", n_args: 2, returns: false }); }
    if g("unbind") { imports.push(HostFn { name: "unbind", n_args: 0, returns: false }); }
    if g("bind") { imports.push(HostFn { name: "bind", n_args: 1, returns: true }); }
    if g("rise") { imports.push(HostFn { name: "rise", n_args: 1, returns: false }); }
    if g("other") { imports.push(HostFn { name: "other", n_args: 2, returns: true }); }
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile_with_opts(
        &prog,
        &PARAMS,
        &imports,
        codegen::CompileOpts { fuel: Some(200_000), memory_pages: None },
    )
    .map_err(|e| JsError::new(&e))
}

/// The module-level twin of the above for externally-compiled child souls:
/// audit that the begotten soul's imports ⊆ the subset its parent granted.
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn audit_entity_bytes_grants(bytes: &[u8], grants: Vec<String>) -> Result<(), JsError> {
    use audit::Grant;
    let g = |n: &str| grants.iter().any(|x| x == n);
    let mut allow = vec![
        Grant { module: "env", name: "sin" },
        Grant { module: "env", name: "cos" },
    ];
    if g("get") { allow.push(Grant { module: "env", name: "get" }); }
    if g("set") { allow.push(Grant { module: "env", name: "set" }); }
    if g("fr") { allow.push(Grant { module: "env", name: "fr" }); }
    if g("mv") { allow.push(Grant { module: "env", name: "mv" }); }
    if g("unbind") { allow.push(Grant { module: "env", name: "unbind" }); }
    if g("bind") { allow.push(Grant { module: "env", name: "bind" }); }
    if g("rise") { allow.push(Grant { module: "env", name: "rise" }); }
    if g("other") { allow.push(Grant { module: "env", name: "other" }); }
    audit::audit(bytes, &allow).map_err(|e| JsError::new(&e))
}

/// Benchmark lane with fuel metering on: same `run(n)->f64` ABI plus an
/// exported "fuel" gauge. Used to measure the back-edge-counter tax.
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_to_wasm_fueled(src: &str, budget: u32) -> Result<Vec<u8>, JsError> {
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile_with_opts(
        &prog,
        &["n"],
        &[],
        codegen::CompileOpts { fuel: Some(budget), memory_pages: None },
    )
    .map_err(|e| JsError::new(&e))
}

#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn transpile_to_js(src: &str) -> Result<String, JsError> {
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    Ok(parser::to_js(&prog))
}

/// L4 — the seed IS a GPU fragment shader: DSL → GLSL ES 3.00 body. The shader
/// fence is the narrowest of all: pure math + the colour verbs + the pointer
/// uniforms; no get/set (a pixel has no memory), no drawing calls, no reach.
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn transpile_to_glsl(src: &str) -> Result<String, JsError> {
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    parser::to_glsl(&prog).map_err(|e| JsError::new(&e))
}

/// The default benchmark kernel, hand-written in Rust and AOT-compiled into
/// this module — the performance ceiling reference. Only meaningful when the
/// page's script is the unmodified default kernel.
#[cfg_attr(feature = "js-api", wasm_bindgen)]
pub fn native_kernel(n: f64) -> f64 {
    let mut sum = 0.0f64;
    let mut i = 0.0f64;
    while i < n {
        sum = sum + i * i - sum / (i + 1.0);
        i += 1.0;
    }
    sum
}

#[cfg(test)]
mod tests {
    const KERNEL: &str = "let sum = 0.0;\nlet i = 0.0;\nwhile i < n {\n sum = sum + i * i - sum / (i + 1.0);\n i = i + 1.0;\n}\nsum";

    /// JS transpilation of the kernel must be syntactically plausible.
    #[test]
    fn kernel_transpiles() {
        let prog = crate::parser::parse(KERNEL).unwrap();
        let js = crate::parser::to_js(&prog);
        assert!(js.contains("while("));
        assert!(js.contains("return sum;"));
    }

    /// Canvas kernel transpiled to JS keeps the call sites (sin/cos/out become
    /// function parameters on the JS side).
    #[test]
    fn canvas_kernel_transpiles_calls() {
        let src = "let a = t * 2.0;\nout(hx + cos(a), hy + sin(a));\na * 0.5";
        let js = crate::parser::to_js(&crate::parser::parse(src).unwrap());
        assert!(js.contains("out((hx+cos(a)),(hy+sin(a)));"));
        assert!(js.trim_end().ends_with("return (a*0.5);"));
    }
}
