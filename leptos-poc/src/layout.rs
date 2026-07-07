//! layout.rs — layout as schema: the whole app layout (header/menu/profile/table)
//! is manifested at runtime from a recursive JSON tree.
//!
//! Same pattern as form.rs, one level up: a form schema is a "flat list of fields",
//! a layout schema is a "recursive tree of containers". Vocabulary = 9 layout cells
//! (the compile-time substance); composition = the schema (the runtime use); styles,
//! as before, may only reference design tokens.
//! A layout surface shouldn't be drawn with drawing primitives — text/scrolling/focus/accessibility belong to the DOM.

use crate::tokens::style_of;
use gloo_net::http::Request;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Column {
    key: String,
    label: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Node {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    items: Vec<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    columns: Vec<Column>,
    #[serde(default)]
    children: Vec<Node>,
    #[serde(default)]
    style: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Recursively manifest one layout node. An unknown node type = not in the vocabulary → show an error (rather than silently dropping it).
fn render(node: Node) -> AnyView {
    let style = node
        .style
        .as_ref()
        .map(|m| style_of(m).unwrap_or_else(|e| { let _ = e; String::new() }))
        .unwrap_or_default();
    let style_err = node
        .style
        .as_ref()
        .and_then(|m| style_of(m).err())
        .unwrap_or_default();

    let inner: AnyView = match node.kind.as_str() {
        "shell" => view! {
            <div class="ly-shell" style=style.clone()>
                {node.children.into_iter().map(render).collect_view()}
            </div>
        }
        .into_any(),
        "header" => view! {
            <div class="ly-header" style=style.clone()>
                <b>{node.title.unwrap_or_default()}</b>
                <nav>{node.items.into_iter().map(|i| view! { <span>{i}</span> }).collect_view()}</nav>
            </div>
        }
        .into_any(),
        "side" => view! {
            <div class="ly-side" style=style.clone()>
                {node.children.into_iter().map(render).collect_view()}
            </div>
        }
        .into_any(),
        "main" => view! {
            <div class="ly-main" style=style.clone()>
                {node.children.into_iter().map(render).collect_view()}
            </div>
        }
        .into_any(),
        "card" => view! {
            <div class="ly-card" style=style.clone()>
                {node.title.map(|t| view! { <h3>{t}</h3> })}
                {node.children.into_iter().map(render).collect_view()}
            </div>
        }
        .into_any(),
        "menu" => view! {
            <div class="ly-card">
                <ul class="ly-menu" style=style.clone()>
                    {node.items.into_iter().map(|i| view! { <li>{i}</li> }).collect_view()}
                </ul>
            </div>
        }
        .into_any(),
        "profile" => {
            let name = node.name.unwrap_or_default();
            let initial = name.chars().next().map(String::from).unwrap_or_default();
            view! {
                <div class="ly-card ly-profile" style=style.clone()>
                    <div class="avatar">{initial}</div>
                    <div class="who">
                        <b>{name}</b>
                        <span>{node.text.unwrap_or_default()}</span>
                    </div>
                </div>
            }
            .into_any()
        }
        "table" => view! {
            <LyTable source=node.source.unwrap_or_default() columns=node.columns />
        }
        .into_any(),
        "text" => view! { <p class="ly-text" style=style.clone()>{node.text.unwrap_or_default()}</p> }
            .into_any(),
        other => view! {
            <div class="cell-err">{format!("unknown layout node '{other}' — vocabulary: shell/header/side/main/card/menu/profile/table/text")}</div>
        }
        .into_any(),
    };

    if style_err.is_empty() {
        inner
    } else {
        view! {
            <div>
                {inner}
                <div class="cell-err">"style: "{style_err}</div>
            </div>
        }
        .into_any()
    }
}

/// Table cell: rows are loaded at runtime from the API source named in the schema.
#[component]
fn LyTable(source: String, columns: Vec<Column>) -> impl IntoView {
    let rows: RwSignal<Vec<serde_json::Value>> = RwSignal::new(Vec::new());
    let err = RwSignal::new(String::new());
    {
        let source = source.clone();
        spawn_local(async move {
            match Request::get(&source).send().await {
                Ok(r) => rows.set(r.json().await.unwrap_or_default()),
                Err(e) => err.set(format!("table source error: {e}")),
            }
        });
    }
    let heads = columns.clone();
    view! {
        <table class="ly-table">
            <thead>
                <tr>{heads.into_iter().map(|c| view! { <th>{c.label}</th> }).collect_view()}</tr>
            </thead>
            <tbody>
                {move || {
                    let cols = columns.clone();
                    rows.get()
                        .into_iter()
                        .map(|row| {
                            let cols = cols.clone();
                            view! {
                                <tr>
                                    {cols
                                        .into_iter()
                                        .map(|c| {
                                            let v = row
                                                .get(&c.key)
                                                .and_then(|v| v.as_str().map(String::from))
                                                .unwrap_or_default();
                                            view! { <td>{v}</td> }
                                        })
                                        .collect_view()}
                                </tr>
                            }
                        })
                        .collect_view()
                }}
            </tbody>
        </table>
        <Show when=move || !err.get().is_empty()>
            <div class="cell-err">{move || err.get()}</div>
        </Show>
    }
}

#[component]
pub fn LayoutPoc() -> impl IntoView {
    let root: RwSignal<Option<Node>> = RwSignal::new(None);
    let err = RwSignal::new(String::new());
    let load = move || {
        spawn_local(async move {
            match Request::get("/api/layout-schema").send().await {
                Ok(r) => match r.json::<Node>().await {
                    Ok(n) => {
                        root.set(Some(n));
                        err.set(String::new());
                    }
                    Err(e) => err.set(format!("layout schema parse failed: {e}")),
                },
                Err(e) => err.set(format!("layout schema API error: {e}")),
            }
        })
    };
    load();

    view! {
        <p class="sub">
            "The whole app layout (header/menu/profile/table) is manifested from the recursive tree in api-server/layout-schema.json; "
            "the table's rows are then loaded from the API source named in the schema. Edit file → reload → the layout changes, zero rebuild."
        </p>
        <button class="apply reload-layout" on:click=move |_| load()>"Reload layout"</button>
        <Show when=move || !err.get().is_empty()>
            <div class="cell-err">{move || err.get()}</div>
        </Show>
        {move || root.get().map(render)}
    }
}
