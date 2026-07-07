//! DynamicCell PoC — runtime 動態元件 in a pure-Rust CSR app (Leptos).
//!
//! 證明三件事:
//! 1. 行為動態:每格的腳本可即時編輯 → wasm-jit 編成 WASM 細胞(~µs 級)→
//!    細胞驅動 Leptos signal → DOM 反應式更新。全程零手寫 JS。
//! 2. 結構動態:元件樹 = schema 資料(JSON),Apply 即重組——結構即資料,
//!    由編譯期的靜態 renderer 解譯。
//! 3. 沙箱:細胞的 capability 只有 sin/cos/out,無 DOM 權限;腳本裡寫
//!    fetch() 在 codegen 即被拒(顯示 granted capabilities 清單)。

use js_sys::{Function, Object, Reflect, Uint8Array, WebAssembly};
use leptos::prelude::*;
use serde::Deserialize;
use std::rc::Rc;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_jit::{codegen, parser};

/// UI-cell ABI: run(a, b) -> f64,capabilities = env.{sin, cos, out}。
const UI_PARAMS: [&str; 2] = ["a", "b"];

/// A live generated cell: exported `run` + the import closures that must
/// outlive the instance.
struct CellRt {
    run: Function,
    _sin: Closure<dyn Fn(f64) -> f64>,
    _cos: Closure<dyn Fn(f64) -> f64>,
    _out: Closure<dyn Fn(f64, f64)>,
}

fn compile_cell(src: &str) -> Result<Vec<u8>, String> {
    let prog = parser::parse(src)?;
    codegen::compile_with(&prog, &UI_PARAMS, &codegen::KERNEL_IMPORTS)
}

fn instantiate_cell(bytes: &[u8], out_sig: RwSignal<(f64, f64)>) -> Result<CellRt, String> {
    let jerr = |e: JsValue| format!("{e:?}");
    let module =
        WebAssembly::Module::new(&Uint8Array::from(bytes).into()).map_err(jerr)?;
    let sin: Closure<dyn Fn(f64) -> f64> = Closure::new(|x: f64| x.sin());
    let cos: Closure<dyn Fn(f64) -> f64> = Closure::new(|x: f64| x.cos());
    let out: Closure<dyn Fn(f64, f64)> =
        Closure::new(move |x: f64, y: f64| out_sig.set((x, y)));
    let env = Object::new();
    Reflect::set(&env, &"sin".into(), sin.as_ref()).map_err(jerr)?;
    Reflect::set(&env, &"cos".into(), cos.as_ref()).map_err(jerr)?;
    Reflect::set(&env, &"out".into(), out.as_ref()).map_err(jerr)?;
    let imports = Object::new();
    Reflect::set(&imports, &"env".into(), &env).map_err(jerr)?;
    let instance = WebAssembly::Instance::new(&module, &imports).map_err(jerr)?;
    let run = Reflect::get(&instance.exports(), &"run".into())
        .map_err(jerr)?
        .dyn_into::<Function>()
        .map_err(|_| "export 'run' is not a function".to_string())?;
    Ok(CellRt { run, _sin: sin, _cos: cos, _out: out })
}

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
    let cell: RwSignal<Option<Rc<CellRt>>, LocalStorage> = RwSignal::new_local(None);

    // 種子→細胞:script 變即重編(µs 級,live)
    Effect::new(move |_| {
        let src = script.get();
        match compile_cell(&src).and_then(|bytes| instantiate_cell(&bytes, out)) {
            Ok(rt) => {
                cell.set(Some(Rc::new(rt)));
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
        if let Some(rt) = cell.get() {
            if let Ok(v) = rt.run.call2(&JsValue::NULL, &av.into(), &bv.into()) {
                ret.set(v.as_f64().unwrap_or(f64::NAN));
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

    view! {
        <h1>"DynamicCell — 純 Rust CSR(Leptos)裡的 runtime 動態元件"
            <span class="nav"><a href="../..">"↩ wasm-jit"</a></span></h1>
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
    }
}

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}
