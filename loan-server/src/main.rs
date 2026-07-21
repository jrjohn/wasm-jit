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
/// The local-data workbench: the file never leaves the browser; only its schema is sent.
async fn analyst() -> impl IntoResponse {
    Html(include_str!("../../apps/analyst.html"))
}

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
 {"type":"grid","cols":2,"children":[...]}                     // 2-D layout: children flow across N columns (rows cannot align columns; this is a real primitive)
 {"type":"repeat","count":5,"template":{...}}                   // repeat the template N times; "$i" inside it becomes the 0-based index. template may be an ARRAY of nodes (e.g. to fill 2 grid columns per row)
 {"type":"value","bind":"cellId","arg":"$i"}                    // value with arg: runs the bound cell with x=arg — this is how a table row reads its own datum, e.g. cell {"id":"at","script":"ld(x)"}
 {"type":"pie","slices":[{"label":"美國","bind":"usa"},{"label":"中國","bind":"china"}]}   // pie chart; each slice's ANGLE = its bound cell's return value (percentages auto-computed & drawn inside each slice)
 {"type":"bar","slices":[{"label":"...","bind":"cellId"}]}                                    // bar chart; each bar's HEIGHT = its bound cell's return value

PATTERN: an input writes its value into a slot via `set(n, x)` then returns x; each compute cell reads slots via get(n); a value node binds to a compute cell; wire every input to every compute cell that depends on it.
CHART PATTERN: for a pie/bar of fixed data, make ONE cell per slice returning that slice's raw number (e.g. {"id":"usa","script":"35.0"}), and bind each slice to its cell. Do NOT precompute percentages or bake labels like "35%" into text — the pie draws the shares and % itself. Use a real "pie"/"bar" node, never fake a chart with rows of labels.

EXAMPLE (loan calculator):
{"title":"房貸試算","cells":[{"id":"setP","script":"set(0.0, x);\nx"},{"id":"setR","script":"set(1.0, x);\nx"},{"id":"setY","script":"set(2.0, x);\nx"},{"id":"monthly","script":"let P = get(0.0);\nlet r = get(1.0) / 1200.0;\nlet n = get(2.0) * 12.0;\nlet M = 0.0;\nif n >= 1.0 {\n if r < 0.0000001 { M = P / n; }\n if r >= 0.0000001 {\n  let pw = 1.0;\n  let i = 0.0;\n  while i < n { pw = pw * (1.0 + r); i = i + 1.0; }\n  M = P * r * pw / (pw - 1.0);\n }\n}\nM"}],"wires":[{"from":"setP","to":"monthly"},{"from":"setR","to":"monthly"},{"from":"setY","to":"monthly"}],"init":[{"cell":"setP","arg":300000},{"cell":"setR","arg":5},{"cell":"setY","arg":30}],"tree":{"type":"stack","children":[{"type":"label","text":"房貸試算"},{"type":"row","children":[{"type":"label","text":"本金"},{"type":"input","placeholder":"300000","on_input":{"cell":"setP"}}]},{"type":"row","children":[{"type":"label","text":"利率%"},{"type":"input","placeholder":"5","on_input":{"cell":"setR"}}]},{"type":"row","children":[{"type":"label","text":"年數"},{"type":"input","placeholder":"30","on_input":{"cell":"setY"}}]},{"type":"row","children":[{"type":"label","text":"月付"},{"type":"value","bind":"monthly"}]}]}}
"#;


// ── 真實資料:白名單 + host 代抓 ────────────────────────────────────────────────
// 這是第三層(擴 reach)唯一被允許的形狀:**cell 永遠沒有 fetch**。host 只從這張
// 白名單抓,正規化成 {labels, values},再餵進 collection store,cell 只 ld() 讀。
// 於是 App 有了真資料,而 cell 的觸及範圍一格都沒有變大。
const DATA_SOURCES: [(&str, &str, &str); 1] = [(
    "reservoirs",
    "台灣水庫即時水情(21 座):percentage=蓄水率%、volume=蓄水量(萬立方公尺,水 1m³≈1 公噸,故此即『萬噸』)、inflow=日進流量",
    "https://water.taiwanstat.com/data/data.json",
)];

static DATA_CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, (std::time::Instant, serde_json::Value)>>> =
    std::sync::OnceLock::new();

async fn api_data_list() -> impl IntoResponse {
    Json(
        DATA_SOURCES.iter()
            .map(|(id, desc, url)| json!({"id": id, "desc": desc, "url": url}))
            .collect::<Vec<_>>(),
    )
}

