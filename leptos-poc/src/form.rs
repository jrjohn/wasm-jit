//! form.rs — 把 DynamicCell 模式套滿一整張表單(每種元件各一次)。
//!
//! - 結構 = FORM_SCHEMA(JSON)→ 靜態 renderer 解譯(結構即資料)
//! - 行為 = 數值欄的驗證規則 / 計算欄都是 DSL 種子 → wasm-jit 細胞
//!   (字串驗證留在 host——§16 的邊界紀律:字串/物件不下沉)
//! - 部門下拉 = 掛載時呼叫 Rust API(Axum)/api/departments;
//!   選定部門 → /api/members/{id} 載入人員列表

use crate::cell::Cell;
use crate::tokens::style_of;
use gloo_net::http::Request;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;
use std::rc::Rc;

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct FieldSpec {
    name: String,
    label: String,
    widget: String,
    #[serde(default)]
    options: Vec<String>,
    #[serde(default)]
    rule: Option<String>,
    #[serde(default)]
    err: Option<String>,
    #[serde(default)]
    script: Option<String>,
    #[serde(default)]
    params: Vec<String>,
    /// 樣式:只准引用 design token(tokens.rs 驗證);raw CSS 會被拒。
    #[serde(default)]
    style: Option<serde_json::Map<String, serde_json::Value>>,
}

/// 每欄一組泛用 signal(text/num/flag 依 widget 取用)+ 驗證狀態。
#[derive(Clone)]
struct FieldRt {
    spec: FieldSpec,
    text: RwSignal<String>,
    num: RwSignal<f64>,
    flag: RwSignal<bool>,
    valid: RwSignal<bool>,
}

