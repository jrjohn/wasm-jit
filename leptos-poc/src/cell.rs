//! cell.rs — the ONLY module that touches js-sys. All boundary ugliness
//! (Reflect, Closure lifetimes, dynamic casts) is contained here; the rest
//! of the app sees a typed, capability-declared API:
//!
//! ```ignore
//! let cell = Cell::builder(&["a", "b"])
//!     .cap1("sin", f64::sin)
//!     .cap2_void("out", move |x, y| sig.set((x, y)))
//!     .compile(src)?;
//! let v = cell.call(&[a, b])?;
//! ```
//!
//! The grant list is the single source of truth: it derives BOTH the codegen
//! import table (what the script may call) and the JS `env` object (what the
//! instance actually gets) — the two can never drift.

use js_sys::{Array, Function, Object, Reflect, Uint8Array, WebAssembly};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_jit::codegen::{self, HostFn};
use wasm_jit::parser;

enum Cap {
    Fn1(Closure<dyn Fn(f64) -> f64>),
    Fn1Void(Closure<dyn Fn(f64)>),
    Fn2Void(Closure<dyn Fn(f64, f64)>),
    Fn3Void(Closure<dyn Fn(f64, f64, f64)>),
    Fn4Void(Closure<dyn Fn(f64, f64, f64, f64)>),
    Fn5Void(Closure<dyn Fn(f64, f64, f64, f64, f64)>),
    Fn6Void(Closure<dyn Fn(f64, f64, f64, f64, f64, f64)>),
}

impl Cap {
    fn js(&self) -> &JsValue {
        match self {
            Cap::Fn1(c) => c.as_ref(),
            Cap::Fn1Void(c) => c.as_ref(),
            Cap::Fn2Void(c) => c.as_ref(),
            Cap::Fn3Void(c) => c.as_ref(),
            Cap::Fn4Void(c) => c.as_ref(),
            Cap::Fn5Void(c) => c.as_ref(),
            Cap::Fn6Void(c) => c.as_ref(),
        }
    }
    fn host_fn(&self, name: &'static str) -> HostFn {
        let (n_args, returns) = match self {
            Cap::Fn1(_) => (1, true),
            Cap::Fn1Void(_) => (1, false),
            Cap::Fn2Void(_) => (2, false),
            Cap::Fn3Void(_) => (3, false),
            Cap::Fn4Void(_) => (4, false),
            Cap::Fn5Void(_) => (5, false),
            Cap::Fn6Void(_) => (6, false),
        };
        HostFn { name, n_args, returns }
    }
}

pub struct CellBuilder {
    params: Vec<String>,
    caps: Vec<(&'static str, Cap)>,
}

/// A live generated cell. Holds the import closures for exactly as long as
/// the instance lives — the lifetime hazard is encoded in the type, not in
/// the caller's discipline.
pub struct Cell {
    run: Function,
    bytes_len: usize,
    _caps: Vec<Cap>,
}

impl Cell {
    pub fn builder(params: &[&str]) -> CellBuilder {
        CellBuilder {
            params: params.iter().map(|p| p.to_string()).collect(),
            caps: Vec::new(),
        }
    }

    /// Generated module size in bytes.
    pub fn size(&self) -> usize {
        self.bytes_len
    }

    /// Call the cell with `args` (arity = the builder's params).
    pub fn call(&self, args: &[f64]) -> Result<f64, String> {
        let arr = Array::new();
        for a in args {
            arr.push(&JsValue::from_f64(*a));
        }
        self.run
            .apply(&JsValue::NULL, &arr)
            .map_err(fmt_js)?
            .as_f64()
            .ok_or_else(|| "cell returned a non-number".into())
    }
}

impl CellBuilder {
    /// Grant a pure `f64 -> f64` capability (e.g. sin, cos).
    pub fn cap1(mut self, name: &'static str, f: impl Fn(f64) -> f64 + 'static) -> Self {
        self.caps.push((name, Cap::Fn1(Closure::new(f))));
        self
    }