/// GET /api/data/{id} — host fetches an allow-listed source and returns it normalised.
async fn api_data(Path(id): Path<String>) -> Response {
    let Some((_, _desc, url)) = DATA_SOURCES.iter().find(|(i, _, _)| *i == id) else {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "此來源不在白名單"}))).into_response();
    };
    let cache = DATA_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Ok(c) = cache.lock() {
        if let Some((t, v)) = c.get(&id) {
            if t.elapsed() < std::time::Duration::from_secs(300) {
                return Json(v.clone()).into_response();
            }
        }
    }
    let raw: serde_json::Value = match reqwest::Client::new()
        .get(*url)
        .header("User-Agent", "wasm-jit-genapp/0.1")
        .timeout(std::time::Duration::from_secs(20))
        .send().await
    {
        Ok(r) => match r.json().await { Ok(j) => j, Err(e) => return (StatusCode::BAD_GATEWAY, Json(json!({"error": format!("來源回應不是 JSON: {e}")}))).into_response() },
        Err(e) => return (StatusCode::BAD_GATEWAY, Json(json!({"error": format!("抓取失敗: {e}")}))).into_response(),
    };
    let out = normalise_reservoirs(&raw, url);
    if let Ok(mut c) = cache.lock() { c.insert(id.clone(), (std::time::Instant::now(), out.clone())); }
    Json(out).into_response()
}

/// upstream is {"石門水庫": {name, percentage, volumn, baseAvailable, daliyInflow, updateAt}, ...}
fn normalise_reservoirs(raw: &serde_json::Value, url: &str) -> serde_json::Value {
    let num = |v: &serde_json::Value| -> f64 {
        v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse::<f64>().ok())).unwrap_or(0.0)
    };
    let (mut labels, mut pct, mut vol, mut inflow) = (vec![], vec![], vec![], vec![]);
    let mut updated = String::new();
    if let Some(map) = raw.as_object() {
        for (k, v) in map {
            labels.push(v["name"].as_str().unwrap_or(k).to_string());
            pct.push(num(&v["percentage"]));
            vol.push(num(&v["volumn"]));
            inflow.push(num(&v["daliyInflow"]));
            if updated.is_empty() { updated = v["updateAt"].as_str().unwrap_or("").to_string(); }
        }
    }
    json!({
        "id": "reservoirs", "title": "台灣水庫即時水情",
        "source": url, "updated": updated, "count": labels.len(),
        "fields": ["percentage", "volume", "inflow"],
        "labels": labels,
        "values": { "percentage": pct, "volume": vol, "inflow": inflow }
    })
}

#[derive(Deserialize)]
struct GenReq {
    intent: String,
    /// prior turns of THIS chat, oldest first — so a follow-up like 「要全部的」has a referent
    #[serde(default)]
    history: Vec<serde_json::Value>,
    /// the app currently on screen — a follow-up usually means "change THIS", not "start over"
    #[serde(default)]
    prev_schema: Option<serde_json::Value>,
    /// SCHEMA ONLY of the user's local dataset — column names/types/counts. Never any rows:
    /// the data stays in their browser. The model writes the program; it never sees the data.
    #[serde(default)]
    data_schema: Option<serde_json::Value>,
}

/// The word library — composites the AI has defined, persisted so everyone gets them next time.
/// A word is made of fenced parts, so a word is fenced too (§21 attenuation: child ⊆ parent);
/// growing the vocabulary therefore never widens reach.
const WORDS_FILE: &str = "apps/assets/words.json";

fn load_words() -> Vec<serde_json::Value> {
    std::fs::read_to_string(WORDS_FILE)
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(&s).ok())
        .unwrap_or_default()
}

fn save_words(words: &[serde_json::Value]) -> Result<(), String> {
    let s = serde_json::to_string_pretty(words).map_err(|e| e.to_string())?;
    std::fs::write(WORDS_FILE, s).map_err(|e| e.to_string())
}

/// Substitute a word's $param defaults so its cells can be compile-checked like any other.
fn bake_defaults(script: &str, params: &[serde_json::Value]) -> String {
    let mut s = script.to_string();
    for p in params {
        if let (Some(n), Some(d)) = (p["name"].as_str(), p.get("default")) {
            let val = d.as_str().map(|x| x.to_string()).unwrap_or_else(|| d.to_string());
            s = s.replace(&format!("${n}"), &val);
        }
    }
    s
}

