//! live_tab.rs — LiveUI: the live-manifestation loop (docs §18 gates + layers,
//! assembled). One schema declares cells (seeds), a widget tree, patches, and
//! wires; the host renders the tree, routes events to cells, cascades outputs
//! along the bus, and lets cell verdicts gate structural patches. Every cell
//! is fuel-metered and supervised: a runaway loop traps, degrades, and
//! quarantines — the page never freezes.

use crate::bus::{self, Wire};
use crate::cell::{cache_stats, Cell};
use crate::patch;
use crate::supervisor::{Health, Supervised};
use gloo_net::http::Request;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde_json::Value;
use std::collections::HashMap;

const FUEL_BUDGET: u32 = 200_000;

const DEFAULT_SCHEMA: &str = r#"{
  "cells": [
    {"id":"celsius","params":["x"],"script":"x"},
    {"id":"fahrenheit","params":["x"],"script":"x * 1.8 + 32.0"},
    {"id":"hot","params":["x"],"script":"let r = 0.0;\nif x >= 86.0 { r = 1.0; }\nr"},
    {"id":"always","params":["x"],"script":"1.0"}
  ],
  "wires": [
    {"from":"celsius","to":"fahrenheit"}
  ],
  "tree": {
    "type":"stack","children":[
      {"type":"label","text":"Pipeline: slider (°C) cascades over a wire to °F; the hot-gate cell's verdict gates patch 0."},
      {"type":"row","children":[
        {"type":"slider","min":0,"max":60,"step":1,"on_input":{"cell":"celsius"}},
        {"type":"value","bind":"celsius","prefix":"°C "},
        {"type":"value","bind":"fahrenheit","prefix":"°F "}
      ]},
      {"type":"row","children":[
        {"type":"button","text":"hot? patch the headline","on_click":{"cell":"hot","arg_from":"fahrenheit","patch":0}},
        {"type":"button","text":"manifest a note row","on_click":{"cell":"always","patch":1}},
        {"type":"button","text":"un-manifest it","on_click":{"cell":"always","patch":2}}
      ]}
    ]
  },
  "patches": [
    {"op":"update","path":[0],"props":{"text":"HOT (≥30°C / 86°F) — this headline was patched because the hot-gate cell returned 1.0"}},
    {"op":"add","path":[3],"node":{"type":"label","text":"manifested by a patch — structure changed with no reload, validated against the vocabulary"}},
    {"op":"remove","path":[3]}
  ]
}"#;

/// Sum of squares of memory slots 0..n — the memory-ABI demo kernel.
const MEM_KERNEL: &str = "let i = 0.0;\nlet s = 0.0;\nwhile i < n {\n    s = s + load(i) * load(i);\n    i = i + 1.0;\n}\ns";

#[derive(Clone, Copy)]
struct Ctx {
    cells: RwSignal<HashMap<String, Supervised>, LocalStorage>,
    outputs: RwSignal<HashMap<String, f64>>,
    tree: RwSignal<Value>,
    patches: RwSignal<Vec<Value>>,
    wires: RwSignal<Vec<Wire>>,
    bus_msg: RwSignal<String>,
}

fn make_cell(params: Vec<String>, script: String, mem_pages: Option<u32>) -> Supervised {
    Supervised::new(move || {
        let p: Vec<&str> = params.iter().map(|s| s.as_str()).collect();
        let mut b = Cell::builder(&p)
            .cap1("sin", f64::sin)
            .cap1("cos", f64::cos)
            .fuel(FUEL_BUDGET);
        if let Some(pages) = mem_pages {
            b = b.memory(pages);
        }
        b.compile(&script)
    })
}

