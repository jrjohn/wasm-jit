//! DynamicCell PoC — runtime dynamic components in a pure-Rust CSR app (Leptos).
//!
//! Proves three things:
//! 1. Behavior is dynamic: each cell's script can be live-edited → wasm-jit
//!    compiles a WASM cell (~µs) → the cell drives a Leptos signal → reactive
//!    DOM update. Zero hand-written JS throughout.
//! 2. Structure is dynamic: the component tree = schema data (JSON), Apply to
//!    recompose — structure is data, interpreted by a compile-time static renderer.
//! 3. Sandbox: a cell's only capabilities are sin/cos/out, no DOM authority;
//!    writing fetch() in a script is rejected at codegen (shows the granted list).

mod cell;
mod draw_tab;
mod form;
mod layout;
mod spectrum_tab;
mod tokens;
mod tokens_tab;

use cell::Cell;
use draw_tab::DrawPoc;
use form::FormPoc;
use layout::LayoutPoc;
use spectrum_tab::SpectrumPoc;
use tokens_tab::TokensPoc;
use leptos::prelude::*;
use serde::Deserialize;
use std::rc::Rc;

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct CellSpec {
    label: String,
    script: String,
}

#[component]
fn DynamicCell(spec: CellSpec, a: RwSignal<f64>, b: RwSignal<f64>) -> impl IntoView {
    let script = RwSignal::new(spec.script.clone());
    let err = RwSignal::new(String::new());
    let out = RwSignal::new((0.0f64, 0.0f64));
    let ret = RwSignal::new(0.0f64);
    let cell: RwSignal<Option<Rc<Cell>>, LocalStorage> = RwSignal::new_local(None);

    // seed→cell: recompile whenever the script changes (µs, live). The grant
    // list is right here and is also codegen's import table — they can't drift.
    Effect::new(move |_| {
        let src = script.get();
        let built = Cell::builder(&["a", "b"])
            .cap1("sin", f64::sin)
            .cap1("cos", f64::cos)
            .cap2_void("out", move |x, y| out.set((x, y)))
            .compile(&src);
        match built {
            Ok(c) => {
                cell.set(Some(Rc::new(c)));
                err.set(String::new());
            }
            Err(e) => {
                cell.set(None);
                err.set(e);
            }
        }
    });
    // trigger→manifest: input (or cell) changes → run cell → out()/return writes signal → reactive DOM
    Effect::new(move |_| {
        let (av, bv) = (a.get(), b.get());
        if let Some(c) = cell.get() {
            if let Ok(v) = c.call(&[av, bv]) {
                ret.set(v);
            }
        }
    });

    let inject_fetch = move |_| script.set("fetch(a);\n0.0".to_string());

    view! {
        <div class="cell">
            <div class="cell-head">
                <b>{spec.label.clone()}</b>
                <button class="try-fetch" on:click=inject_fetch>"try to escalate: fetch()"</button>
            </div>
            <textarea class="cell-src" rows="4"
                prop:value=move || script.get()
                on:input=move |ev| script.set(event_target_value(&ev))></textarea>
            <Show when=move || !err.get().is_empty()>
                <div class="cell-err">{move || err.get()}</div>
            </Show>
            <div class="cell-vis">
                <span class="cell-ret">{move || format!("{:.3}", ret.get())}</span>
                <div class="bar">
                    <div class="bar-fill"
                        style:width=move || format!("{:.1}%", out.get().0.clamp(0.0, 100.0))></div>
                </div>
                <div class="patch"
                    style:background=move || {
                        format!("hsl({:.0},70%,55%)", (out.get().1 * 360.0).rem_euclid(360.0))
                    }></div>
            </div>
        </div>
    }
}

const DEFAULT_SCHEMA: &str = r#"[
  {"label":"Product + harmonic","script":"out(a * b * 0.01 + sin(a * 0.1) * 20.0, a * 0.01);\na * b"},
  {"label":"Energy","script":"let e = a * a + b * b;\nout(e * 0.005, b * 0.01);\ne"},
  {"label":"Phase","script":"let p = sin(a * 0.05) * cos(b * 0.05);\nout(50.0 + p * 50.0, p * 0.5 + 0.5);\np"}
]"#;