/// Render the library into the prompt so the model composes with proven words instead of
/// re-deriving them — this is what makes "define once, everyone reuses" real.
fn words_section(words: &[serde_json::Value]) -> String {
    if words.is_empty() {
        return String::new();
    }
    let mut out = String::from("\nAVAILABLE WORDS (詞庫 — already-proven reusable composites; PREFER these over rebuilding from scratch):\n");
    for w in words {
        let name = w["name"].as_str().unwrap_or("?");
        let desc = w["desc"].as_str().unwrap_or("");
        let ps: Vec<String> = w["params"]
            .as_array()
            .map(|a| a.iter().filter_map(|p| p["name"].as_str().map(String::from)).collect())
            .unwrap_or_default();
        out.push_str(&format!("- {name}({}) — {desc}\n", ps.join(", ")));
        let cells: Vec<String> = w["cells"]
            .as_array()
            .map(|a| a.iter().filter_map(|c| c["id"].as_str().map(String::from)).collect())
            .unwrap_or_default();
        if !cells.is_empty() {
            out.push_str(&format!("    its cells become \"<id>.{}\" — wire from those\n", cells.join("\", \"<id>.")));
        }
    }
    out.push_str(
        r#"USE a word:  {"type":"word","name":"input_row","id":"p","args":{"label":"本金","slot":"0.0"}}
   ("id" prefixes that instance's cells, so the same word can be used many times; wire from "p.set".)
DEFINE a new word by adding a top-level "define_words": [{"name":"...","desc":"...","params":[{"name":"x","default":"0"}],"cells":[{"id":"c","script":"...$x..."}],"tree":{...}}].
   Define one whenever the pattern is generally reusable (a donut, a gauge, a stat card, a sortable table).
   Words are saved and offered to EVERY future generation — define once, everyone reuses.
"#,
    );
    out
}

/// Tell the model which REAL data sources the host can fetch on its behalf.
/// The cell still has no network — it only ever reads numbers the host already placed.
/// Render the conversation so far + the app on screen, so follow-ups refine instead of restart.
fn context_section(history: &[serde_json::Value], prev: &Option<serde_json::Value>) -> String {
    if history.is_empty() && prev.is_none() { return String::new(); }
    let mut out = String::from("\nCONVERSATION SO FAR (oldest first). The new message is very likely a FOLLOW-UP to these:\n");
    for (i, h) in history.iter().enumerate() {
        let intent = h["intent"].as_str().unwrap_or("");
        let title = h["title"].as_str().unwrap_or("");
        out.push_str(&format!("  {}. 使用者:「{intent}」 → 你做了:「{title}」\n", i + 1));
    }
    if let Some(p) = prev {
        out.push_str(&format!(
            "\nCURRENT APP ON SCREEN (its full schema):\n{}\n",
            serde_json::to_string(p).unwrap_or_default()
        ));
        out.push_str(
"RULE FOR FOLLOW-UPS: if the new message refines, corrects, or asks about the CURRENT app
 (e.g.「要全部的」「改成圓餅」「你是用噸排名嗎?」「加上百分比」), you MUST return an updated FULL
 schema of THAT app — keep its data source, title and structure, change only what was asked.
 Only start a brand-new unrelated app if the user clearly asks for a different thing.
 If the message is a QUESTION about the current app, answer it by returning the app corrected
 to what the user evidently wants (e.g. asked「你是用噸排名嗎?」about a percentage chart →
 return the same chart switched to the tonnage field), and say so in the title.
");
    }
    out
}

/// Describe the user's LOCAL dataset to the model — schema only — plus the exact contract
/// for reading it from inside a cell. The rows never left the user's machine.
fn dataset_section(ds: &Option<serde_json::Value>) -> String {
    let Some(ds) = ds else { return String::new() };
    format!(r#"
THE USER'S LOCAL DATASET — you can see ONLY this schema. The rows NEVER leave their machine and you
will never see them. Write a program over it; do not invent data, do not ask for the data.
{}

HOW A CELL READS IT:
- rows = get(30.0)   cols = get(31.0)
- value at row r (0-based), column c (0-based):  ld(r * get(31.0) + c)
- a TEXT column stores a CATEGORY INDEX (0 .. distinct-1), never the text itself. You never learn the
  actual names — the host renders them as labels.
- there is NO == in the DSL. Test equality with two guards:   if v >= k {{ if v <= k {{ ... }} }}
- loop rows:   let r = 0.0; while r < n {{ ... r = r + 1.0; }}
- keep loops O(rows); fuel is 200000 ops.

GROUPED CHART (use this instead of inventing labels):
  {{"type":"bar","groups":{{"col":<text column index>,"bind":"cellId","sort":"desc"}}}}
  "sort" is "desc" | "asc" | "none" — YOU cannot rank the groups yourself (you never see the values),
  so when the user asks for a ranking/排名/由高到低, declare "sort":"desc" and the HOST sorts them.
  The host draws one bar per category using the REAL local label, and calls your cell with
  x = the category index. So write ONE cell that takes x (a category index) and returns that
  group's number (mean/sum/count). Same for "pie".

DECLARATIVE STATS — PREFER THIS, it is the easy path and you should use it for anything routine.
Do NOT hand-write a row loop for a plain mean/sum/count/max/min. Just declare it and the host
synthesises + compiles the cell for you (same fence):
  grouped:      {{"type":"bar","groups":{{"col":3,"agg":"mean","of":5,"sort":"desc"}}}}
  whole column: {{"type":"value","label":"全球平均","agg":"mean","of":5}}
  "agg" is "mean" | "sum" | "count" | "max" | "min"; "of" = the numeric column index;
  optional "where": {{"col":0,"min":1990,"max":2025}} filters rows by a numeric column (e.g. a period);
  "col" = the column to group by. No cell of your own is needed at all in these cases.
ONLY write your own cell (with "cells" + "bind") when the statistic is genuinely custom
(ratios, thresholds, two-period comparisons, weighted things) — then the loop contract above applies.

WHOLE-COLUMN STATS (custom): a plain cell + {{"type":"value","label":"...","bind":"cellId"}}.
"#, serde_json::to_string_pretty(ds).unwrap_or_default())
}

fn data_section() -> String {
    let mut out = String::from("\nREAL DATA SOURCES (host-fetched, allow-listed — the cell itself still has NO network):\n");
    for (id, desc, _url) in DATA_SOURCES.iter() {
        out.push_str(&format!("- \"{id}\" — {desc}\n"));
    }
    out.push_str(r#"USE real data in a chart:  {"type":"bar","from":"reservoirs","field":"percentage","top":10}
   ("from" = source id, "field" = which series, "top" = keep the N largest, sorted desc by default.)
   The host also loads that source's numbers into the collection store, so cells can read them with ld(i),
   and the row count is in slot 31 (get(31.0)) — e.g. a table via {"type":"repeat","count":N,...} + {"id":"at","script":"ld(x)"}.
WHEN the user asks about real-world facts covered by a source above, USE the source — do NOT invent numbers.
If no source covers it, invent small sample numbers AND make the title say 示意.
"#);
    out
}

/// The app the demo page arrives with, already built — visitors extend it by talking.
async fn api_seed() -> impl IntoResponse {
    match std::fs::read_to_string("apps/assets/seed_app.json") {
        Ok(t) => (
            [(header::CONTENT_TYPE, "application/json")],
            t,
        ).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn api_words() -> impl IntoResponse {
    Json(load_words())
}

// ─────────────────────────────────────────────────────────────────────────────
// ① An app that outlives the tab.  Until now every generated app died with the
//    localStorage that held it: you could grow one by talking, but you could not
//    keep it or hand it to anyone.  A creativity engine with no exit is an engine
//    only its author ever starts.
//
//    What makes saving safe is the same thing that makes generating safe: every
//    cell is compile-checked against the fence BEFORE it is written to disk, so a
//    saved app is a provably fenced app.  Nothing is executed to save it.
// ─────────────────────────────────────────────────────────────────────────────

const SAVED_DIR: &str = "apps/saved";

/// Short, URL-safe, non-sequential id. Derived from the clock plus a content hash so
/// two saves in the same nanosecond still differ, and ids are not guessable by counting.
fn mint_id(content: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in content.bytes().chain(nanos.to_le_bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    let alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut id = String::new();
    for _ in 0..8 {
        id.push(alphabet[(h % 36) as usize] as char);
        h /= 36;
    }
    id
}

/// Ids come back from URLs, so they are hostile input until proven otherwise —
/// this is what keeps `{id}` from walking out of SAVED_DIR.
fn valid_id(id: &str) -> bool {
    id.len() >= 4 && id.len() <= 16 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

#[derive(Deserialize)]
struct SaveReq {
    schema: serde_json::Value,
    #[serde(default)]
    title: String,
}

async fn api_save(Json(req): Json<SaveReq>) -> impl IntoResponse {
    // Re-verify the fence at the door. The client cannot save what it could not compile,
    // and we do not take its word for it — an app arriving here from anywhere is checked.
    let mut errs: Vec<String> = Vec::new();
    match req.schema["cells"].as_array() {
        Some(cells) => {
            for c in cells {
                let id = c["id"].as_str().unwrap_or("?");
                if let Err(e) = try_compile_ui(c["script"].as_str().unwrap_or("")) {
                    errs.push(format!("cell '{id}': {e}"));
                }
            }
        }
        None => errs.push("缺 cells 陣列".into()),
    }
    if !errs.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"ok": false, "error": format!("圍籬檢查未過,未存檔:{}", errs.join("; "))})),
        )
            .into_response();
    }

    let title = {
        let t = req.title.trim();
        let t = if t.is_empty() { req.schema["title"].as_str().unwrap_or("未命名") } else { t };
        t.chars().take(80).collect::<String>()
    };
    let body = req.schema.to_string();
    let id = mint_id(&body);
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if let Err(e) = std::fs::create_dir_all(SAVED_DIR) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"ok": false, "error": e.to_string()}))).into_response();
    }
    let rec = json!({"id": id, "title": title, "created": created, "schema": req.schema});
    if let Err(e) = std::fs::write(format!("{SAVED_DIR}/{id}.json"), rec.to_string()) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"ok": false, "error": e.to_string()}))).into_response();
    }
    Json(json!({"ok": true, "id": id, "url": format!("/a/{id}"), "title": title})).into_response()
}