fn apply_schema(ctx: Ctx, text: &str) -> Result<(), String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("schema parse failed: {e}"))?;
    let defs = v
        .get("cells")
        .and_then(|c| c.as_array())
        .ok_or("schema lacks \"cells\" []")?;
    let mut map = HashMap::new();
    for c in defs {
        let id = c["id"].as_str().ok_or("a cell lacks \"id\"")?.to_string();
        let script = c["script"].as_str().ok_or("a cell lacks \"script\"")?.to_string();
        let params: Vec<String> = c
            .get("params")
            .and_then(|p| p.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_else(|| vec!["x".into()]);
        let mem = c.get("memory").and_then(|m| m.as_u64()).map(|p| p as u32);
        map.insert(id, make_cell(params, script, mem));
    }
    let tree = v.get("tree").cloned().ok_or("schema lacks \"tree\"")?;
    patch::validate_node(&tree)?;
    let patches: Vec<Value> = v
        .get("patches")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();
    let wires: Vec<Wire> = v
        .get("wires")
        .and_then(|w| w.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|w| {
                    Some(Wire {
                        from: w.get("from")?.as_str()?.to_string(),
                        to: w.get("to")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    ctx.cells.set(map);
    ctx.tree.set(tree);
    ctx.patches.set(patches);
    ctx.wires.set(wires);
    ctx.outputs.set(HashMap::new());
    ctx.bus_msg.set(String::new());
    Ok(())
}

/// Run one cell through an event, cascade along the bus, maybe apply a
/// verdict-gated patch. This function IS the live-manifestation loop.
fn fire(ctx: Ctx, id: String, arg: f64, patch_idx: Option<usize>) {
    let origin = ctx
        .cells
        .try_update(|m| m.get_mut(&id).map(|s| s.call(&[arg])))
        .flatten();
    let Some(v) = origin else {
        ctx.bus_msg.set(format!("cell '{id}' not found"));
        return;
    };
    ctx.outputs.update(|o| {
        o.insert(id.clone(), v);
    });

    let wires = ctx.wires.get_untracked();
    let report = bus::dispatch(&wires, &id, v, bus::DEFAULT_BUDGET, |cid, x| {
        let out = ctx
            .cells
            .try_update(|m| m.get_mut(cid).map(|s| s.call(&[x])))
            .flatten();
        if let Some(o) = out {
            let cid = cid.to_string();
            ctx.outputs.update(|m| {
                m.insert(cid, o);
            });
        }
        out
    });
    ctx.bus_msg.set(if report.overflow {
        format!(
            "bus: {} dispatches — budget hit, cascade cut (cycle/storm contained)",
            report.dispatches
        )
    } else {
        format!("bus: {} dispatches", report.dispatches)
    });

    if let Some(pi) = patch_idx {
        if v != 0.0 {
            let p = ctx.patches.get_untracked().get(pi).cloned();
            match p {
                Some(p) => {
                    let mut res = Ok(());
                    ctx.tree.update(|t| res = patch::apply_patch(t, &p));
                    match res {
                        Ok(()) => ctx.bus_msg.update(|m| m.push_str(&format!(" · patch {pi} applied"))),
                        Err(e) => ctx.bus_msg.set(format!("patch {pi} rejected: {e}")),
                    }
                }
                None => ctx.bus_msg.set(format!("patch {pi} does not exist")),
            }
        } else {
            ctx.bus_msg
                .update(|m| m.push_str(&format!(" · verdict 0.0 — patch {pi} withheld")));
        }
    }
}

fn fire_event(ctx: Ctx, spec: &Value, event_val: Option<f64>) {
    let Some(cell) = spec.get("cell").and_then(|c| c.as_str()) else {
        return;
    };
    let arg = if let Some(src) = spec.get("arg_from").and_then(|a| a.as_str()) {
        ctx.outputs
            .with_untracked(|o| o.get(src).copied().unwrap_or(0.0))
    } else if let Some(a) = spec.get("arg").and_then(|a| a.as_f64()) {
        a
    } else {
        event_val.unwrap_or(1.0)
    };
    let patch_idx = spec.get("patch").and_then(|p| p.as_u64()).map(|i| i as usize);
    fire(ctx, cell.to_string(), arg, patch_idx);
}

fn render_node(node: &Value, ctx: Ctx) -> AnyView {
    let t = node.get("type").and_then(|v| v.as_str()).unwrap_or("?");
    match t {
        "stack" | "row" => {
            let class = if t == "stack" { "lv-stack" } else { "lv-row" };
            let kids = node
                .get("children")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            view! {
                <div class=class>{kids.iter().map(|k| render_node(k, ctx)).collect_view()}</div>
            }
            .into_any()
        }
        "label" => {
            let text = node.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
            view! { <p class="lv-label">{text}</p> }.into_any()
        }
        "value" => {
            let bind = node.get("bind").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let prefix = node.get("prefix").and_then(|v| v.as_str()).unwrap_or("").to_string();
            view! {
                <span class="lv-value">
                    {prefix}
                    <b>{move || {
                        format!("{:.2}", ctx.outputs.with(|o| o.get(&bind).copied().unwrap_or(0.0)))
                    }}</b>
                </span>
            }
            .into_any()
        }
        "button" => {
            let text = node.get("text").and_then(|v| v.as_str()).unwrap_or("run").to_string();
            let spec = node.get("on_click").cloned();
            view! {
                <button class="lv-button" on:click=move |_| {
                    if let Some(s) = &spec { fire_event(ctx, s, None); }
                }>{text}</button>
            }
            .into_any()
        }
        "slider" => {
            let min = node.get("min").and_then(|v| v.as_f64()).unwrap_or(0.0).to_string();
            let max = node.get("max").and_then(|v| v.as_f64()).unwrap_or(100.0).to_string();
            let step = node.get("step").and_then(|v| v.as_f64()).unwrap_or(1.0).to_string();
            let spec = node.get("on_input").cloned();
            view! {
                <input type="range" class="lv-slider" min=min max=max step=step
                    on:input=move |ev| {
                        let v: f64 = event_target_value(&ev).parse().unwrap_or(0.0);
                        if let Some(s) = &spec { fire_event(ctx, s, Some(v)); }
                    } />
            }
            .into_any()
        }
        "input" => {
            let placeholder = node
                .get("placeholder")
                .and_then(|v| v.as_str())
                .unwrap_or("number")
                .to_string();
            let spec = node.get("on_input").cloned();
            view! {
                <input type="text" class="lv-input" placeholder=placeholder
                    on:input=move |ev| {
                        if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                            if let Some(s) = &spec { fire_event(ctx, s, Some(v)); }
                        }
                    } />
            }
            .into_any()
        }
        other => view! {
            <div class="cell-err">{format!("unknown node '{other}' — vocabulary: {}", patch::VOCAB.join("/"))}</div>
        }
        .into_any(),
    }
}

#[component]
pub fn LivePoc() -> impl IntoView {
    let ctx = Ctx {
        cells: RwSignal::new_local(HashMap::new()),
        outputs: RwSignal::new(HashMap::new()),
        tree: RwSignal::new(Value::Null),
        patches: RwSignal::new(Vec::new()),
        wires: RwSignal::new(Vec::new()),
        bus_msg: RwSignal::new(String::new()),
    };
    let schema_text = RwSignal::new(String::new());
    let err = RwSignal::new(String::new());

    let apply = move |text: &str| match apply_schema(ctx, text) {
        Ok(()) => err.set(String::new()),
        Err(e) => err.set(e),
    };

    // Load from the API (edit the file on disk + reload = new UI); fall back
    // to the embedded default so the tab works without the server too.
    spawn_local(async move {
        let text = match Request::get("/api/live-schema").send().await {
            Ok(r) if r.ok() => r.text().await.unwrap_or_else(|_| DEFAULT_SCHEMA.to_string()),
            _ => DEFAULT_SCHEMA.to_string(),
        };
        schema_text.set(text.clone());
        apply(&text);
    });

    // Sabotage button: swap a pipeline cell's seed for a runaway loop. Fuel
    // traps it, the supervisor degrades → quarantines it, the page stays live.
    let inject_bad = move |_| {
        ctx.cells.update(|m| {
            m.insert(
                "fahrenheit".into(),
                make_cell(vec!["x".into()], "while 0.0 < 1.0 { }\n0.0".into(), None),
            );
        });
        ctx.bus_msg
            .set("injected a runaway loop into 'fahrenheit' — move the slider".into());
    };

    // Memory-ABI demo: host writes 1..=N into the cell's own memory, the cell
    // sums the squares via load(); cross-checked against the host.
    let mem_n = RwSignal::new(64i32);
    let mem_msg = RwSignal::new(String::new());
    let run_mem = move |_| {
        let n = mem_n.get_untracked().max(1) as u32;
        let built = Cell::builder(&["n"]).fuel(2_000_000).memory(1).compile(MEM_KERNEL);
        match built {
            Ok(c) => {
                let data: Vec<f64> = (1..=n).map(|k| k as f64).collect();
                if let Err(e) = c.write_mem(0, &data) {
                    mem_msg.set(format!("write_mem: {e}"));
                    return;
                }
                match c.call(&[n as f64]) {
                    Ok(v) => {
                        let host: f64 = data.iter().map(|x| x * x).sum();
                        let fuel = c.fuel_used().unwrap_or(0.0);
                        mem_msg.set(format!(
                            "cell Σx² = {v} · host Σx² = {host} · consistent = {} · fuel used {fuel}",
                            v == host
                        ));
                    }
                    Err(e) => mem_msg.set(format!("call: {e}")),
                }
            }
            Err(e) => mem_msg.set(format!("compile: {e}")),
        }
    };

    view! {
        <p class="sub">
            "The live-manifestation loop: one schema declares seeds (cells), a widget tree, patches, and wires. "
            "Events run cells; outputs cascade along the bus (budgeted — a cycle degrades into a report, not a hang); "
            "a cell's verdict gates structural patches (validated against the vocabulary before touching the tree). "
            "Every cell is fuel-metered and supervised: a runaway loop traps → degrades → quarantines. The page never freezes."
        </p>

        <div class="ly-card lv-canvas">
            {move || {
                let t = ctx.tree.get();
                if t.is_null() {
                    view! { <p class="lv-label">"loading schema…"</p> }.into_any()
                } else {
                    render_node(&t, ctx)
                }
            }}
        </div>

        <div class="lv-statusbar">
            <span class="lv-bus">{move || ctx.bus_msg.get()}</span>
            <span class="lv-cache">{move || {
                let (h, m) = cache_stats();
                format!("module cache: {h} hits / {m} misses")
            }}</span>
        </div>

        <div class="ly-card">
            <h3>"Cell health (supervision)"</h3>
            <div class="live-health">
                {move || {
                    let mut rows: Vec<(String, Health, u32, String, Option<f64>)> =
                        ctx.cells.with(|m| {
                            m.iter()
                                .map(|(id, s)| {
                                    (
                                        id.clone(),
                                        s.health(),
                                        s.failures,
                                        s.last_error.clone(),
                                        s.cell().and_then(|c| c.fuel_used()),
                                    )
                                })
                                .collect()
                        });
                    rows.sort_by(|a, b| a.0.cmp(&b.0));
                    rows.into_iter()
                        .map(|(id, health, fails, error, fuel)| {
                            let chip = format!("live-chip {}", health.label());
                            let restart_id = id.clone();
                            view! {
                                <div class=chip>
                                    <b>{id.clone()}</b>
                                    <span>{health.label()}</span>
                                    <span>{move || if fails > 0 { format!("{fails} traps") } else { String::new() }}</span>
                                    <span class="lv-fuel">{fuel.map(|f| format!("fuel {f}")).unwrap_or_default()}</span>
                                    <Show when={
                                        let e = error.clone();
                                        move || !e.is_empty()
                                    }>
                                        <span class="lv-err">{error.clone()}</span>
                                    </Show>
                                    <button class="lv-restart" on:click=move |_| {
                                        ctx.cells.update(|m| {
                                            if let Some(s) = m.get_mut(&restart_id) { s.restart(); }
                                        });
                                    }>"restart"</button>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <button class="tok-violate lv-inject" on:click=inject_bad>
                "inject a runaway loop into 'fahrenheit' (fuel will trap it)"
            </button>
        </div>

        <div class="ly-card">
            <h3>"Memory capability (buffer ABI)"</h3>
            <div class="lv-row">
                "N = " <input type="number" class="lv-mem-n" min="1" max="4096"
                    prop:value=move || mem_n.get().to_string()
                    on:input=move |ev| mem_n.set(event_target_value(&ev).parse().unwrap_or(64)) />
                <button class="lv-mem-run" on:click=run_mem>"host writes 1..=N → cell sums squares via load()"</button>
            </div>
            <p class="lv-label lv-mem-out">{move || mem_msg.get()}</p>
        </div>

        <div class="ly-card">
            <h3>"Schema (cells + tree + patches + wires — served from api-server/live-schema.json)"</h3>
            <textarea class="lv-schema" rows="14"
                prop:value=move || schema_text.get()
                on:input=move |ev| schema_text.set(event_target_value(&ev))></textarea>
            <button class="apply lv-apply" on:click=move |_| apply(&schema_text.get_untracked())>"Apply schema"</button>
            <Show when=move || !err.get().is_empty()>
                <div class="cell-err">{move || err.get()}</div>
            </Show>
        </div>
    }
}