impl FieldRt {
    fn new(spec: FieldSpec) -> Self {
        let init = match spec.name.as_str() {
            "age" => 30.0,
            "salary" => 50000.0,
            "ratio" => 10.0,
            _ => 0.0,
        };
        FieldRt {
            spec,
            text: RwSignal::new(String::new()),
            num: RwSignal::new(init),
            flag: RwSignal::new(true),
            valid: RwSignal::new(true),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Dept {
    id: u32,
    name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Member {
    name: String,
    title: String,
}

/// 規則細胞:run(v) -> 1.0/0.0;計算細胞:run(<params>) -> 值。
/// grant 一律只有 sin/cos/out——表單邏輯也拿不到 DOM/網路。
fn build_cell(params: &[&str], src: &str) -> Result<Cell, String> {
    Cell::builder(params)
        .cap1("sin", f64::sin)
        .cap1("cos", f64::cos)
        .cap2_void("out", |_, _| {})
        .compile(src)
}

fn wire_rules(fields: &Rc<Vec<FieldRt>>) {
    for f in fields.iter() {
        // 驗證規則:數值欄 → 細胞判 1.0/0.0
        if let Some(rule) = &f.spec.rule {
            match build_cell(&["v"], rule) {
                Ok(cellv) => {
                    let cellv = Rc::new(cellv);
                    let (num, valid) = (f.num, f.valid);
                    Effect::new(move |_| {
                        let v = num.get();
                        valid.set(matches!(cellv.call(&[v]), Ok(x) if x == 1.0));
                    });
                }
                Err(_) => f.valid.set(false), // schema 規則本身壞了:恆標 invalid,浮上檯面
            }
        }
        // 計算欄:params 欄位變 → 細胞重算 → 寫回本欄 num
        if let (Some(script), false) = (&f.spec.script, f.spec.params.is_empty()) {
            let names: Vec<&str> = f.spec.params.iter().map(|s| s.as_str()).collect();
            if let Ok(cellc) = build_cell(&names, script) {
                let cellc = Rc::new(cellc);
                let srcs: Vec<RwSignal<f64>> = f
                    .spec
                    .params
                    .iter()
                    .filter_map(|p| fields.iter().find(|x| &x.spec.name == p).map(|x| x.num))
                    .collect();
                let target = f.num;
                Effect::new(move |_| {
                    let args: Vec<f64> = srcs.iter().map(|s| s.get()).collect();
                    if let Ok(v) = cellc.call(&args) {
                        target.set(v);
                    }
                });
            }
        }
    }
}

/// 表單 schema 完全不在 Rust source 裡:runtime 由 GET /api/form-schema 載入
/// (server 每次請求現讀 api-server/form-schema.json)。改檔 → 重載 → 表單即變,零重編。
#[component]
pub fn FormPoc() -> impl IntoView {
    let specs: RwSignal<Option<Vec<FieldSpec>>> = RwSignal::new(None);
    let schema_err = RwSignal::new(String::new());
    let load = move || {
        spawn_local(async move {
            match Request::get("/api/form-schema").send().await {
                Ok(r) => match r.json::<Vec<FieldSpec>>().await {
                    Ok(s) => {
                        specs.set(Some(s));
                        schema_err.set(String::new());
                    }
                    Err(e) => schema_err.set(format!("schema 解析失敗:{e}")),
                },
                Err(e) => schema_err.set(format!("schema API error: {e}")),
            }
        })
    };
    load();

    view! {
        <p class="sub">
            "此表單的 schema 不在 Rust 原始碼裡 —— runtime 由 GET /api/form-schema 載入,"
            "server 每次請求現讀 api-server/form-schema.json。改那個檔案、按重載,表單即變,零重編。"
        </p>
        <button class="apply reload-schema" on:click=move |_| load()>"重新載入 schema"</button>
        <Show when=move || !schema_err.get().is_empty()>
            <div class="cell-err">{move || schema_err.get()}</div>
        </Show>
        {move || specs.get().map(|s| view! { <FormBody specs=s /> })}
    }
}

#[component]
fn FormBody(specs: Vec<FieldSpec>) -> impl IntoView {
    let fields: Rc<Vec<FieldRt>> =
        Rc::new(specs.into_iter().map(FieldRt::new).collect());
    wire_rules(&fields);

    // 部門 / 人員(Rust API)
    let depts = RwSignal::new(Vec::<Dept>::new());
    let members = RwSignal::new(Vec::<Member>::new());
    let loading = RwSignal::new(String::new());
    spawn_local(async move {
        loading.set("載入部門…".into());
        match Request::get("/api/departments").send().await {
            Ok(r) => depts.set(r.json().await.unwrap_or_default()),
            Err(e) => loading.set(format!("API error: {e}")),
        }
        loading.set(String::new());
    });

    let fields_view = fields.clone();
    let dept_field = fields.iter().find(|f| f.spec.name == "dept").unwrap().clone();

    let on_dept = move |ev: leptos::ev::Event| {
        let id = event_target_value(&ev);
        dept_field.text.set(id.clone());
        spawn_local(async move {
            loading.set("載入人員…".into());
            match Request::get(&format!("/api/members/{id}")).send().await {
                Ok(r) => members.set(r.json().await.unwrap_or_default()),
                Err(e) => loading.set(format!("API error: {e}")),
            }
            loading.set(String::new());
        });
    };

    view! {
        <div class="form">
            {fields_view
                .iter()
                .cloned()
                .map(|f| {
                    let spec = f.spec.clone();
                    let widget: &str = &spec.widget;
                    let inner = match widget {
                        "text" => view! {
                            <input type="text" class="w-text"
                                prop:value=move || f.text.get()
                                on:input=move |ev| f.text.set(event_target_value(&ev)) />
                        }.into_any(),
                        "number" => view! {
                            <input type="number" class="w-number"
                                prop:value=move || f.num.get().to_string()
                                on:input=move |ev| f.num.set(event_target_value(&ev).parse().unwrap_or(0.0)) />
                        }.into_any(),
                        "dept-select" => view! {
                            <select class="w-dept" on:change=on_dept.clone()>
                                <option value="">"— 選擇部門 —"</option>
                                {move || depts.get().into_iter()
                                    .map(|d| view! { <option value=d.id.to_string()>{d.name}</option> })
                                    .collect_view()}
                            </select>
                        }.into_any(),
                        "radio" => view! {
                            <span class="w-radio">
                                {spec.options.iter().cloned().map(|o| {
                                    let ov = o.clone();
                                    view! {
                                        <label>
                                            <input type="radio" name=spec.name.clone()
                                                prop:checked=move || f.text.get() == ov
                                                on:change={let o2 = o.clone(); move |_| f.text.set(o2.clone())} />
                                            {o.clone()}
                                        </label>
                                    }
                                }).collect_view()}
                            </span>
                        }.into_any(),
                        "checkbox" => view! {
                            <input type="checkbox" class="w-check"
                                prop:checked=move || f.flag.get()
                                on:change=move |_| f.flag.update(|v| *v = !*v) />
                        }.into_any(),
                        "date" => view! {
                            <input type="date" class="w-date"
                                prop:value=move || f.text.get()
                                on:input=move |ev| f.text.set(event_target_value(&ev)) />
                        }.into_any(),
                        "range" => view! {
                            <span class="w-range">
                                <input type="range" min="0" max="30" step="1"
                                    prop:value=move || f.num.get().to_string()
                                    on:input=move |ev| f.num.set(event_target_value(&ev).parse().unwrap_or(0.0)) />
                                <span>{move || format!("{:.0}%", f.num.get())}</span>
                            </span>
                        }.into_any(),
                        "computed" => view! {
                            <span class="w-computed">{move || format!("{:.0}", f.num.get())}</span>
                        }.into_any(),
                        "textarea" => view! {
                            <textarea class="w-textarea" rows="3"
                                prop:value=move || f.text.get()
                                on:input=move |ev| f.text.set(event_target_value(&ev))></textarea>
                        }.into_any(),
                        _ => view! { <span>"(unknown widget)"</span> }.into_any(),
                    };
                    let err = spec.err.clone().unwrap_or_default();
                    // schema 的 style 只能引用 token;raw CSS / 未授權屬性在此被拒
                    let (style_attr, style_err) = match &spec.style {
                        Some(m) => match style_of(m) {
                            Ok(s) => (s, String::new()),
                            Err(e) => (String::new(), e),
                        },
                        None => (String::new(), String::new()),
                    };
                    view! {
                        <div class="field" style=style_attr class:invalid=move || !f.valid.get()>
                            <label class="f-label">{spec.label.clone()}</label>
                            {inner}
                            <Show when=move || !f.valid.get()>
                                <span class="f-err">{err.clone()}</span>
                            </Show>
                            {(!style_err.is_empty()).then(|| view! {
                                <span class="f-err">"style: "{style_err.clone()}</span>
                            })}
                        </div>
                    }
                })
                .collect_view()}
        </div>

        <h2>"人員列表(選部門後由 Rust API 載入)" <span class="loading">{move || loading.get()}</span></h2>
        <table class="members">
            <thead><tr><th>"姓名"</th><th>"職稱"</th></tr></thead>
            <tbody>
                {move || members.get().into_iter()
                    .map(|m| view! { <tr><td>{m.name}</td><td>{m.title}</td></tr> })
                    .collect_view()}
            </tbody>
        </table>
    }
}
