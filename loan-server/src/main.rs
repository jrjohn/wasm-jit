//! Server-side compute — the mirror image of `apps/form.html`.
//!
//! In `form.html` the loan cells travel to the browser as plaintext DSL (view-source
//! sees the amortization formula) and the browser compiles + runs them. That is *safe*
//! — the cell can only do what its import table allows — but it is not *secret*.
//!
//! Here the exact same cells stay on the server. The browser POSTs only the intent
//! `{p, r, y}` and receives only the numbers. The formula never appears in any byte the
//! client can download. Two deployments of one vocabulary:
//!
//!   form.html      : cell on the client — readable, and safe by construction
//!   loan-server    : cell on the server  — unreadable, and safe by construction
//!
//! The fence is identical in both: the loan cells are compiled with `wasm-jit`'s native
//! `UI_IMPORTS` (sin, cos, get, set, ld, sd) — the same table the browser uses — and run
//! under a wasmi `Linker` that offers *only* those six host functions. A cell that reached
//! for a socket, the filesystem, or `fetch` could not have compiled and could not link.

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tower_http::services::ServeDir;
use wasm_jit::{codegen, parser, UI_IMPORTS, UI_PARAMS};
use wasmi::{Caller, Engine, Linker, Module, Store};

/// The host data root every UI cell shares: a 32-slot f64 array (get/set) and the
/// 4096-slot collection store (ld/sd). This is the cell's ENTIRE world — there is no
/// field on it for a file handle, a socket, or a clock, because the fence grants none.
struct Host {
    slots: [f64; 32],
    coll: Vec<f64>,
}

impl Host {
    fn new() -> Self {
        Host { slots: [0.0; 32], coll: vec![0.0; 4096] }
    }
}

/// The six host functions that ARE the UI capability fence, wired to `Host`. The linker
/// exposes these and nothing else; `wasm-jit` will not emit an import this list omits, so
/// compile-time reach and run-time reach are the same closed set.
fn fenced_linker(engine: &Engine) -> Linker<Host> {
    let mut l: Linker<Host> = Linker::new(engine);
    l.func_wrap("env", "sin", |_: Caller<'_, Host>, x: f64| x.sin()).unwrap();
    l.func_wrap("env", "cos", |_: Caller<'_, Host>, x: f64| x.cos()).unwrap();
    l.func_wrap("env", "get", |c: Caller<'_, Host>, i: f64| c.data().slots[(i as usize) & 31])
        .unwrap();
    l.func_wrap("env", "set", |mut c: Caller<'_, Host>, i: f64, v: f64| {
        c.data_mut().slots[(i as usize) & 31] = v;
    })
    .unwrap();
    l.func_wrap("env", "ld", |c: Caller<'_, Host>, i: f64| {
        let n = c.data().coll.len();
        c.data().coll[(i as usize) % n]
    })
    .unwrap();
    l.func_wrap("env", "sd", |mut c: Caller<'_, Host>, i: f64, v: f64| {
        let n = c.data_mut().coll.len();
        c.data_mut().coll[(i as usize) % n] = v;
    })
    .unwrap();
    l
}

/// A compiled loan cell: its id, its wasm bytes, and the imports it actually reaches for
/// (parsed back out of the bytes — the machine proof that reach ⊆ fence).
struct Cell {
    id: String,
    module: Module,
    bytes: Vec<u8>,
    bytes_len: usize,
    imports: Vec<String>,
}

/// Compile one DSL source under the UI fence, exactly as the browser's `compile_ui_cell_wasm`
/// does (same params, same imports, same 200k fuel), then read the module's own import
/// table back and assert every import lives in the fence. Panics at boot on over-reach —
/// the server refuses to start holding a cell it cannot contain.
fn compile_cell(engine: &Engine, id: &str, src: &str) -> Cell {
    let prog = parser::parse(src).unwrap_or_else(|e| panic!("cell '{id}' failed to parse: {e}"));
    let bytes = codegen::compile_with_opts(
        &prog,
        &UI_PARAMS,
        &UI_IMPORTS,
        codegen::CompileOpts { fuel: Some(200_000), memory_pages: None },
    )
    .unwrap_or_else(|e| panic!("cell '{id}' failed to compile: {e}"));

    let module = Module::new(engine, &bytes[..]).expect("wasmi accepts the compiled cell");

    // read the module's declared imports straight back out of it — do not trust, verify.
    // every import must live in the fence, or the server refuses to hold this cell.
    let fence: Vec<&str> = UI_IMPORTS.iter().map(|h| h.name).collect();
    let mut imports = Vec::new();
    for imp in module.imports() {
        let (m, name) = (imp.module(), imp.name());
        assert!(
            m == "env" && fence.contains(&name),
            "cell '{id}' reached outside the fence: {m}.{name}"
        );
        imports.push(format!("{m}.{name}"));
    }

    let bytes_len = bytes.len();
    Cell { id: id.to_string(), module, bytes, bytes_len, imports }
}

