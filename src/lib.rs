//! wasm-jit — runtime script→WASM codegen (the browser engine JITs it) vs
//! Rhai tree-walk interpretation.
//!
//! Exposed to JS (benchmark page, index.html):
//! - `compile_to_wasm(src)`   : DSL source → .wasm module bytes, `run(n)->f64`, no imports
//! - `transpile_to_js(src)`   : same AST → JS function body (V8 JS-JIT reference lane)
//! - `RhaiProgram`            : precompiled Rhai AST, `run(n)` (interpretation lane)
//! - `native_kernel(n)`       : default kernel hand-written in Rust (AOT ceiling lane)
//!
//! Exposed to JS (canvas page, canvas.html):
//! - `compile_kernel_wasm(src)`: DSL → .wasm cell, `run(t,i,hx,hy)->hue`,
//!    capabilities (imports): env.sin, env.cos, env.out(x,y)
//! - `RhaiKernel` + `kernel_out_x/y()`: same kernel interpreted by Rhai

pub mod audit;
pub mod codegen;
pub mod parser;

#[cfg(feature = "rhai-bench")]
use std::cell::Cell;
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

/// 自由繪 kernel:`run(t, w, h)`,capabilities = 2D 繪圖 primitives。
/// 不需要任何 widget——primitive 詞彙對 2D 完備(SVG ~10 個 path 指令可表達任何圖形),
/// 任意圖形 = 生成腳本對 primitives 的組合。
#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn compile_draw_wasm(src: &str) -> Result<Vec<u8>, JsError> {
    use codegen::HostFn;
    const PARAMS: [&str; 3] = ["t", "w", "h"];
    const IMPORTS: [HostFn; 7] = [
        HostFn { name: "sin", n_args: 1, returns: true },
        HostFn { name: "cos", n_args: 1, returns: true },
        HostFn { name: "hue", n_args: 1, returns: false },   // 設定色相
        HostFn { name: "disc", n_args: 3, returns: false },  // 實心圓 (x,y,r)
        HostFn { name: "ring", n_args: 3, returns: false },  // 空心圓 (x,y,r)
        HostFn { name: "arc", n_args: 5, returns: false },   // 弧 (x,y,r,a0,a1)
        HostFn { name: "line", n_args: 4, returns: false },  // 線 (x1,y1,x2,y2)
    ];
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile_with(&prog, &PARAMS, &IMPORTS).map_err(|e| JsError::new(&e))
}

#[cfg(feature = "js-api")]
#[wasm_bindgen]
pub fn transpile_to_js(src: &str) -> Result<String, JsError> {
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    Ok(parser::to_js(&prog))
}

#[cfg(feature = "rhai-bench")]
#[wasm_bindgen]
pub struct RhaiProgram {
    engine: rhai::Engine,
    ast: rhai::AST,
}

#[cfg(feature = "rhai-bench")]
#[wasm_bindgen]
impl RhaiProgram {
    /// Compile the script once (default Rhai engine, no metering — its fastest config).
    #[wasm_bindgen(constructor)]
    pub fn new(src: &str) -> Result<RhaiProgram, JsError> {
        let engine = rhai::Engine::new();
        let ast = engine
            .compile(src)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(RhaiProgram { engine, ast })
    }

