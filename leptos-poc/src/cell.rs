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

use js_sys::{Array, Float64Array, Function, Object, Reflect, Uint8Array, WebAssembly};
use std::cell::RefCell;
use std::collections::HashMap;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_jit::codegen::{self, CompileOpts, HostFn};
use wasm_jit::parser;

// ---------------------------------------------------------------------------
// Module cache: content-hash → compiled WebAssembly.Module. Re-manifesting an
// identical seed skips parse+codegen+Module::new entirely (instantiation still
// happens per cell — imports differ). Crude eviction: clear at capacity.
// ---------------------------------------------------------------------------

thread_local! {
    static MODULE_CACHE: RefCell<HashMap<u64, WebAssembly::Module>> =
        RefCell::new(HashMap::new());
    static CACHE_STATS: std::cell::Cell<(u32, u32)> = const { std::cell::Cell::new((0, 0)) }; // (hits, misses)
}

const CACHE_CAP: usize = 256;

fn fnv1a(bytes: &[u8], seed: u64) -> u64 {
    let mut h = seed ^ 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

/// (hits, misses) since page load — surfaced in the LiveUI status panel.
pub fn cache_stats() -> (u32, u32) {
    CACHE_STATS.with(|s| s.get())
}

fn cached_module(
    key: u64,
    make_bytes: impl FnOnce() -> Result<Vec<u8>, String>,
) -> Result<(WebAssembly::Module, usize), String> {
    if let Some(m) = MODULE_CACHE.with(|c| c.borrow().get(&key).cloned()) {
        CACHE_STATS.with(|s| {
            let (h, mi) = s.get();
            s.set((h + 1, mi));
        });
        return Ok((m, 0)); // cached: no fresh bytes were produced
    }
    CACHE_STATS.with(|s| {
        let (h, mi) = s.get();
        s.set((h, mi + 1));
    });
    let bytes = make_bytes()?;
    let module =
        WebAssembly::Module::new(&Uint8Array::from(bytes.as_slice()).into()).map_err(fmt_js)?;
    MODULE_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= CACHE_CAP {
            c.clear();
        }
        c.insert(key, module.clone());
    });
    Ok((module, bytes.len()))
}

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
    fuel: Option<u32>,
    memory_pages: Option<u32>,
}

/// A live generated cell. Holds the import closures for exactly as long as
/// the instance lives — the lifetime hazard is encoded in the type, not in
/// the caller's discipline.
pub struct Cell {
    run: Function,
    bytes_len: usize,
    memory: Option<WebAssembly::Memory>,
    fuel_gauge: Option<WebAssembly::Global>,
    fuel_budget: Option<u32>,
    _caps: Vec<Cap>,
}

impl Cell {
    pub fn builder(params: &[&str]) -> CellBuilder {
        CellBuilder {
            params: params.iter().map(|p| p.to_string()).collect(),
            caps: Vec::new(),
            fuel: None,
            memory_pages: None,
        }
    }

    /// Generated module size in bytes (0 when served from the module cache).
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

    /// Fuel consumed by the most recent call (budget − remaining), if metered.
    pub fn fuel_used(&self) -> Option<f64> {
        let budget = self.fuel_budget? as f64;
        let remaining = self.fuel_gauge.as_ref()?.value().as_f64()?;
        Some(budget - remaining)
    }

    /// Write f64 values into the cell's own exported memory at `slot`.
    pub fn write_mem(&self, slot: u32, data: &[f64]) -> Result<(), String> {
        let mem = self.memory.as_ref().ok_or("cell has no memory capability")?;
        let view = Float64Array::new(&mem.buffer());
        if (slot as usize + data.len()) as u32 > view.length() {
            return Err(format!(
                "write_mem out of bounds: slot {slot} + len {} > {} slots",
                data.len(),
                view.length()
            ));
        }
        for (k, v) in data.iter().enumerate() {
            view.set_index(slot + k as u32, *v);
        }
        Ok(())
    }