struct AppState {
    engine: Engine,
    monthly: Cell,
    total: Cell,
    interest: Cell,
    /// the fence, as a plain list, for the client to display honestly
    fence: Vec<String>,
}

impl AppState {
    /// look up a compiled cell by the id the browser asks for (variant-2: ship the .wasm).
    fn cell(&self, id: &str) -> Option<&Cell> {
        match id {
            "monthly" => Some(&self.monthly),
            "totalPaid" => Some(&self.total),
            "interest" => Some(&self.interest),
            _ => None,
        }
    }
}

/// Run one cell inside a shared store: instantiate it into `store` (so it sees the same
/// `Host` slots the previous cells wrote), call `run(0.0)`, return the f64. A fresh
/// instance per call means the fuel global starts full every time.
fn run_cell(store: &mut Store<Host>, linker: &Linker<Host>, cell: &Cell) -> Result<f64, String> {
    let inst = linker
        .instantiate(&mut *store, &cell.module)
        .map_err(|e| format!("instantiate {}: {e}", cell.id))?
        .start(&mut *store)
        .map_err(|e| format!("start {}: {e}", cell.id))?;
    let run = inst
        .get_typed_func::<f64, f64>(&*store, "run")
        .map_err(|e| format!("no run() on {}: {e}", cell.id))?;
    run.call(&mut *store, 0.0).map_err(|e| format!("trap in {}: {e}", cell.id))
}

#[derive(Deserialize)]
struct LoanReq {
    p: f64,
    r: f64,
    y: f64,
}

#[derive(Serialize)]
struct FenceCell {
    id: String,
    bytes: usize,
    imports: Vec<String>,
}

/// POST /api/loan — the only thing that crosses the wire toward the logic. Sets the data
/// root (slots 0..2) from the request, runs monthly → total → interest sharing one store,
/// and returns the numbers plus the fence evidence for each cell.
async fn api_loan(State(st): State<Arc<AppState>>, Json(req): Json<LoanReq>) -> impl IntoResponse {
    let linker = fenced_linker(&st.engine);
    let mut store = Store::new(&st.engine, Host::new());
    // the host owns the data root; the browser never sees these slots, only their result.
    store.data_mut().slots[0] = req.p;
    store.data_mut().slots[1] = req.r;
    store.data_mut().slots[2] = req.y;

    let mut compute = || -> Result<(f64, f64, f64), String> {
        let m = run_cell(&mut store, &linker, &st.monthly)?; // writes slot 3
        let t = run_cell(&mut store, &linker, &st.total)?; // reads slot 3
        let i = run_cell(&mut store, &linker, &st.interest)?;
        Ok((m, t, i))
    };

    match compute() {
        Ok((m, t, i)) => Json(json!({
            "monthly": round2(m),
            "total": round2(t),
            "interest": round2(i),
            // the honest receipt: what fenced machine produced each number
            "fence": {
                "imports_offered": st.fence,
                "cells": [
                    FenceCell { id: st.monthly.id.clone(),  bytes: st.monthly.bytes_len,  imports: st.monthly.imports.clone() },
                    FenceCell { id: st.total.id.clone(),    bytes: st.total.bytes_len,    imports: st.total.imports.clone() },
                    FenceCell { id: st.interest.id.clone(), bytes: st.interest.bytes_len, imports: st.interest.imports.clone() },
                ]
            }
        }))
        .into_response(),
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "error": e }))).into_response(),
    }
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

async fn index() -> impl IntoResponse {
    Html(include_str!("../index.html"))
}

/// The three-deployments comparison — variant 1 (DSL in the page), variant 2 (this page
/// fetches precompiled .wasm and runs it locally), variant 3 (server compute), side by side.
async fn compare() -> impl IntoResponse {
    Html(include_str!("../compare.html"))
}

