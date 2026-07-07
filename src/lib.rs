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