    /// Read `len` f64 values from the cell's exported memory at `slot`.
    pub fn read_mem(&self, slot: u32, len: u32) -> Result<Vec<f64>, String> {
        let mem = self.memory.as_ref().ok_or("cell has no memory capability")?;
        let view = Float64Array::new(&mem.buffer());
        if slot + len > view.length() {
            return Err(format!(
                "read_mem out of bounds: slot {slot} + len {len} > {} slots",
                view.length()
            ));
        }
        Ok((0..len).map(|k| view.get_index(slot + k)).collect())
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

    /// Fuel-meter every loop: `budget` units per call, trap at zero. The one
    /// switch that makes running seeds you didn't write yourself survivable.
    pub fn fuel(mut self, budget: u32) -> Self {
        self.fuel = Some(budget);
        self
    }

    /// Grant the cell its own `pages`×64KiB linear memory (fixed size),
    /// enabling the DSL's load/store builtins and host read/write_mem.
    pub fn memory(mut self, pages: u32) -> Self {
        self.memory_pages = Some(pages);
        self
    }

    /// Compile DSL source against exactly the granted capabilities, then
    /// instantiate (sync — generated modules are tiny, far under Chrome's 4KB
    /// main-thread limit). Identical (source, params, grants, opts) hits the
    /// module cache and skips parse+codegen+Module compilation.
    pub fn compile(self, src: &str) -> Result<Cell, String> {
        let mut key = fnv1a(src.as_bytes(), 0);
        for p in &self.params {
            key = fnv1a(p.as_bytes(), key);
        }
        for (n, _) in &self.caps {
            key = fnv1a(n.as_bytes(), key);
        }
        key = fnv1a(&self.fuel.unwrap_or(0).to_le_bytes(), key);
        key = fnv1a(&self.memory_pages.unwrap_or(0).to_le_bytes(), key);

        let params: Vec<String> = self.params.clone();
        let host: Vec<HostFn> = self.caps.iter().map(|(n, c)| c.host_fn(n)).collect();
        let opts = CompileOpts { fuel: self.fuel, memory_pages: self.memory_pages };
        let (module, bytes_len) = cached_module(key, || {
            let prog = parser::parse(src)?;
            let p: Vec<&str> = params.iter().map(|s| s.as_str()).collect();
            codegen::compile_with_opts(&prog, &p, &host, opts)
        })?;
        self.instantiate_module(&module, bytes_len)
    }

    /// The foundation of the seed-language spectrum: accept WASM bytes from **any
    /// source** (AssemblyScript / Rust→wasm / hand-written WAT), first audit that
    /// the import section ⊆ the granted capabilities, and only instantiate if it
    /// passes. This is the language-agnostic version of compile() — the fence is
    /// in the import table, not the grammar. (The audit always runs, even on a
    /// module-cache hit — it is a cheap parse, and correctness must not depend
    /// on cache-key discipline.)
    pub fn from_wasm_bytes(self, bytes: &[u8]) -> Result<Cell, String> {
        let grants: Vec<wasm_jit::audit::Grant> = self
            .caps
            .iter()
            .map(|(n, _)| wasm_jit::audit::Grant { module: "env", name: n })
            .collect();
        wasm_jit::audit::audit(bytes, &grants)?; // an over-reaching import → rejected right here
        let key = fnv1a(bytes, 1); // distinct seed from the DSL path
        let len = bytes.len();
        let owned = bytes.to_vec();
        let (module, _) = cached_module(key, move || Ok(owned))?;
        self.instantiate_module(&module, len)
    }

    fn instantiate_module(self, module: &WebAssembly::Module, bytes_len: usize) -> Result<Cell, String> {
        let env = Object::new();
        for (name, cap) in &self.caps {
            Reflect::set(&env, &(*name).into(), cap.js()).map_err(fmt_js)?;
        }
        let imports = Object::new();
        Reflect::set(&imports, &"env".into(), &env).map_err(fmt_js)?;
        let instance = WebAssembly::Instance::new(module, &imports).map_err(fmt_js)?;
        let exports = instance.exports();
        let run = Reflect::get(&exports, &"run".into())
            .map_err(fmt_js)?
            .dyn_into::<Function>()
            .map_err(|_| "export 'run' is not a function".to_string())?;
        let memory = Reflect::get(&exports, &"mem".into())
            .ok()
            .and_then(|v| v.dyn_into::<WebAssembly::Memory>().ok());
        let fuel_gauge = Reflect::get(&exports, &"fuel".into())
            .ok()
            .and_then(|v| v.dyn_into::<WebAssembly::Global>().ok());
        Ok(Cell {
            run,
            bytes_len,
            memory,
            fuel_gauge,
            fuel_budget: self.fuel,
            _caps: self.caps.into_iter().map(|(_, c)| c).collect(),
        })
    }
}

fn fmt_js(e: JsValue) -> String {
    format!("{e:?}")
}