/// A second cell demo: Taiwan reservoirs as particles that morph bar⇄pie⇄line, each
/// particle's target computed by a fenced UI cell compiled in the browser via /pkg.
async fn reservoirs() -> impl IntoResponse {
    Html(include_str!("../../apps/reservoirs.html"))
}

/// The fully cell-native twin: the whole shatter→fly→solidify trajectory (incl. a
/// coherence dimension) lives in ONE behavior cell; JS is a ~15-line dumb painter that
/// knows nothing about bar/pie/line — the same engine shape as moon4's per-tick entities.
async fn reservoirs_native() -> impl IntoResponse {
    Html(include_str!("../../apps/reservoirs-native.html"))
}

/// Cells that sense each other: each particle writes its position to the shared field
/// (ld/sd) and reads its neighbours', yielding on contact so they cooperatively fill the
/// gaps into a solid bar — Indra's Net gate 7 (jewel-reflecting-jewel), made whole.
async fn reservoirs_net() -> impl IntoResponse {
    Html(include_str!("../../apps/reservoirs-net.html"))
}

/// Cells declare richer primitives than points: a fill (中間填色) and edges (四邊成線),
/// so a handful of cells manifest a crisp solid bar/pie/line at rest, decomposing into
/// flying particles only during the morph. Gaps were a primitive problem, not a fill count.
async fn reservoirs_solid() -> impl IntoResponse {
    Html(include_str!("../../apps/reservoirs-solid.html"))
}

/// The end-to-end page: type an intent, an AI grows a fenced app on the spot.
async fn genapp() -> impl IntoResponse {
    Html(include_str!("../../apps/genapp.html"))
}

/// Compile a UI cell without panicking — the fence check as a Result, for self-repair.
fn try_compile_ui(src: &str) -> Result<usize, String> {
    let prog = parser::parse(src)?;
    let bytes = codegen::compile_with_opts(
        &prog,
        &UI_PARAMS,
        &UI_IMPORTS,
        codegen::CompileOpts { fuel: Some(200_000), memory_pages: None },
    )?;
    Ok(bytes.len())
}

/// The spec handed to the model. It describes the fenced DSL + the schema shape so the
/// model's whole job is to translate an intent into a declaration — it cannot express
/// anything outside the fence (and if it tries, the compile below rejects it).
const GEN_SPEC: &str = r#"Respond IMMEDIATELY with ONLY the JSON described below — nothing else. Do NOT use any tools, do NOT search the web, do NOT ask clarifying questions, do NOT explain. If the app needs real-world data you don't have (statistics, prices, etc.), just invent small representative sample numbers. This is a code-generation task, answer in one shot.

You translate a user's intent into a tiny UI app expressed as a JSON schema for a capability-fenced runtime. The app's compute is done by "cells" in a tiny DSL.

DSL (each cell is a function `run(x) -> f64`; the LAST line is the returned number):
- statements end with ; ; `let NAME = EXPR;`
- arithmetic + - * / ; comparisons >= > < <= (yield 1.0 / 0.0)
- `if COND { ... }` — there is NO else; use several guard `if`s instead
- `while COND { ... }`
- functions: sin(a), cos(a)
- STATE: get(n) reads shared slot n (0..31); set(n, v) writes it. ALL cells share these 32 slots — use them to hold values and pass data between cells.
- x is the event value (e.g. the number typed into an input, or 0 for a button).
- Comments: // only. NO other functions exist. NO fetch, network, DOM, files — impossible here.
- Numbers MUST be floats: write 2.0 not 2, 0.0 not 0, 1200.0 not 1200.