    /// Grant a void `(f64, f64)` capability (e.g. out — the write channel).
    pub fn cap2_void(mut self, name: &'static str, f: impl Fn(f64, f64) + 'static) -> Self {
        self.caps.push((name, Cap::Fn2Void(Closure::new(f))));
        self
    }

    pub fn cap1_void(mut self, name: &'static str, f: impl Fn(f64) + 'static) -> Self {
        self.caps.push((name, Cap::Fn1Void(Closure::new(f))));
        self
    }

    pub fn cap3_void(mut self, name: &'static str, f: impl Fn(f64, f64, f64) + 'static) -> Self {
        self.caps.push((name, Cap::Fn3Void(Closure::new(f))));
        self
    }

    pub fn cap4_void(mut self, name: &'static str, f: impl Fn(f64, f64, f64, f64) + 'static) -> Self {
        self.caps.push((name, Cap::Fn4Void(Closure::new(f))));
        self
    }

    pub fn cap5_void(
        mut self,
        name: &'static str,
        f: impl Fn(f64, f64, f64, f64, f64) + 'static,
    ) -> Self {
        self.caps.push((name, Cap::Fn5Void(Closure::new(f))));
        self
    }

    pub fn cap6_void(
        mut self,
        name: &'static str,
        f: impl Fn(f64, f64, f64, f64, f64, f64) + 'static,
    ) -> Self {
        self.caps.push((name, Cap::Fn6Void(Closure::new(f))));
        self
    }

    /// Compile DSL source against exactly the granted capabilities, then
    /// instantiate (sync — generated modules are tiny, far under Chrome's 4KB
    /// main-thread limit).
    pub fn compile(self, src: &str) -> Result<Cell, String> {
        let prog = parser::parse(src)?;
        let params: Vec<&str> = self.params.iter().map(|s| s.as_str()).collect();
        let host: Vec<HostFn> = self.caps.iter().map(|(n, c)| c.host_fn(n)).collect();
        let bytes = codegen::compile_with(&prog, &params, &host)?;
        self.instantiate_bytes(&bytes)
    }

    /// The foundation of the seed-language spectrum: accept WASM bytes from **any
    /// source** (AssemblyScript / Rust→wasm / hand-written WAT), first audit that
    /// the import section ⊆ the granted capabilities, and only instantiate if it
    /// passes. This is the language-agnostic version of compile() — the fence is
    /// in the import table, not the grammar.
    pub fn from_wasm_bytes(self, bytes: &[u8]) -> Result<Cell, String> {
        let grants: Vec<wasm_jit::audit::Grant> = self
            .caps
            .iter()
            .map(|(n, _)| wasm_jit::audit::Grant { module: "env", name: n })
            .collect();
        wasm_jit::audit::audit(bytes, &grants)?; // an over-reaching import → rejected right here
        self.instantiate_bytes(bytes)
    }

    fn instantiate_bytes(self, bytes: &[u8]) -> Result<Cell, String> {
        let module =
            WebAssembly::Module::new(&Uint8Array::from(bytes).into()).map_err(fmt_js)?;
        let env = Object::new();
        for (name, cap) in &self.caps {
            Reflect::set(&env, &(*name).into(), cap.js()).map_err(fmt_js)?;
        }
        let imports = Object::new();
        Reflect::set(&imports, &"env".into(), &env).map_err(fmt_js)?;
        let instance = WebAssembly::Instance::new(&module, &imports).map_err(fmt_js)?;
        let run = Reflect::get(&instance.exports(), &"run".into())
            .map_err(fmt_js)?
            .dyn_into::<Function>()
            .map_err(|_| "export 'run' is not a function".to_string())?;
        Ok(Cell {
            run,
            bytes_len: bytes.len(),
            _caps: self.caps.into_iter().map(|(_, c)| c).collect(),
        })
    }
}

fn fmt_js(e: JsValue) -> String {
    format!("{e:?}")
}