#[component]
fn App() -> impl IntoView {
    let a = RwSignal::new(30.0f64);
    let b = RwSignal::new(60.0f64);
    let schema_text = RwSignal::new(DEFAULT_SCHEMA.trim().to_string());
    let specs =
        RwSignal::new(serde_json::from_str::<Vec<CellSpec>>(DEFAULT_SCHEMA).unwrap());
    let schema_err = RwSignal::new(String::new());
    let apply = move |_| match serde_json::from_str::<Vec<CellSpec>>(&schema_text.get()) {
        Ok(s) => {
            specs.set(s);
            schema_err.set(String::new());
        }
        Err(e) => schema_err.set(e.to_string()),
    };

    let tab = RwSignal::new("cells");

    view! {
        <h1>"DynamicCell — runtime dynamic components in a pure-Rust CSR app (Leptos)"
            <span class="nav"><a href="../..">"↩ wasm-jit"</a></span></h1>
        <div class="tabs">
            <button class="tab-cells" class:on=move || tab.get() == "cells"
                on:click=move |_| tab.set("cells")>"DynamicCell"</button>
            <button class="tab-form" class:on=move || tab.get() == "form"
                on:click=move |_| tab.set("form")>"Form (all widgets × cell rules × Rust API)"</button>
            <button class="tab-tokens" class:on=move || tab.get() == "tokens"
                on:click=move |_| tab.set("tokens")>"Tokens (style as capability)"</button>
            <button class="tab-layout" class:on=move || tab.get() == "layout"
                on:click=move |_| tab.set("layout")>"Layout (layout as schema)"</button>
            <button class="tab-draw" class:on=move || tab.get() == "draw"
                on:click=move |_| tab.set("draw")>"Freeform draw (Buddha)"</button>
            <button class="tab-mc" class:on=move || tab.get() == "mc"
                on:click=move |_| tab.set("mc")>"3D voxel (Minecraft)"</button>
            <button class="tab-spectrum" class:on=move || tab.get() == "spectrum"
                on:click=move |_| tab.set("spectrum")>"Seed-language spectrum"</button>
        </div>
        <Show when=move || tab.get() == "mc">
            <DrawPoc example="mc3p" />
        </Show>
        <Show when=move || tab.get() == "spectrum">
            <SpectrumPoc />
        </Show>
        <Show when=move || tab.get() == "layout">
            <LayoutPoc />
        </Show>
        <Show when=move || tab.get() == "draw">
            <DrawPoc />
        </Show>
        <Show when=move || tab.get() == "form">
            <FormPoc />
        </Show>
        <Show when=move || tab.get() == "tokens">
            <TokensPoc />
        </Show>
        <Show when=move || tab.get() == "cells">
        <div class="cells-tab">
        <p class="sub">
            "Structure = schema data (the JSON below; Apply to recompose the tree); behavior = script seeds (each cell "
            "is live-editable, wasm-jit compiles a WASM cell on the spot); a cell's only capabilities are sin/cos/out, no DOM authority — "
            "it drives a signal, the DOM is handled by compile-time Leptos. Zero hand-written JS throughout."
        </p>
        <div class="inputs">
            "a = "
            <input type="range" min="0" max="100" step="1" class="in-a"
                prop:value=move || a.get().to_string()
                on:input=move |ev| a.set(event_target_value(&ev).parse().unwrap_or(0.0)) />
            <span class="val-a">{move || format!("{:.0}", a.get())}</span>
            "b = "
            <input type="range" min="0" max="100" step="1" class="in-b"
                prop:value=move || b.get().to_string()
                on:input=move |ev| b.set(event_target_value(&ev).parse().unwrap_or(0.0)) />
            <span class="val-b">{move || format!("{:.0}", b.get())}</span>
        </div>
        <div class="cells">
            {move || {
                specs
                    .get()
                    .into_iter()
                    .map(|s| view! { <DynamicCell spec=s a=a b=b /> })
                    .collect_view()
            }}
        </div>
        <h2>"Structure schema (the component tree is data)"</h2>
        <textarea class="schema" rows="8"
            prop:value=move || schema_text.get()
            on:input=move |ev| schema_text.set(event_target_value(&ev))></textarea>
        <div>
            <button class="apply" on:click=apply>"Apply schema"</button>
            <Show when=move || !schema_err.get().is_empty()>
                <span class="cell-err">{move || schema_err.get()}</span>
            </Show>
        </div>
        </div>
        </Show>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}