SCHEMA (output ONLY this JSON — no markdown fences, no prose):
{
 "title": "short title",
 "cells": [ {"id":"...", "script":"...DSL..."} ],
 "wires": [ {"from":"cellId","to":"cellId"} ],   // when 'from' runs, re-run 'to'
 "init":  [ {"cell":"cellId","arg": number} ],   // run once at start with x=arg
 "tree": { "type":"stack", "children":[ ...nodes... ] }
}
NODES:
 {"type":"stack","children":[...]}    // vertical
 {"type":"row","children":[...]}      // horizontal
 {"type":"label","text":"..."}
 {"type":"input","placeholder":"...","on_input":{"cell":"id"}}   // number input; typing runs cell with x=value
 {"type":"value","label":"...","bind":"cellId"}                  // shows cellId's latest return (2 decimals)
 {"type":"button","text":"...","on_click":{"cell":"id"}}         // runs cell with x=0
 {"type":"slider","min":0,"max":100,"step":1,"label":"...","on_input":{"cell":"id"}}
 {"type":"pie","slices":[{"label":"美國","bind":"usa"},{"label":"中國","bind":"china"}]}   // pie chart; each slice's ANGLE = its bound cell's return value (percentages auto-computed & drawn inside each slice)
 {"type":"bar","slices":[{"label":"...","bind":"cellId"}]}                                    // bar chart; each bar's HEIGHT = its bound cell's return value

PATTERN: an input writes its value into a slot via `set(n, x)` then returns x; each compute cell reads slots via get(n); a value node binds to a compute cell; wire every input to every compute cell that depends on it.
CHART PATTERN: for a pie/bar of fixed data, make ONE cell per slice returning that slice's raw number (e.g. {"id":"usa","script":"35.0"}), and bind each slice to its cell. Do NOT precompute percentages or bake labels like "35%" into text — the pie draws the shares and % itself. Use a real "pie"/"bar" node, never fake a chart with rows of labels.

EXAMPLE (loan calculator):
{"title":"房貸試算","cells":[{"id":"setP","script":"set(0.0, x);\nx"},{"id":"setR","script":"set(1.0, x);\nx"},{"id":"setY","script":"set(2.0, x);\nx"},{"id":"monthly","script":"let P = get(0.0);\nlet r = get(1.0) / 1200.0;\nlet n = get(2.0) * 12.0;\nlet M = 0.0;\nif n >= 1.0 {\n if r < 0.0000001 { M = P / n; }\n if r >= 0.0000001 {\n  let pw = 1.0;\n  let i = 0.0;\n  while i < n { pw = pw * (1.0 + r); i = i + 1.0; }\n  M = P * r * pw / (pw - 1.0);\n }\n}\nM"}],"wires":[{"from":"setP","to":"monthly"},{"from":"setR","to":"monthly"},{"from":"setY","to":"monthly"}],"init":[{"cell":"setP","arg":300000},{"cell":"setR","arg":5},{"cell":"setY","arg":30}],"tree":{"type":"stack","children":[{"type":"label","text":"房貸試算"},{"type":"row","children":[{"type":"label","text":"本金"},{"type":"input","placeholder":"300000","on_input":{"cell":"setP"}}]},{"type":"row","children":[{"type":"label","text":"利率%"},{"type":"input","placeholder":"5","on_input":{"cell":"setR"}}]},{"type":"row","children":[{"type":"label","text":"年數"},{"type":"input","placeholder":"30","on_input":{"cell":"setY"}}]},{"type":"row","children":[{"type":"label","text":"月付"},{"type":"value","bind":"monthly"}]}]}}
"#;

#[derive(Deserialize)]
struct GenReq {
    intent: String,
}