async fn api_app(Path(id): Path<String>) -> impl IntoResponse {
    if !valid_id(&id) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "id 格式不合"}))).into_response();
    }
    match std::fs::read_to_string(format!("{SAVED_DIR}/{id}.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    {
        Some(v) => Json(v).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({"error": "找不到這個作品"}))).into_response(),
    }
}

/// Everything anyone kept — the point of an exit is that other people can walk in.
async fn api_gallery() -> impl IntoResponse {
    let mut items: Vec<serde_json::Value> = std::fs::read_dir(SAVED_DIR)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let v: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok()?;
            Some(json!({
                "id": v["id"], "title": v["title"], "created": v["created"],
                "nodes": v["schema"]["tree"]["children"].as_array().map(|a| a.len()).unwrap_or(0),
            }))
        })
        .collect();
    items.sort_by_key(|v| std::cmp::Reverse(v["created"].as_u64().unwrap_or(0)));
    items.truncate(48);
    Json(items)
}

// ─────────────────────────────────────────────────────────────────────────────
// ② A word anyone may contribute — and NOBODY has to review.
//
//    This is the one thing this architecture can do that a code-generating rival
//    structurally cannot.  Their community components are arbitrary code, so every
//    contribution needs a human to read it before it can be offered to anyone else.
//    A word here is built only from fenced parts, so by §21 attenuation the word is
//    fenced too: a contributor cannot smuggle in reach that its parts never had.
//
//    So the compile gate IS the review.  The vocabulary can grow at the speed of a
//    crowd while the reach stays exactly where it was.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct WordReq {
    word: serde_json::Value,
    #[serde(default)]
    by: String,
}

