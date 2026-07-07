//! wasm-jit — runtime script→WASM codegen (V8 JITs it) vs Rhai tree-walk interpretation.
//!
//! Exposed to JS:
//! - `compile_to_wasm(src)`  : DSL source → complete .wasm module bytes (export `run(f64)->f64`)
//! - `transpile_to_js(src)`  : same AST → JS function body (V8 JS-JIT reference lane)
//! - `RhaiProgram`           : precompiled Rhai AST, `run(n)` (tree-walk interpretation lane)
//! - `native_kernel(n)`      : the default kernel hand-written in Rust (AOT ceiling lane)

mod codegen;
mod parser;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn compile_to_wasm(src: &str) -> Result<Vec<u8>, JsError> {
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    codegen::compile(&prog).map_err(|e| JsError::new(&e))
}

#[wasm_bindgen]
pub fn transpile_to_js(src: &str) -> Result<String, JsError> {
    let prog = parser::parse(src).map_err(|e| JsError::new(&e))?;
    Ok(parser::to_js(&prog))
}

#[wasm_bindgen]
pub struct RhaiProgram {
    engine: rhai::Engine,
    ast: rhai::AST,
}

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

/// The default benchmark kernel, hand-written in Rust and AOT-compiled into
/// this module — the performance ceiling reference. Only meaningful when the
/// page's script is the unmodified default kernel.
#[wasm_bindgen]
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
}