async fn call_claude(prompt: &str) -> Result<String, String> {
    let bin = std::env::var("CLAUDE_BIN").unwrap_or_else(|_| "/opt/homebrew/bin/claude".into());
    let fut = tokio::process::Command::new(&bin)
        .arg("-p")
        .arg("--model")
        .arg("claude-sonnet-5")
        .arg(prompt)
        .output();
    let out = tokio::time::timeout(std::time::Duration::from_secs(180), fut)
        .await
        .map_err(|_| "claude 逾時(>120s)".to_string())?
        .map_err(|e| format!("claude 啟動失敗: {e}"))?;
    if !out.status.success() {
        return Err(format!("claude 退出 {}: {}", out.status, String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn extract_json(s: &str) -> String {
    let s = s.trim();
    match (s.find('{'), s.rfind('}')) {
        (Some(a), Some(b)) if b > a => s[a..=b].to_string(),
        _ => s.to_string(),
    }
}

/// POST /api/gen — the whole loop: intent → claude -p → parse → compile-check every cell
/// under the UI fence → on failure, feed the error back and retry (self-repair) → return
/// the validated schema. The browser then runs it; the cells are fenced by construction.
async fn api_gen(Json(req): Json<GenReq>) -> Response {
    let intent = req.intent.trim().to_string();
    if intent.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "說一句話吧"}))).into_response();
    }
    let mut repair = String::new();
    let mut last_err = String::from("unknown");
    for attempt in 1..=3u32 {
        let prompt = format!("{GEN_SPEC}\n{repair}\nUSER INTENT: {intent}\n\nOutput the JSON now.");
        let raw = match call_claude(&prompt).await {
            Ok(o) => o,
            Err(e) => {
                last_err = e;
                break;
            }
        };
        let schema: serde_json::Value = match serde_json::from_str(&extract_json(&raw)) {
            Ok(v) => v,
            Err(e) => {
                last_err = format!("JSON 無效: {e}");
                repair = format!("你上次的輸出不是合法 JSON({e})。只輸出合法 JSON,不要 markdown 圍欄。");
                continue;
            }
        };
        let mut errs = Vec::new();
        let mut cells_out = Vec::new();
        match schema["cells"].as_array() {
            Some(cells) => {
                for c in cells {
                    let id = c["id"].as_str().unwrap_or("?");
                    let src = c["script"].as_str().unwrap_or("");
                    match try_compile_ui(src) {
                        Ok(n) => cells_out.push(json!({"id": id, "bytes": n, "script": src})),
                        Err(e) => errs.push(format!("cell '{id}': {e}")),
                    }
                }
            }
            None => errs.push("缺 cells 陣列".to_string()),
        }
        if errs.is_empty() {
            let fence: Vec<String> = UI_IMPORTS.iter().map(|h| format!("env.{}", h.name)).collect();
            return Json(json!({
                "ok": true, "attempt": attempt, "intent": intent,
                "schema": schema, "cells": cells_out, "fence": fence,
            }))
            .into_response();
        }
        last_err = errs.join("; ");
        repair = format!(
            "你上次的 JSON 有 cell 在受圍籬 DSL 裡編不過:\n{}\n只修正這些 cell 的 DSL(記得:浮點要寫 2.0、只有 // 註解、沒有 else 改用多個 guard if、只能用 get/set/sin/cos/ld/sd)。輸出修正後的完整 JSON。",
            errs.join("\n")
        );
    }
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"ok": false, "error": last_err})),
    )
        .into_response()
}