async fn api_word_add(Json(req): Json<WordReq>) -> impl IntoResponse {
    let d = req.word;
    let name = d["name"].as_str().unwrap_or("").trim().to_lowercase();
    let name_ok = (2..=32).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if !name_ok {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false, "error": "詞名只能用小寫英數與底線,2–32 字"}))).into_response();
    }
    if !d["tree"].is_object() {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false, "error": "缺 tree(這個詞畫出來長什麼樣)"}))).into_response();
    }

    let mut lib = load_words();
    if lib.iter().any(|w| w["name"].as_str() == Some(name.as_str())) {
        return (StatusCode::CONFLICT, Json(json!({"ok": false, "error": format!("詞庫裡已經有 '{name}' 了")}))).into_response();
    }

    // The gate. Every cell must compile inside the fence with its defaults baked in —
    // exactly the check a model-defined word goes through, applied to a stranger's word.
    let params = d["params"].as_array().cloned().unwrap_or_default();
    let mut errs: Vec<String> = Vec::new();
    if let Some(cs) = d["cells"].as_array() {
        for c in cs {
            let cid = c["id"].as_str().unwrap_or("?");
            let baked = bake_defaults(c["script"].as_str().unwrap_or(""), &params);
            if let Err(e) = try_compile_ui(&baked) {
                errs.push(format!("cell '{cid}': {e}"));
            }
        }
    }
    if !errs.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"ok": false, "error": format!("這個詞沒過圍籬,沒有收進詞庫:{}", errs.join("; ")),
                        "fence": UI_IMPORTS.iter().map(|h| h.name).collect::<Vec<_>>()})),
        )
            .into_response();
    }

    let mut w = d.clone();
    w["name"] = json!(name);
    w["by"] = json!(req.by.trim().chars().take(40).collect::<String>());
    w["added"] = json!(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|x| x.as_secs())
        .unwrap_or(0));
    lib.push(w);
    if let Err(e) = save_words(&lib) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"ok": false, "error": e}))).into_response();
    }
    Json(json!({"ok": true, "name": name, "count": lib.len(), "words": lib})).into_response()
}

