//! spectrum_tab.rs — the seed-language spectrum (Tier 1 home DSL / Tier 2 externally compiled WASM).
//!
//! The core proof: the host's Cell doesn't care who compiled the bytes, only that
//! the import section ⊆ the grant list.
//! - Tier 1: DSL source → home codegen → Cell::compile
//! - Tier 2: an "external toolchain" (here wasm-encoder assembles a module on the fly
//!   inside the browser to stand in for an AssemblyScript/Rust→wasm artifact) → a blob
//!   of .wasm bytes → Cell::from_wasm_bytes.
//!   Both paths use the same `run(a,b)->f64` ABI and the same granted capabilities (env.sin/cos);
//!   if a Tier 2 seed imports an unauthorized env.fetch, the import audit rejects it before instantiation.

use crate::cell::Cell;
use leptos::prelude::*;
use wasm_encoder::{
    CodeSection, EntityType, ExportKind, ExportSection, Function, FunctionSection, ImportSection,
    Instruction, Module, TypeSection, ValType,
};

/// Simulate "an external toolchain's codegen": emit a run(a,b)->f64 module.
/// Imports env.sin(f64)->f64 and env.cos(f64)->f64; body = sin(a)*b + cos(a).
/// When naughty=true it additionally imports env.fetch — simulating an over-reaching external seed.
fn external_toolchain_emit(naughty: bool) -> Vec<u8> {
    let mut m = Module::new();

    let mut types = TypeSection::new();
    types.ty().function([ValType::F64], [ValType::F64]); // ty0: (f64)->f64  (sin/cos/fetch)
    types.ty().function([ValType::F64, ValType::F64], [ValType::F64]); // ty1: run
    m.section(&types);

    let mut imports = ImportSection::new();
    imports.import("env", "sin", EntityType::Function(0)); // func 0
    imports.import("env", "cos", EntityType::Function(0)); // func 1
    if naughty {
        imports.import("env", "fetch", EntityType::Function(0)); // func 2 — unauthorized!
    }
    m.section(&imports);

    let run_idx = if naughty { 3 } else { 2 };
    let mut funcs = FunctionSection::new();
    funcs.function(1); // run : ty1
    m.section(&funcs);

    let mut exports = ExportSection::new();
    exports.export("run", ExportKind::Func, run_idx);
    m.section(&exports);

    // run(a,b) = sin(a)*b + cos(a)   (params: local 0=a, 1=b)
    let mut f = Function::new([]);
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::Call(0)); // sin(a)
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::F64Mul); // sin(a)*b
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::Call(1)); // cos(a)
    f.instruction(&Instruction::F64Add);
    if naughty {
        // the naughty seed also tries to call fetch(a) and discard the result (showing it "attempts" to use the over-reaching capability)
        f.instruction(&Instruction::LocalGet(0));
        f.instruction(&Instruction::Call(2));
        f.instruction(&Instruction::Drop);
    }
    f.instruction(&Instruction::End);
    let mut code = CodeSection::new();
    code.function(&f);
    m.section(&code);

    m.finish()
}

/// Both tiers build the Cell from this same capability set (language-agnostic; env.sin/cos, no fetch).
fn tier_cell_from_dsl(src: &str) -> Result<Cell, String> {
    Cell::builder(&["a", "b"])
        .cap1("sin", f64::sin)
        .cap1("cos", f64::cos)
        .compile(src)
}
fn tier_cell_from_bytes(bytes: &[u8]) -> Result<Cell, String> {
    Cell::builder(&["a", "b"])
        .cap1("sin", f64::sin)
        .cap1("cos", f64::cos)
        .from_wasm_bytes(bytes)
}