/// GET /api/wasm/{id} — variant 2's source: hand the browser the precompiled cell bytes.
/// This is the "ship the .wasm to the client" deployment. The bytes run in the browser and
/// carry no DSL, but they are downloadable and (see examples/reveal.rs) disassemble.
async fn api_wasm(State(st): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    match st.cell(&id) {
        Some(c) => (
            [
                (header::CONTENT_TYPE, "application/wasm"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            c.bytes.clone(),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "no such cell").into_response(),
    }
}

/// GET /api/source — prove the point in one call: this is every byte the client can pull,
/// and grepping it for the amortization formula finds nothing.
async fn api_source() -> impl IntoResponse {
    let page = include_str!("../index.html");
    let has_formula = page.contains("pw * (1.0 + r)") || page.contains("P * r * pw");
    (
        [(header::CONTENT_TYPE, "application/json")],
        Json(json!({
            "note": "this is the entire client. the loan formula (monthly amortization) is absent — it lives only in the server binary.",
            "client_bytes": page.len(),
            "contains_amortization_formula": has_formula,
        })),
    )
}

#[tokio::main]
async fn main() {
    let engine = Engine::default();

    // one source of truth for the DSL — the same schema apps/form.html embeds. Here we read
    // the compute cells out of it and compile them; the browser will never receive them.
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../apps/assets/loan_schema.json"))
            .expect("loan_schema.json parses");
    let script = |id: &str| -> String {
        schema["cells"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap_or_else(|| panic!("no cell '{id}' in schema"))
            ["script"]
            .as_str()
            .unwrap()
            .to_string()
    };

    let monthly = compile_cell(&engine, "monthly", &script("monthly"));
    let total = compile_cell(&engine, "totalPaid", &script("totalPaid"));
    let interest = compile_cell(&engine, "interest", &script("interest"));
    let fence: Vec<String> = UI_IMPORTS.iter().map(|h| format!("env.{}", h.name)).collect();

    println!("loan-server — server-side compute, same fence as the browser");
    println!("  fence offered to every cell: {}", fence.join(", "));
    for c in [&monthly, &total, &interest] {
        println!("  cell {:>10}: {:>3} bytes, reaches {{{}}}", c.id, c.bytes_len, c.imports.join(", "));
    }
    println!("  the DSL for these cells is NOT served to the client (GET /api/source proves it)");

    let state = Arc::new(AppState { engine, monthly, total, interest, fence });

    let app = Router::new()
        .route("/", get(index))
        .route("/compare", get(compare))
        .route("/reservoirs", get(reservoirs))
        .route("/reservoirs-native", get(reservoirs_native))
        .route("/reservoirs-net", get(reservoirs_net))
        .route("/reservoirs-solid", get(reservoirs_solid))
        .route("/genapp", get(genapp))
        .route("/api/gen", post(api_gen))
        .route("/api/loan", post(api_loan))
        .route("/api/wasm/{id}", get(api_wasm))
        .route("/api/source", get(api_source))
        // variant 1 needs the in-browser compiler; serve the wasm-bindgen bundle from ./pkg
        // (run this binary from the repo root so the relative path resolves).
        .nest_service("/pkg", ServeDir::new("pkg"))
        .with_state(state);

    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8787);
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("  listening on http://{addr}");
    println!("    /            server-side compute (variant 3: formula hidden)");
    println!("    /compare     all three deployments side by side");
    println!("    /api/wasm/monthly   the precompiled .wasm bytes (variant 2: ship the binary)");
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    // the loan cells stay inside the fence — verified by re-reading their own import tables.
    #[test]
    fn loan_cells_stay_within_fence() {
        let engine = Engine::default();
        let fence: Vec<&str> = UI_IMPORTS.iter().map(|h| h.name).collect();
        for (id, src) in [
            ("monthly", "let P = get(0.0);\nlet r = get(1.0) / 1200.0;\nlet n = get(2.0) * 12.0;\nlet M = 0.0;\nif n >= 1.0 {\n if r < 0.0000001 { M = P / n; }\n if r >= 0.0000001 {\n  let pw = 1.0;\n  let i = 0.0;\n  while i < n { pw = pw * (1.0 + r); i = i + 1.0; }\n  M = P * r * pw / (pw - 1.0);\n }\n}\nset(3.0, M);\nM"),
            ("total", "get(3.0) * get(2.0) * 12.0"),
            ("interest", "get(3.0) * get(2.0) * 12.0 - get(0.0)"),
        ] {
            let cell = compile_cell(&engine, id, src);
            for imp in &cell.imports {
                let name = imp.strip_prefix("env.").unwrap();
                assert!(fence.contains(&name), "'{id}' reached outside fence via {imp}");
            }
        }
    }

    // a cell that reaches for an entity capability (mv — move) the UI fence does not grant
    // cannot compile. the fence is a compile error, not a runtime check.
    #[test]
    fn over_reaching_cell_is_rejected_at_compile() {
        let prog = parser::parse("mv(1.0, 2.0);\n0.0").expect("parses as a call");
        let out = codegen::compile_with_opts(
            &prog,
            &UI_PARAMS,
            &UI_IMPORTS,
            codegen::CompileOpts { fuel: Some(200_000), memory_pages: None },
        );
        assert!(out.is_err(), "a UI cell must NOT be able to compile a call to mv()");
    }

    // end to end: the fenced cells compute correct amortization. 300k / 5% / 30y -> 1610.46.
    #[test]
    fn amortization_is_correct() {
        let engine = Engine::default();
        let monthly = compile_cell(&engine, "monthly", "let P = get(0.0);\nlet r = get(1.0) / 1200.0;\nlet n = get(2.0) * 12.0;\nlet M = 0.0;\nif n >= 1.0 {\n if r < 0.0000001 { M = P / n; }\n if r >= 0.0000001 {\n  let pw = 1.0;\n  let i = 0.0;\n  while i < n { pw = pw * (1.0 + r); i = i + 1.0; }\n  M = P * r * pw / (pw - 1.0);\n }\n}\nset(3.0, M);\nM");
        let linker = fenced_linker(&engine);
        let mut store = Store::new(&engine, Host::new());
        store.data_mut().slots[0] = 300_000.0;
        store.data_mut().slots[1] = 5.0;
        store.data_mut().slots[2] = 30.0;
        let m = run_cell(&mut store, &linker, &monthly).unwrap();
        assert!((m - 1610.46).abs() < 0.01, "monthly was {m}, expected 1610.46");
    }
}