async fn call_claude(prompt: &str) -> Result<String, String> {
    let bin = std::env::var("CLAUDE_BIN").unwrap_or_else(|_| "/opt/homebrew/bin/claude".into());
    let fut = tokio::process::Command::new(&bin)
        .arg("-p")
        .arg("--model")
        .arg("claude-sonnet-5")
        .arg(prompt)
        .output();
    let out = tokio::time::timeout(std::time::Duration::from_secs(75), fut)
        .await
        .map_err(|_| "claude 無回應(>75s,已自動重試)".to_string())?
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
    let wsec = words_section(&load_words());
    let ctx = context_section(&req.history, &req.prev_schema);
    let mut repair = String::new();
    let mut last_err = String::from("unknown");
    for attempt in 1..=3u32 {
        let prompt = format!("{GEN_SPEC}\n{}\n{}\n{wsec}\n{}\n{repair}\nUSER'S NEW MESSAGE: {intent}\n\nOutput the JSON now.", data_section(), dataset_section(&req.data_schema), ctx);
        let raw = match call_claude(&prompt).await {
            Ok(o) => o,
            Err(e) => {
                // a hung/slow claude is usually transient — retry rather than give up
                last_err = e;
                continue;
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
            // the model may DEFINE new reusable words — compile-check them, then persist for everyone
            let mut lib = load_words();
            let mut added: Vec<String> = Vec::new();
            if let Some(defs) = schema["define_words"].as_array() {
                for d in defs {
                    let name = d["name"].as_str().unwrap_or("").trim().to_string();
                    if name.is_empty() || !d["tree"].is_object() { continue; }
                    if lib.iter().any(|w| w["name"].as_str() == Some(name.as_str())) { continue; }
                    let params = d["params"].as_array().cloned().unwrap_or_default();
                    let ok = d["cells"].as_array().map_or(true, |cs| {
                        cs.iter().all(|c| {
                            try_compile_ui(&bake_defaults(c["script"].as_str().unwrap_or(""), &params)).is_ok()
                        })
                    });
                    if ok { lib.push(d.clone()); added.push(name); }
                }
                if !added.is_empty() { let _ = save_words(&lib); }
            }
            let fence: Vec<String> = UI_IMPORTS.iter().map(|h| format!("env.{}", h.name)).collect();
            return Json(json!({
                "ok": true, "attempt": attempt, "intent": intent,
                "schema": schema, "cells": cells_out, "fence": fence,
                "words": lib, "new_words": added,
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
        .route("/analyst", get(analyst))
        // ① a saved app has a URL of its own — the same page, told which app to load
        .route("/a/{id}", get(genapp))
        .route("/gallery", get(genapp))
        .route("/api/save", post(api_save))
        .route("/api/app/{id}", get(api_app))
        .route("/api/gallery", get(api_gallery))
        .route("/api/gen", post(api_gen))
        .route("/api/words", get(api_words))
        // ② anyone may add a word; the compile gate is the only reviewer there is
        .route("/api/words", post(api_word_add))
        .route("/api/seed", get(api_seed))
        .route("/api/data", get(api_data_list))
        .route("/api/data/{id}", get(api_data))
        .route("/api/loan", post(api_loan))
        .route("/api/wasm/{id}", get(api_wasm))
        .route("/api/source", get(api_source))
        // variant 1 needs the in-browser compiler; serve the wasm-bindgen bundle from ./pkg
        // (run this binary from the repo root so the relative path resolves).
        .nest_service("/pkg", ServeDir::new("pkg"))
        .nest_service("/assets", ServeDir::new("apps/assets"))
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
