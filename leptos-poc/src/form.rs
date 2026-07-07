//! form.rs — apply the DynamicCell pattern across a whole form (one of each widget).
//!
//! - Structure = FORM_SCHEMA (JSON) → interpreted by a static renderer (structure is data)
//! - Behavior = a numeric field's validation rule / computed field is a DSL seed → a wasm-jit cell
//!   (string validation stays in the host — the §16 boundary discipline: strings/objects don't sink down)
//! - Department dropdown = calls the Rust API (Axum) /api/departments on mount;
//!   picking a department → /api/members/{id} loads the member list

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
    /// Style: may only reference design tokens (validated by tokens.rs); raw CSS is rejected.
    #[serde(default)]
    style: Option<serde_json::Map<String, serde_json::Value>>,
}

/// One set of generic signals per field (text/num/flag, used per widget) + validation state.
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

/// Rule cell: run(v) -> 1.0/0.0; computed cell: run(<params>) -> value.
/// The grant is always only sin/cos/out — so form logic can't reach the DOM/network either.
fn build_cell(params: &[&str], src: &str) -> Result<Cell, String> {
    Cell::builder(params)
        .cap1("sin", f64::sin)
        .cap1("cos", f64::cos)
        .cap2_void("out", |_, _| {})
        .compile(src)
}

fn wire_rules(fields: &Rc<Vec<FieldRt>>) {
    for f in fields.iter() {
        // Validation rule: numeric field → cell decides 1.0/0.0
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
                Err(_) => f.valid.set(false), // the schema rule itself is broken: always mark invalid so it surfaces
            }
        }
        // Computed field: a param field changes → cell recomputes → write back to this field's num
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

/// The form schema is not in the Rust source at all: it's loaded at runtime via
/// GET /api/form-schema (the server reads api-server/form-schema.json fresh per request).
/// Edit the file → reload → the form changes, zero rebuild.
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
                    Err(e) => schema_err.set(format!("schema parse failed: {e}")),
                },
                Err(e) => schema_err.set(format!("schema API error: {e}")),
            }
        })
    };
    load();

    view! {
        <p class="sub">
            "This form's schema is not in the Rust source — it is loaded at runtime via GET /api/form-schema, "
            "which the server reads from api-server/form-schema.json per request. Edit that file, hit reload, and the form changes — zero rebuild."
        </p>
        <button class="apply reload-schema" on:click=move |_| load()>"Reload schema"</button>
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

    // Departments / members (Rust API)
    let depts = RwSignal::new(Vec::<Dept>::new());
    let members = RwSignal::new(Vec::<Member>::new());
    let loading = RwSignal::new(String::new());
    spawn_local(async move {
        loading.set("Loading departments…".into());
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
            loading.set("Loading people…".into());
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
                                <option value="">"— Select a department —"</option>
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
                    // the schema's style may only reference tokens; raw CSS / unauthorized properties are rejected here
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

        <h2>"People (loaded by the Rust API after choosing a department)" <span class="loading">{move || loading.get()}</span></h2>
        <table class="members">
            <thead><tr><th>"Name"</th><th>"Title"</th></tr></thead>
            <tbody>
                {move || members.get().into_iter()
                    .map(|m| view! { <tr><td>{m.name}</td><td>{m.title}</td></tr> })
                    .collect_view()}
            </tbody>
        </table>
    }
}
