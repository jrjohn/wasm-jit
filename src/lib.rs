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

/// Free-drawing kernel: `run(t, w, h)`, capabilities = 2D drawing primitives.
/// No widgets required — the primitive vocabulary is complete for 2D (SVG's
/// ~10 path commands can express any shape); any shape is just the generated
/// script composing those primitives.
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_draw_wasm(src: &str) -> Result<Vec<u8>, JsError> {
    use codegen::HostFn;
    const PARAMS: [&str; 3] = ["t", "w", "h"];
    const IMPORTS: [HostFn; 9] = [
        HostFn { name: "sin", n_args: 1, returns: true },
        HostFn { name: "cos", n_args: 1, returns: true },
        HostFn { name: "hue", n_args: 1, returns: false },   // set colour by hue (fixed sat/light)
        HostFn { name: "rgb", n_args: 3, returns: false },   // set colour by r,g,b (0..1 each)
        HostFn { name: "hsl", n_args: 3, returns: false },   // set colour by hue,sat,light (0..1) — natural tones, shadows
        HostFn { name: "disc", n_args: 3, returns: false },  // filled circle (x,y,r)
        HostFn { name: "ring", n_args: 3, returns: false },  // outlined circle (x,y,r)
        HostFn { name: "arc", n_args: 5, returns: false },   // arc (x,y,r,a0,a1)
        HostFn { name: "line", n_args: 4, returns: false },  // line (x1,y1,x2,y2)
    ];
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile_with(&prog, &PARAMS, &IMPORTS).map_err(|e| JsError::new(&e))
}

/// UI-logic cell for the live-generation demo: `run(x) -> f64`, capabilities
/// env.{sin, cos, get, set} — get/set is a host-granted 32-slot f64 store so
/// multi-input logic works (input cells persist to slots, computed cells read
/// them). Fuel-metered at 200k. The same contract the gen-server validates
/// against natively — browser and server compile identical modules.
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_ui_cell_wasm(src: &str) -> Result<Vec<u8>, JsError> {
    use codegen::HostFn;
    const PARAMS: [&str; 1] = ["x"];
    const IMPORTS: [HostFn; 4] = [
        HostFn { name: "sin", n_args: 1, returns: true },
        HostFn { name: "cos", n_args: 1, returns: true },
        HostFn { name: "get", n_args: 1, returns: true },
        HostFn { name: "set", n_args: 2, returns: false },
    ];
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile_with_opts(
        &prog,
        &PARAMS,
        &IMPORTS,
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

/// Inhabitant (entity) behavior for the Field: `run(t, ex, ey) -> f64`.
/// Capabilities env.{sin, cos, get, set, fr, mv, unbind} — fr reads the shared
/// field, mv REQUESTS movement (host clamps speed/bounds; position is
/// host-owned), unbind() releases the being from whatever it rides (§19: the
/// freedom to leave a condition, now in the being's own ABI).
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_entity_wasm(src: &str) -> Result<Vec<u8>, JsError> {
    use codegen::HostFn;
    const PARAMS: [&str; 3] = ["t", "ex", "ey"];
    const IMPORTS: [HostFn; 9] = [
        HostFn { name: "sin", n_args: 1, returns: true },
        HostFn { name: "cos", n_args: 1, returns: true },
        HostFn { name: "get", n_args: 1, returns: true },
        HostFn { name: "set", n_args: 2, returns: false },
        HostFn { name: "fr", n_args: 3, returns: true },
        HostFn { name: "mv", n_args: 2, returns: false },
        HostFn { name: "unbind", n_args: 0, returns: false },
        HostFn { name: "rise", n_args: 1, returns: false }, // the vertical faculty: request a change in altitude (host clamps)
        HostFn { name: "other", n_args: 2, returns: true },  // sense the i-th nearest being: other(i,0)=dist, (i,1)=dx, (i,2)=dy
    ];
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile_with_opts(
        &prog,
        &PARAMS,
        &IMPORTS,
        codegen::CompileOpts { fuel: Some(200_000), memory_pages: None },
    )
    .map_err(|e| JsError::new(&e))
}

/// Runtime-generated SKIN: `run(px, py, s, t)`, capabilities = the 2D drawing
/// primitives only (env.{sin, cos, hue, disc, ring, arc, line}). Turns the key
/// of docs §20.1 — a skin no longer needs raw canvas authority, so a novel
/// inhabitant's *look* (a lotus, a deer, a tent) can be generated at runtime
/// and enter through the same drawing-primitive fence as everything else.
/// px,py = the entity's canvas center; s = canvas px per grid unit; t = seconds.
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_skin_wasm(src: &str) -> Result<Vec<u8>, JsError> {
    use codegen::HostFn;
    const PARAMS: [&str; 6] = ["px", "py", "s", "t", "nx", "ny"]; // nx,ny (-1..1) = direction to the nearest other being
    const IMPORTS: [HostFn; 9] = [
        HostFn { name: "sin", n_args: 1, returns: true },
        HostFn { name: "cos", n_args: 1, returns: true },
        HostFn { name: "hue", n_args: 1, returns: false },
        HostFn { name: "rgb", n_args: 3, returns: false }, // colour by r,g,b (0..1)
        HostFn { name: "hsl", n_args: 3, returns: false }, // colour by hue,sat,light (0..1) — skin tones, shading
        HostFn { name: "disc", n_args: 3, returns: false },
        HostFn { name: "ring", n_args: 3, returns: false },
        HostFn { name: "arc", n_args: 5, returns: false },
        HostFn { name: "line", n_args: 4, returns: false },
    ];
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile_with_opts(
        &prog,
        &PARAMS,
        &IMPORTS,
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
    const GRANTS: [Grant; 9] = [
        Grant { module: "env", name: "sin" },
        Grant { module: "env", name: "cos" },
        Grant { module: "env", name: "get" },
        Grant { module: "env", name: "set" },
        Grant { module: "env", name: "fr" },
        Grant { module: "env", name: "mv" },
        Grant { module: "env", name: "unbind" },
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
