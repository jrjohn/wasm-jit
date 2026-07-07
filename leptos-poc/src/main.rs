//! DynamicCell PoC — runtime 動態元件 in a pure-Rust CSR app (Leptos).
//!
//! 證明三件事:
//! 1. 行為動態:每格的腳本可即時編輯 → wasm-jit 編成 WASM 細胞(~µs 級)→
//!    細胞驅動 Leptos signal → DOM 反應式更新。全程零手寫 JS。
//! 2. 結構動態:元件樹 = schema 資料(JSON),Apply 即重組——結構即資料,
//!    由編譯期的靜態 renderer 解譯。
//! 3. 沙箱:細胞的 capability 只有 sin/cos/out,無 DOM 權限;腳本裡寫
//!    fetch() 在 codegen 即被拒(顯示 granted capabilities 清單)。

mod cell;
mod draw_tab;
mod form;
mod layout;
mod tokens;
mod tokens_tab;

use cell::Cell;
use draw_tab::DrawPoc;
use form::FormPoc;
use layout::LayoutPoc;
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

    // 種子→細胞:script 變即重編(µs 級,live)。grant 清單在此一目瞭然,
    // 同時就是 codegen 的 import 表——兩者不可能漂移。
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
    // 緣→現行:輸入(或細胞)變 → 跑細胞 → out()/回傳寫 signal → DOM 反應式更新
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
                <button class="try-fetch" on:click=inject_fetch>"試圖越權 fetch()"</button>
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
  {"label":"乘積 + 諧波","script":"out(a * b * 0.01 + sin(a * 0.1) * 20.0, a * 0.01);\na * b"},
  {"label":"能量","script":"let e = a * a + b * b;\nout(e * 0.005, b * 0.01);\ne"},
  {"label":"相位","script":"let p = sin(a * 0.05) * cos(b * 0.05);\nout(50.0 + p * 50.0, p * 0.5 + 0.5);\np"}
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
        <h1>"DynamicCell — 純 Rust CSR(Leptos)裡的 runtime 動態元件"
            <span class="nav"><a href="../..">"↩ wasm-jit"</a></span></h1>
        <div class="tabs">
            <button class="tab-cells" class:on=move || tab.get() == "cells"
                on:click=move |_| tab.set("cells")>"DynamicCell"</button>
            <button class="tab-form" class:on=move || tab.get() == "form"
                on:click=move |_| tab.set("form")>"表單(全元件 × 細胞規則 × Rust API)"</button>
            <button class="tab-tokens" class:on=move || tab.get() == "tokens"
                on:click=move |_| tab.set("tokens")>"Tokens(樣式即 capability)"</button>
            <button class="tab-layout" class:on=move || tab.get() == "layout"
                on:click=move |_| tab.set("layout")>"Layout(版面即 schema)"</button>
            <button class="tab-draw" class:on=move || tab.get() == "draw"
                on:click=move |_| tab.set("draw")>"自由繪(佛陀)"</button>
        </div>
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
            "結構 = schema 資料(下方 JSON,Apply 即重組元件樹);行為 = 腳本種子(每格可即時編輯,"
            "wasm-jit 當場編成 WASM 細胞);細胞的 capability 只有 sin/cos/out,無 DOM 權限——"
            "它驅動 signal,DOM 由編譯期的 Leptos 管。全程零手寫 JS。"
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
        <h2>"結構 schema(元件樹即資料)"</h2>
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