    /// Interpret the precompiled AST with `n` in scope.
    pub fn run(&self, n: f64) -> Result<f64, JsError> {
        let mut scope = rhai::Scope::new();
        scope.push("n", n);
        self.engine
            .eval_ast_with_scope::<f64>(&mut scope, &self.ast)
            .map_err(|e| JsError::new(&e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Canvas kernel — Rhai lane. One shared engine (registering the stdlib per
// element would be unfair overhead); out(x, y) writes a thread-local slot the
// host reads back after each run (WASM is single-threaded).
// ---------------------------------------------------------------------------

#[cfg(feature = "rhai-bench")]
thread_local! {
    static OUT: Cell<(f64, f64)> = const { Cell::new((0.0, 0.0)) };
    static KERNEL_ENGINE: rhai::Engine = {
        let mut e = rhai::Engine::new();
        e.register_fn("out", |x: f64, y: f64| OUT.with(|o| o.set((x, y))));
        // Function-form trig, mirroring the WASM cells' env.sin/env.cos grants.
        e.register_fn("sin", |x: f64| x.sin());
        e.register_fn("cos", |x: f64| x.cos());
        e
    };
}

#[cfg(feature = "rhai-bench")]
#[wasm_bindgen]
pub struct RhaiKernel {
    ast: rhai::AST,
}

#[cfg(feature = "rhai-bench")]
#[wasm_bindgen]
impl RhaiKernel {
    #[wasm_bindgen(constructor)]
    pub fn new(src: &str) -> Result<RhaiKernel, JsError> {
        KERNEL_ENGINE
            .with(|e| e.compile(src))
            .map(|ast| RhaiKernel { ast })
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Interpret one frame step; returns hue. Read the position the script
    /// wrote via `kernel_out_x()` / `kernel_out_y()`.
    pub fn run(&self, t: f64, i: f64, hx: f64, hy: f64) -> Result<f64, JsError> {
        let mut scope = rhai::Scope::new();
        scope.push("t", t);
        scope.push("i", i);
        scope.push("hx", hx);
        scope.push("hy", hy);
        KERNEL_ENGINE
            .with(|e| e.eval_ast_with_scope::<f64>(&mut scope, &self.ast))
            .map_err(|e| JsError::new(&e.to_string()))
    }
}

#[cfg(feature = "rhai-bench")]
#[wasm_bindgen]
pub fn kernel_out_x() -> f64 {
    OUT.with(|o| o.get().0)
}

#[cfg(feature = "rhai-bench")]
#[wasm_bindgen]
pub fn kernel_out_y() -> f64 {
    OUT.with(|o| o.get().1)
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

    /// The same source must produce the same value on Rhai as the native kernel
    /// (identical f64 operation order ⇒ bit-identical result).
    #[cfg(feature = "rhai-bench")]
    #[test]
    fn rhai_matches_native() {
        let engine = rhai::Engine::new();
        let ast = engine.compile(KERNEL).unwrap();
        for n in [0.0, 1.0, 10.0, 1000.0] {
            let mut scope = rhai::Scope::new();
            scope.push("n", n);
            let rhai_val = engine
                .eval_ast_with_scope::<f64>(&mut scope, &ast)
                .unwrap();
            assert_eq!(rhai_val, super::native_kernel(n), "n={n}");
        }
    }

    /// JS transpilation of the kernel must be syntactically plausible.
    #[test]
    fn kernel_transpiles() {
        let prog = crate::parser::parse(KERNEL).unwrap();
        let js = crate::parser::to_js(&prog);
        assert!(js.contains("while("));
        assert!(js.contains("return sum;"));
    }

    /// Canvas-kernel semantics on a raw Rhai engine: out(x, y) writes the slot,
    /// the final expression is the hue.
    #[cfg(feature = "rhai-bench")]
    #[test]
    fn rhai_canvas_kernel_out_and_hue() {
        use std::cell::Cell;
        use std::rc::Rc;

        let slot = Rc::new(Cell::new((0.0f64, 0.0f64)));
        let s2 = slot.clone();
        let mut engine = rhai::Engine::new();
        engine.register_fn("out", move |x: f64, y: f64| s2.set((x, y)));
        engine.register_fn("sin", |x: f64| x.sin());
        engine.register_fn("cos", |x: f64| x.cos());

        let src = "let a = t * 2.0;\nout(hx + cos(a), hy + sin(a));\na * 0.5";
        let ast = engine.compile(src).unwrap();
        let mut scope = rhai::Scope::new();
        scope.push("t", 1.0f64);
        scope.push("i", 0.0f64);
        scope.push("hx", 10.0f64);
        scope.push("hy", 20.0f64);
        let hue = engine
            .eval_ast_with_scope::<f64>(&mut scope, &ast)
            .unwrap();
        assert_eq!(hue, 1.0);
        let (x, y) = slot.get();
        assert_eq!(x, 10.0 + 2.0f64.cos());
        assert_eq!(y, 20.0 + 2.0f64.sin());
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