#[component]
pub fn SpectrumPoc() -> impl IntoView {
    let a = RwSignal::new(1.2f64);
    let b = RwSignal::new(2.0f64);

    // Tier 1: home DSL
    let dsl_src = RwSignal::new("sin(a) * b + cos(a)".to_string());
    let t1 = RwSignal::new(String::new());
    let t1cell: RwSignal<Option<std::rc::Rc<Cell>>, LocalStorage> = RwSignal::new_local(None);
    Effect::new(move |_| match tier_cell_from_dsl(&dsl_src.get()) {
        Ok(c) => { t1.set(format!("DSL → home codegen → {} bytes", c.size())); t1cell.set(Some(std::rc::Rc::new(c))); }
        Err(e) => { t1.set(format!("compile error: {e}")); t1cell.set(None); }
    });

    // Tier 2: external toolchain output (good / naughty)
    let naughty = RwSignal::new(false);
    let t2 = RwSignal::new(String::new());
    let t2ok = RwSignal::new(true);
    let t2cell: RwSignal<Option<std::rc::Rc<Cell>>, LocalStorage> = RwSignal::new_local(None);
    Effect::new(move |_| {
        let bytes = external_toolchain_emit(naughty.get());
        match tier_cell_from_bytes(&bytes) {
            Ok(c) => { t2.set(format!("external {} bytes → import audit passed → instantiate", bytes.len())); t2ok.set(true); t2cell.set(Some(std::rc::Rc::new(c))); }
            Err(e) => { t2.set(format!("import audit rejected: {e}")); t2ok.set(false); t2cell.set(None); }
        }
    });

    let run = move |cell: RwSignal<Option<std::rc::Rc<Cell>>, LocalStorage>| {
        cell.get().and_then(|c| c.call(&[a.get(), b.get()]).ok())
    };

    view! {
        <p class="sub">
            "Seed-language spectrum: both tiers use the same run(a,b)→f64 ABI and the same granted capabilities (env.sin/cos). "
            "Tier 1 = home DSL (codegen); Tier 2 = a .wasm produced by an external toolchain (AssemblyScript / Rust→wasm). "
            "The host's Cell doesn't care who compiled the bytes — only that the import section ⊆ the grant list. The fence is in the import table, not the grammar."
        </p>
        <div class="inputs">
            "a=" <input type="range" min="0" max="6.28" step="0.01" class="sp-a"
                prop:value=move || a.get().to_string()
                on:input=move |ev| a.set(event_target_value(&ev).parse().unwrap_or(0.0)) />
            <span>{move || format!("{:.2}", a.get())}</span>
            "b=" <input type="range" min="0" max="4" step="0.01" class="sp-b"
                prop:value=move || b.get().to_string()
                on:input=move |ev| b.set(event_target_value(&ev).parse().unwrap_or(0.0)) />
            <span>{move || format!("{:.2}", b.get())}</span>
        </div>

        <div class="ly-card">
            <h3>"Tier 1 — home DSL (source fits in a prompt, µs compile)"</h3>
            <textarea class="sp-dsl" rows="2"
                prop:value=move || dsl_src.get()
                on:input=move |ev| dsl_src.set(event_target_value(&ev))></textarea>
            <div class="sp-line">
                <span class="draw-status ok">{move || t1.get()}</span>
                " → run(a,b) = "
                <b class="sp-t1">{move || run(t1cell).map(|v| format!("{v:.5}")).unwrap_or_else(|| "—".into())}</b>
            </div>
        </div>

        <div class="ly-card">
            <h3>"Tier 2 — external toolchain output (rich language, too big for a prompt; here wasm-encoder stands in for asc output, generated on the fly)"</h3>
            <label class="sp-naughty">
                <input type="checkbox" prop:checked=move || naughty.get()
                    on:change=move |_| naughty.update(|v| *v = !*v) />
                " make the external seed over-reach (extra import env.fetch)"
            </label>
            <div class="sp-line">
                <span class="draw-status" class:ok=move || t2ok.get() class:bad=move || !t2ok.get()>
                    {move || t2.get()}
                </span>
                " → run(a,b) = "
                <b class="sp-t2">{move || run(t2cell).map(|v| format!("{v:.5}")).unwrap_or_else(|| "rejected".into())}</b>
            </div>
        </div>

        <p class="sub">
            "Check the box: the external .wasm's import section gains env::fetch, not in the grant list → "
            <b>"the import audit rejects it before instantiate (the module-level version of the DSL's fetch() codegen rejection)"</b>
            ". The two tiers agree in value (sin·b+cos), proving the shared ABI is interchangeable; the security is identical, proving the fence is language-agnostic."
        </p>
    }
}
