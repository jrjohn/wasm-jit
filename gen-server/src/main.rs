//! gen-server — the live-generation loop, made real (docs §9's "slow loop"):
//!
//!   browser chat → POST /api/generate → Claude CLI inside a Docker container
//!   (the GENERATOR is sandboxed: no volumes, only API egress) → the returned
//!   seed is validated by NATIVELY COMPILING it with wasm-jit (self-repair
//!   retry on failure) → only a seed that compiles reaches the browser, where
//!   the same compiler manifests it into fuel-metered, capability-sandboxed
//!   WASM cells (the GENERATED is sandboxed too).
//!
//! Generation is seconds-slow; manifestation is µs-fast. That asymmetry is
//! the demo. Runs on :8646, fully separate from the other demos.

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{routing::get, routing::post, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tower_http::services::{ServeDir, ServeFile};
use wasm_jit::codegen::{self, CompileOpts, HostFn};
use wasm_jit::parser;

const CONTRACT: &str = include_str!("../contract.md");
const DEFAULT_MODEL: &str = "claude-sonnet-5";
const ALLOWED_MODELS: [&str; 3] = ["claude-haiku-4-5-20251001", "claude-sonnet-5", "claude-opus-4-8"];
const MAX_ATTEMPTS: u32 = 3;
const GEN_TIMEOUT: Duration = Duration::from_secs(150);

const UI_IMPORTS: [HostFn; 4] = [
    HostFn { name: "sin", n_args: 1, returns: true },
    HostFn { name: "cos", n_args: 1, returns: true },
    HostFn { name: "get", n_args: 1, returns: true },
    HostFn { name: "set", n_args: 2, returns: false },
];
const DRAW_IMPORTS: [HostFn; 7] = [
    HostFn { name: "sin", n_args: 1, returns: true },
    HostFn { name: "cos", n_args: 1, returns: true },
    HostFn { name: "hue", n_args: 1, returns: false },
    HostFn { name: "disc", n_args: 3, returns: false },
    HostFn { name: "ring", n_args: 3, returns: false },
    HostFn { name: "arc", n_args: 5, returns: false },
    HostFn { name: "line", n_args: 4, returns: false },
];
const UI_VOCAB: [&str; 11] = [
    "stack", "row", "label", "value", "button", "slider", "input",
    "barchart", "linechart", "piechart", "gauge",
];

/// World-cell ABI (the Field, docs §19): run(t, gw, gh) -> f64.
/// fr/fw are the collective-karma field capabilities (reads are global —
/// mutual beholding; writes are region-scoped in the host closure).
const FIELD_PARAMS: [&str; 3] = ["t", "gw", "gh"];
const FIELD_IMPORTS: [HostFn; 6] = [
    HostFn { name: "sin", n_args: 1, returns: true },
    HostFn { name: "cos", n_args: 1, returns: true },
    HostFn { name: "get", n_args: 1, returns: true },
    HostFn { name: "set", n_args: 2, returns: false },
    HostFn { name: "fr", n_args: 3, returns: true },
    HostFn { name: "fw", n_args: 4, returns: false },
];
const FIELD_FUEL: u32 = 2_000_000;

/// Inhabitant (entity) ABI: run(t, ex, ey) -> f64, once per tick. The soul is
/// a seed (JSON+DSL, fast loop); the skin is host sprite vocabulary (slow
/// loop); the bounds are this grant template. `mv(dx,dy)` REQUESTS movement —
/// position is host-owned state, clamped and bounded by the host.
const ENTITY_PARAMS: [&str; 3] = ["t", "ex", "ey"];
const ENTITY_IMPORTS: [HostFn; 7] = [
    HostFn { name: "sin", n_args: 1, returns: true },
    HostFn { name: "cos", n_args: 1, returns: true },
    HostFn { name: "get", n_args: 1, returns: true },
    HostFn { name: "set", n_args: 2, returns: false },
    HostFn { name: "fr", n_args: 3, returns: true },
    HostFn { name: "mv", n_args: 2, returns: false },
    HostFn { name: "unbind", n_args: 0, returns: false }, // §19: the freedom to leave a condition
];
const ENTITY_FUEL: u32 = 200_000;
/// The curated skin registry — types the host draws with hand-tuned Rust skins.
const ENTITY_TYPES: [&str; 4] = ["boat", "fisherman", "person", "car"];

/// Generated-skin ABI (docs §20.1): run(px, py, s, t) with drawing primitives
/// only — how a novel inhabitant *looks*, grown at runtime, fenced by the same
/// drawing audit as the draw surface.
const SKIN_PARAMS: [&str; 4] = ["px", "py", "s", "t"];
const SKIN_IMPORTS: [HostFn; 7] = [
    HostFn { name: "sin", n_args: 1, returns: true },
    HostFn { name: "cos", n_args: 1, returns: true },
    HostFn { name: "hue", n_args: 1, returns: false },
    HostFn { name: "disc", n_args: 3, returns: false },
    HostFn { name: "ring", n_args: 3, returns: false },
    HostFn { name: "arc", n_args: 5, returns: false },
    HostFn { name: "line", n_args: 4, returns: false },
];

#[derive(Deserialize)]
struct GenReq {
    prompt: String,
    #[serde(default)]
    prior: Option<Value>,
    #[serde(default)]
    model: Option<String>,
}

/// Run Claude CLI in the sandboxed container. The container gets exactly two
/// env vars and zero volumes — the generator's whole world is the prompt.
async fn claude_generate(prompt: &str, model: &str) -> Result<String, String> {
    let image = std::env::var("GEN_IMAGE").unwrap_or_else(|_| "agent-task-node:local".into());
    let mut cmd = tokio::process::Command::new("docker");
    cmd.args([
        "run", "--rm", "--entrypoint", "claude",
        "-e", "CLAUDE_CODE_OAUTH_TOKEN", "-e", "IS_SANDBOX=1",
        &image,
        "-p", prompt, "--model", model,
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let out = tokio::time::timeout(GEN_TIMEOUT, cmd.output())
        .await
        .map_err(|_| "generation timed out (150s)".to_string())?
        .map_err(|e| format!("docker spawn failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "claude exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).chars().take(400).collect::<String>()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Pull the JSON object out of the model's reply (tolerates stray prose/fences).
fn extract_json(raw: &str) -> Result<Value, String> {
    let start = raw.find('{').ok_or("no JSON object in the reply")?;
    let end = raw.rfind('}').ok_or("no closing brace in the reply")?;
    if end <= start {
        return Err("malformed JSON span".into());
    }
    serde_json::from_str(&raw[start..=end]).map_err(|e| format!("JSON parse: {e}"))
}

fn compile_check(src: &str, params: &[&str], imports: &[HostFn], fuel: u32) -> Result<(), String> {
    let prog = parser::parse(src)?;
    codegen::compile_with_opts(&prog, params, imports, CompileOpts { fuel: Some(fuel), memory_pages: None })
        .map(|_| ())
}

fn nums_of(node: &Value, key: &str) -> Option<usize> {
    node.get(key)
        .and_then(|v| v.as_array())
        .filter(|a| a.iter().all(|x| x.is_number()))
        .map(|a| a.len())
}

/// Chart nodes are DISPLAY vocabulary: they carry data (static `values` or
/// live `bind_values` referencing cells), never events.
fn validate_chart(t: &str, node: &Value, cell_ids: &[String]) -> Result<(), String> {
    let check_binds = |key: &str| -> Result<Option<usize>, String> {
        match node.get(key).and_then(|v| v.as_array()) {
            None => Ok(None),
            Some(a) => {
                for b in a {
                    let id = b.as_str().ok_or(format!("{key} entries must be cell ids"))?;
                    if !cell_ids.iter().any(|i| i == id) {
                        return Err(format!("{key} references unknown cell '{id}'"));
                    }
                }
                Ok(Some(a.len()))
            }
        }
    };
    match t {
        "barchart" | "piechart" => {
            let n = node
                .get("labels")
                .and_then(|l| l.as_array())
                .map(|a| a.len())
                .ok_or(format!("{t} lacks \"labels\" []"))?;
            if n == 0 {
                return Err(format!("{t} has zero labels"));
            }
            let vals = nums_of(node, "values");
            let binds = check_binds("bind_values")?;
            match (vals, binds) {
                (Some(v), _) if v == n => Ok(()),
                (_, Some(b)) if b == n => Ok(()),
                (Some(v), _) => Err(format!("{t}: {n} labels but {v} values")),
                (_, Some(b)) => Err(format!("{t}: {n} labels but {b} bind_values")),
                (None, None) => Err(format!("{t} needs \"values\" (numbers) or \"bind_values\" (cell ids)")),
            }
        }
        "linechart" => {
            let n = node
                .get("labels")
                .and_then(|l| l.as_array())
                .map(|a| a.len())
                .ok_or("linechart lacks \"labels\" []")?;
            let series = node
                .get("series")
                .and_then(|s| s.as_array())
                .ok_or("linechart lacks \"series\" []")?;
            if series.is_empty() {
                return Err("linechart has zero series".into());
            }
            for s in series {
                let len = nums_of(s, "values")
                    .ok_or("each series needs numeric \"values\"")?;
                if len != n {
                    return Err(format!("series length {len} ≠ {n} labels"));
                }
            }
            Ok(())
        }
        "gauge" => {
            let has_static = node.get("value").map(|v| v.is_number()).unwrap_or(false);
            let bind_ok = match node.get("bind").and_then(|b| b.as_str()) {
                Some(id) => {
                    if !cell_ids.iter().any(|i| i == id) {
                        return Err(format!("gauge bind references unknown cell '{id}'"));
                    }
                    true
                }
                None => false,
            };
            if has_static || bind_ok {
                Ok(())
            } else {
                Err("gauge needs a numeric \"value\" or a \"bind\" cell id".into())
            }
        }
        _ => Ok(()),
    }
}

fn validate_tree(node: &Value, cell_ids: &[String]) -> Result<(), String> {
    let t = node
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or("tree node lacks \"type\"")?;
    if !UI_VOCAB.contains(&t) {
        return Err(format!("node type '{t}' not in vocabulary [{}]", UI_VOCAB.join(", ")));
    }
    validate_chart(t, node, cell_ids)?;
    for ev in ["on_click", "on_input"] {
        if let Some(spec) = node.get(ev) {
            let cell = spec
                .get("cell")
                .and_then(|c| c.as_str())
                .ok_or_else(|| format!("{ev} lacks \"cell\""))?;
            if !cell_ids.iter().any(|i| i == cell) {
                return Err(format!("{ev} references unknown cell '{cell}'"));
            }
            if let Some(src) = spec.get("arg_from").and_then(|a| a.as_str()) {
                if !cell_ids.iter().any(|i| i == src) {
                    return Err(format!("arg_from references unknown cell '{src}'"));
                }
            }
        }
    }
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for c in children {
            validate_tree(c, cell_ids)?;
        }
    }
    Ok(())
}

fn validate_region(region: &Value, grid: u64, id: &str) -> Result<(), String> {
    let r: Vec<f64> = region
        .as_array()
        .filter(|a| a.len() == 4)
        .map(|a| a.iter().filter_map(|v| v.as_f64()).collect())
        .ok_or_else(|| format!("world cell '{id}': region must be [x0,y0,x1,y1]"))?;
    if r.len() != 4
        || r[0] < 0.0
        || r[1] < 0.0
        || r[0] >= r[2]
        || r[1] >= r[3]
        || r[2] > grid as f64
        || r[3] > grid as f64
    {
        return Err(format!(
            "world cell '{id}': region [{},{},{},{}] out of bounds for grid {grid}",
            r.first().copied().unwrap_or(-1.0),
            r.get(1).copied().unwrap_or(-1.0),
            r.get(2).copied().unwrap_or(-1.0),
            r.get(3).copied().unwrap_or(-1.0)
        ));
    }
    Ok(())
}

/// The server-side fence: a generated seed is COMPILED here, natively, before
/// the browser ever sees it. Same compiler, same grants, same fuel as the
/// browser side — the validation cannot drift from reality.
fn validate(obj: &Value) -> Result<(), String> {
    match obj.get("surface").and_then(|s| s.as_str()) {
        Some("ui") => {
            let schema = obj.get("schema").ok_or("surface \"ui\" lacks \"schema\"")?;
            let cells = schema
                .get("cells")
                .and_then(|c| c.as_array())
                .ok_or("schema lacks \"cells\" []")?;
            if cells.is_empty() {
                return Err("schema has zero cells".into());
            }
            let mut ids = Vec::new();
            for c in cells {
                let id = c
                    .get("id")
                    .and_then(|i| i.as_str())
                    .ok_or("a cell lacks \"id\"")?;
                let script = c
                    .get("script")
                    .and_then(|s| s.as_str())
                    .ok_or_else(|| format!("cell '{id}' lacks \"script\""))?;
                compile_check(script, &["x"], &UI_IMPORTS, 200_000)
                    .map_err(|e| format!("cell '{id}' failed to compile: {e}"))?;
                ids.push(id.to_string());
            }
            let tree = schema.get("tree").ok_or("schema lacks \"tree\"")?;
            validate_tree(tree, &ids)?;
            if let Some(init) = schema.get("init").and_then(|i| i.as_array()) {
                for entry in init {
                    let cell = entry
                        .get("cell")
                        .and_then(|c| c.as_str())
                        .ok_or("init entry lacks \"cell\"")?;
                    if !ids.iter().any(|i| i == cell) {
                        return Err(format!("init references unknown cell '{cell}'"));
                    }
                    if entry.get("arg").map(|a| !a.is_number()).unwrap_or(false) {
                        return Err("init \"arg\" must be a number".into());
                    }
                }
            }
            if let Some(wires) = schema.get("wires").and_then(|w| w.as_array()) {
                for w in wires {
                    for side in ["from", "to"] {
                        let id = w
                            .get(side)
                            .and_then(|s| s.as_str())
                            .ok_or("a wire lacks from/to")?;
                        if !ids.iter().any(|i| i == id) {
                            return Err(format!("wire references unknown cell '{id}'"));
                        }
                    }
                }
            }
            Ok(())
        }
        Some("draw") => {
            let seed = obj
                .get("seed")
                .and_then(|s| s.as_str())
                .ok_or("surface \"draw\" lacks \"seed\"")?;
            compile_check(seed, &["t", "w", "h"], &DRAW_IMPORTS, 5_000_000)
                .map_err(|e| format!("draw seed failed to compile: {e}"))
        }
        Some("field") => {
            let world = obj.get("world").ok_or("surface \"field\" lacks \"world\"")?;
            let grid = world.get("grid").and_then(|g| g.as_u64()).unwrap_or(96);
            if !(16..=160).contains(&grid) {
                return Err(format!("world grid {grid} out of range 16..=160"));
            }
            if let Some(view) = world.get("view").and_then(|v| v.as_str()) {
                if !matches!(view, "top" | "first_person") {
                    return Err("world view must be \"top\" or \"first_person\"".into());
                }
            }
            let cells = world
                .get("cells")
                .and_then(|c| c.as_array())
                .ok_or("world lacks \"cells\" []")?;
            if cells.is_empty() {
                return Err("world has zero cells".into());
            }
            for c in cells {
                let id = c
                    .get("id")
                    .and_then(|i| i.as_str())
                    .ok_or("a world cell lacks \"id\"")?;
                let script = c
                    .get("script")
                    .and_then(|s| s.as_str())
                    .ok_or_else(|| format!("world cell '{id}' lacks \"script\""))?;
                compile_check(script, &FIELD_PARAMS, &FIELD_IMPORTS, FIELD_FUEL)
                    .map_err(|e| format!("world cell '{id}' failed to compile: {e}"))?;
                if let Some(mode) = c.get("mode").and_then(|m| m.as_str()) {
                    if !matches!(mode, "once" | "frame") {
                        return Err(format!("world cell '{id}': mode must be \"once\" or \"frame\""));
                    }
                }
                if let Some(region) = c.get("region") {
                    validate_region(region, grid, id)?;
                }
            }
            if let Some(entities) = world.get("entities").and_then(|e| e.as_array()) {
                let ids: Vec<&str> = entities
                    .iter()
                    .filter_map(|e| e.get("id").and_then(|i| i.as_str()))
                    .collect();
                for ent in entities {
                    let id = ent
                        .get("id")
                        .and_then(|i| i.as_str())
                        .ok_or("an entity lacks \"id\"")?;
                    // being-carried is a relation, and relations are host law
                    if let Some(on) = ent.get("on").and_then(|o| o.as_str()) {
                        if on == id {
                            return Err(format!("entity '{id}' cannot ride itself"));
                        }
                        if !ids.contains(&on) {
                            return Err(format!("entity '{id}' rides unknown entity '{on}'"));
                        }
                        // reject cycles: walk the chain
                        let mut seen = vec![id];
                        let mut cur = on;
                        loop {
                            if seen.contains(&cur) {
                                return Err(format!("entity '{id}': riding cycle via '{cur}'"));
                            }
                            seen.push(cur);
                            match entities
                                .iter()
                                .find(|e| e.get("id").and_then(|i| i.as_str()) == Some(cur))
                                .and_then(|e| e.get("on"))
                                .and_then(|o| o.as_str())
                            {
                                Some(next) => cur = next,
                                None => break,
                            }
                        }
                    }
                    let ty = ent
                        .get("type")
                        .and_then(|t| t.as_str())
                        .ok_or_else(|| format!("entity '{id}' lacks \"type\""))?;
                    // a type is legal if it's in the curated registry OR the
                    // entity grows its own skin at runtime via a skin_seed.
                    let skin_seed = ent.get("skin_seed").and_then(|s| s.as_str());
                    if !ENTITY_TYPES.contains(&ty) && skin_seed.is_none() {
                        return Err(format!(
                            "entity '{id}': type '{ty}' not in the skin registry [{}] and no \"skin_seed\" to grow one",
                            ENTITY_TYPES.join(", ")
                        ));
                    }
                    if let Some(seed) = skin_seed {
                        compile_check(seed, &SKIN_PARAMS, &SKIN_IMPORTS, 300_000)
                            .map_err(|e| format!("entity '{id}' skin_seed failed to compile: {e}"))?;
                    }
                    let at: Vec<f64> = ent
                        .get("at")
                        .and_then(|a| a.as_array())
                        .filter(|a| a.len() == 2)
                        .map(|a| a.iter().filter_map(|v| v.as_f64()).collect())
                        .ok_or_else(|| format!("entity '{id}' needs \"at\":[x,y]"))?;
                    if at.len() != 2
                        || at[0] < 0.0
                        || at[1] < 0.0
                        || at[0] >= grid as f64
                        || at[1] >= grid as f64
                    {
                        return Err(format!("entity '{id}': at out of the {grid}×{grid} field"));
                    }
                    if let Some(m) = ent.get("mind") {
                        let persona = m
                            .get("persona")
                            .and_then(|p| p.as_str())
                            .ok_or_else(|| format!("entity '{id}': mind needs a \"persona\" string"))?;
                        if persona.len() > 500 {
                            return Err(format!("entity '{id}': persona too long (>500 chars)"));
                        }
                    }
                    if let Some(behavior) = ent.get("behavior").and_then(|b| b.as_str()) {
                        compile_check(behavior, &ENTITY_PARAMS, &ENTITY_IMPORTS, ENTITY_FUEL)
                            .map_err(|e| format!("entity '{id}' behavior failed to compile: {e}"))?;
                    }
                }
            }
            Ok(())
        }
        _ => Err("\"surface\" must be \"ui\", \"draw\", or \"field\"".into()),
    }
}

/// The mind contract: each minded being gets its own sparse-beating Claude.
/// The perception package is the ENTIRETY of the mind's world; a rewritten
/// reflex passes the same compiler gate as everything else.
const MIND_CONTRACT: &str = r#"You are the MIND of one being living on a small world-grid. Stay in character. Be brief — a being of few words.

You receive a PERCEPTION package (JSON): your position, a small window of the world around you (channels: height, water, vegetation, snow), your memory slots, whether snow falls, your last thought, and optionally WORDS someone spoke to you.

Reply with ONE JSON object only (no prose outside it):
{"say":"<one short in-character sentence (reply to words, or react) — may be empty>",
 "thought":"<one short private thought>",
 "behavior":"<OPTIONAL: rewrite your body's reflex, DSL below — omit unless the situation truly calls for a change>",
 "intent":{"7":12.5}   <OPTIONAL slot writes, keys 0..31>}

Your body's reflex is a tiny DSL script run(t, ex, ey), executed ~30 times/second:
- statements: let x = ...; x = ...; while c { }  if c { } else { }; the LAST line is a bare expression (the return value, no semicolon)
- float literals with a decimal point (2.0 not 2); identifiers letters/digits/underscore
- capabilities, NOTHING else: sin(x) cos(x) get(i) set(i,v) fr(c,x,y) [c: 0=height 1=water 2=veg 3=snow] mv(dx,dy) [tiny steps, the host clamps] unbind() [step off whatever you ride — a boat, a car; ONLY after unbind() does your own mv move you]
- ex/ey = your current position. Example, drift gently east: "mv(0.02, 0.0);
0.0"
- to move TOWARD a point, store it in slots and steer each tick, e.g. head for x=10:
  "let dx = 10.0 - ex;
mv(min(max(dx * 0.01, 0.0 - 0.03), 0.03), 0.0);
0.0"
- if you RIDE something and decide to leave (go ashore, get out), your reflex must FIRST call
  unbind(), then mv toward land (height rises away from the water). To leave a boat for shore
  at y=38: "unbind();
let dy = 38.0 - ey;
mv(0.0, min(max(dy * 0.02, 0.0 - 0.03), 0.03));
0.0"
- but you need not obey. A being of your own mind may choose to stay; staying is also an answer.
"#;

#[derive(Deserialize)]
struct MindReq {
    persona: String,
    perception: Value,
    #[serde(default)]
    words: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

/// One heartbeat of one being's mind. The reply's optional reflex rewrite is
/// validated BY COMPILING it against the entity ABI before the browser sees it.
async fn mind(Json(req): Json<MindReq>) -> impl IntoResponse {
    let model = req
        .model
        .as_deref()
        .filter(|m| ALLOWED_MODELS.contains(m))
        .unwrap_or(DEFAULT_MODEL)
        .to_string();
    let mut prompt = format!("{MIND_CONTRACT}
PERSONA: {}

PERCEPTION:
{}", req.persona, req.perception);
    if let Some(w) = &req.words {
        prompt.push_str(&format!("

WORDS spoken to you: {w}"));
    }
    let t0 = Instant::now();
    let mut last_err = String::new();
    for attempt in 1..=2u32 {
        let p = if attempt == 1 {
            prompt.clone()
        } else {
            format!("{prompt}

Your previous reply failed validation:
{last_err}
Return ONLY the corrected JSON object.")
        };
        match claude_generate(&p, &model).await {
            Ok(raw) => match extract_json(&raw) {
                Ok(obj) => {
                    if let Some(b) = obj.get("behavior").and_then(|b| b.as_str()) {
                        if let Err(e) = compile_check(b, &ENTITY_PARAMS, &ENTITY_IMPORTS, ENTITY_FUEL) {
                            eprintln!("[mind] attempt {attempt} reflex failed to compile: {e}");
                            last_err = format!("behavior failed to compile: {e}");
                            continue;
                        }
                    }
                    let mut resp = json!({
                        "ok": true,
                        "gen_ms": t0.elapsed().as_millis() as u64,
                        "attempts": attempt,
                        "say": obj.get("say").and_then(|v| v.as_str()).unwrap_or(""),
                        "thought": obj.get("thought").and_then(|v| v.as_str()).unwrap_or(""),
                    });
                    if let Some(b) = obj.get("behavior") {
                        resp["behavior"] = b.clone();
                    }
                    if let Some(i) = obj.get("intent") {
                        resp["intent"] = i.clone();
                    }
                    return (StatusCode::OK, Json(resp));
                }
                Err(e) => last_err = e,
            },
            Err(e) => last_err = e,
        }
    }
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"ok": false, "error": last_err, "gen_ms": t0.elapsed().as_millis() as u64})),
    )
}

#[derive(Deserialize)]
struct SkinSave {
    #[serde(rename = "type")]
    ty: String,
    skin_seed: String,
}

fn slug_ok(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Save a manifested world + its conversation — the whole "三千大千世界" is data
/// (surface + payload JSON + chat transcript), so a save is just a file. Loading
/// it re-manifests in µs: generate once (slow), then switch worlds at will.
async fn world_save(Json(body): Json<Value>) -> impl IntoResponse {
    use axum::http::header::CONTENT_TYPE;
    let name = body.get("name").and_then(|n| n.as_str()).unwrap_or("");
    if !slug_ok(name) {
        return (StatusCode::BAD_REQUEST, [(CONTENT_TYPE, "text/plain")], "name must be letters/digits/_/-".to_string());
    }
    let _ = tokio::fs::create_dir_all("worlds").await;
    let pretty = serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string());
    match tokio::fs::write(format!("worlds/{name}.json"), pretty).await {
        Ok(()) => (StatusCode::OK, [(CONTENT_TYPE, "text/plain")], format!("saved world '{name}'")),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, [(CONTENT_TYPE, "text/plain")], format!("save failed: {e}")),
    }
}

async fn world_get(axum::extract::Path(name): axum::extract::Path<String>) -> impl IntoResponse {
    use axum::http::header::CONTENT_TYPE;
    if !slug_ok(&name) {
        return (StatusCode::BAD_REQUEST, [(CONTENT_TYPE, "text/plain")], String::new());
    }
    match tokio::fs::read_to_string(format!("worlds/{name}.json")).await {
        Ok(s) => (StatusCode::OK, [(CONTENT_TYPE, "application/json")], s),
        Err(_) => (StatusCode::NOT_FOUND, [(CONTENT_TYPE, "text/plain")], String::new()),
    }
}

async fn world_list() -> impl IntoResponse {
    let mut names = Vec::new();
    if let Ok(mut rd) = tokio::fs::read_dir("worlds").await {
        while let Ok(Some(e)) = rd.next_entry().await {
            if let Some(n) = e.file_name().to_str().and_then(|n| n.strip_suffix(".json")) {
                names.push(n.to_string());
            }
        }
    }
    names.sort();
    Json(json!({ "worlds": names }))
}

fn skin_type_ok(ty: &str) -> bool {
    !ty.is_empty()
        && ty.len() <= 40
        && ty.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Save a self-grown skin into the library (skins-grown/<type>.dsl) — a grown
/// skin is a skin_seed string, so archival is nearly free: files are the ālaya
/// (docs §20.1). Validated by compiling against the skin ABI before it lands.
async fn skin_save(Json(req): Json<SkinSave>) -> impl IntoResponse {
    use axum::http::header::CONTENT_TYPE;
    if !skin_type_ok(&req.ty) {
        return (StatusCode::BAD_REQUEST, [(CONTENT_TYPE, "text/plain")], "bad skin type".to_string());
    }
    if ENTITY_TYPES.contains(&req.ty.as_str()) {
        return (StatusCode::CONFLICT, [(CONTENT_TYPE, "text/plain")], "that name is a curated skin".to_string());
    }
    if let Err(e) = compile_check(&req.skin_seed, &SKIN_PARAMS, &SKIN_IMPORTS, 300_000) {
        return (StatusCode::UNPROCESSABLE_ENTITY, [(CONTENT_TYPE, "text/plain")], format!("skin_seed failed to compile: {e}"));
    }
    let _ = tokio::fs::create_dir_all("skins-grown").await;
    match tokio::fs::write(format!("skins-grown/{}.dsl", req.ty), &req.skin_seed).await {
        Ok(()) => (StatusCode::OK, [(CONTENT_TYPE, "text/plain")], format!("saved skin '{}'", req.ty)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, [(CONTENT_TYPE, "text/plain")], format!("save failed: {e}")),
    }
}

/// A grown skin from the library, by type — the "manifest again next time" path.
async fn skin_get(axum::extract::Path(ty): axum::extract::Path<String>) -> impl IntoResponse {
    use axum::http::header::CONTENT_TYPE;
    if !skin_type_ok(&ty) {
        return (StatusCode::BAD_REQUEST, [(CONTENT_TYPE, "text/plain")], String::new());
    }
    match tokio::fs::read_to_string(format!("skins-grown/{ty}.dsl")).await {
        Ok(s) => (StatusCode::OK, [(CONTENT_TYPE, "text/plain")], s),
        Err(_) => (StatusCode::NOT_FOUND, [(CONTENT_TYPE, "text/plain")], String::new()),
    }
}

/// List the names in the grown-skin library.
async fn skin_list() -> impl IntoResponse {
    let mut names = Vec::new();
    if let Ok(mut rd) = tokio::fs::read_dir("skins-grown").await {
        while let Ok(Some(e)) = rd.next_entry().await {
            if let Some(n) = e.file_name().to_str().and_then(|n| n.strip_suffix(".dsl")) {
                names.push(n.to_string());
            }
        }
    }
    Json(json!({ "skins": names }))
}

/// Inhabitant package manifest — the bundle descriptor binding a Rust skin
/// (slow loop) to an AssemblyScript soul (Tier-2, audited in the browser).
async fn inhabitant_manifest(axum::extract::Path(ty): axum::extract::Path<String>) -> impl IntoResponse {
    use axum::http::header::CONTENT_TYPE;
    if !ENTITY_TYPES.contains(&ty.as_str()) {
        return (StatusCode::NOT_FOUND, [(CONTENT_TYPE, "text/plain")], "unknown inhabitant type".to_string());
    }
    match tokio::fs::read_to_string(format!("inhabitants/{ty}/manifest.json")).await {
        Ok(s) => (StatusCode::OK, [(CONTENT_TYPE, "application/json")], s),
        Err(_) => (StatusCode::NOT_FOUND, [(CONTENT_TYPE, "text/plain")], format!("no package for '{ty}'")),
    }
}

/// The packaged soul itself (asc output). The browser audits it BEFORE
/// instantiating — the server serves bytes, the fence stays client-side.
async fn inhabitant_behavior(axum::extract::Path(ty): axum::extract::Path<String>) -> impl IntoResponse {
    use axum::http::header::CONTENT_TYPE;
    if !ENTITY_TYPES.contains(&ty.as_str()) {
        return (StatusCode::NOT_FOUND, [(CONTENT_TYPE, "text/plain")], Vec::new());
    }
    match tokio::fs::read(format!("inhabitants/{ty}/behavior.wasm")).await {
        Ok(b) => (StatusCode::OK, [(CONTENT_TYPE, "application/wasm")], b),
        Err(_) => (StatusCode::NOT_FOUND, [(CONTENT_TYPE, "text/plain")], Vec::new()),
    }
}

async fn generate(Json(req): Json<GenReq>) -> impl IntoResponse {
    let model = req
        .model
        .as_deref()
        .filter(|m| ALLOWED_MODELS.contains(m))
        .unwrap_or(DEFAULT_MODEL)
        .to_string();

    let mut prompt = String::from(CONTRACT);
    if let Some(prior) = &req.prior {
        prompt.push_str("\n\nCURRENT STATE (the user wants to modify this — return the FULL updated JSON for the same surface):\n");
        prompt.push_str(&prior.to_string());
    }
    prompt.push_str("\n\nUser request: ");
    prompt.push_str(&req.prompt);

    let t0 = Instant::now();
    let mut last_err = String::new();
    let mut raw = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        let p = if attempt == 1 {
            prompt.clone()
        } else {
            // self-repair: feed the compiler/validator error back verbatim
            let trimmed: String = raw.chars().take(4000).collect();
            format!(
                "{prompt}\n\nYour previous reply failed validation:\n{last_err}\nPrevious reply:\n{trimmed}\nReturn ONLY the corrected JSON object."
            )
        };
        match claude_generate(&p, &model).await {
            Ok(text) => {
                raw = text;
                match extract_json(&raw).and_then(|obj| validate(&obj).map(|_| obj)) {
                    Ok(obj) => {
                        let mut resp = json!({
                            "ok": true,
                            "attempts": attempt,
                            "gen_ms": t0.elapsed().as_millis() as u64,
                            "model": model,
                        });
                        resp["surface"] = obj["surface"].clone();
                        if let Some(s) = obj.get("schema") {
                            resp["schema"] = s.clone();
                        }
                        if let Some(s) = obj.get("seed") {
                            resp["seed"] = s.clone();
                        }
                        if let Some(w) = obj.get("world") {
                            resp["world"] = w.clone();
                        }
                        return (StatusCode::OK, Json(resp));
                    }
                    Err(e) => {
                        eprintln!("[gen] attempt {attempt} validation failed: {e}");
                        last_err = e;
                    }
                }
            }
            Err(e) => {
                eprintln!("[gen] attempt {attempt} claude failed: {e}");
                last_err = e;
            }
        }
    }
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "ok": false,
            "error": format!("failed after {MAX_ATTEMPTS} attempts: {last_err}"),
            "raw": raw.chars().take(2000).collect::<String>(),
            "gen_ms": t0.elapsed().as_millis() as u64,
        })),
    )
}

#[tokio::main]
async fn main() {
    if std::env::var("CLAUDE_CODE_OAUTH_TOKEN").is_err() {
        eprintln!("CLAUDE_CODE_OAUTH_TOKEN is not set — the generator container needs it.");
        eprintln!("hint: set -a; . <your .env with the token>; set +a; then rerun (see gen-server/run.sh)");
        std::process::exit(1);
    }
    let app = Router::new()
        .route("/api/generate", post(generate))
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/mind", post(mind))
        .route("/api/skins", get(skin_list).post(skin_save))
        .route("/api/skins/{ty}", get(skin_get))
        .route("/api/worlds", get(world_list).post(world_save))
        .route("/api/worlds/{name}", get(world_get))
        .route("/api/inhabitants/{ty}", get(inhabitant_manifest))
        .route("/api/inhabitants/{ty}/behavior.wasm", get(inhabitant_behavior))
        .route_service("/", ServeFile::new("gen-server/live-gen.html"))
        .nest_service("/pkg", ServeDir::new("pkg"))
        .nest_service("/pkg-skins", ServeDir::new("pkg-skins"));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8646").await.unwrap();
    println!("gen-server: http://127.0.0.1:8646  (generator container: agent-task-node:local, override GEN_IMAGE)");
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_tolerates_prose_and_fences() {
        let raw = "Sure! Here is the UI:\n```json\n{\"surface\":\"draw\",\"seed\":\"0.0\"}\n```\nEnjoy.";
        let v = extract_json(raw).unwrap();
        assert_eq!(v["surface"], "draw");
    }

    #[test]
    fn multi_input_get_set_cells_validate() {
        let obj: Value = serde_json::from_str(
            r#"{"surface":"ui","schema":{
                "cells":[{"id":"bill","script":"set(0.0, x);\nx"},
                         {"id":"tip","script":"get(0.0) * 0.15"}],
                "tree":{"type":"stack","children":[
                    {"type":"input","on_input":{"cell":"bill"}},
                    {"type":"value","bind":"tip"}]},
                "wires":[{"from":"bill","to":"tip"}]}}"#,
        )
        .unwrap();
        assert!(validate(&obj).is_ok());
    }

    #[test]
    fn ui_schema_validates_end_to_end() {
        let obj: Value = serde_json::from_str(
            r#"{"surface":"ui","schema":{
                "cells":[{"id":"c","script":"x"},{"id":"f","script":"x * 1.8 + 32.0"}],
                "tree":{"type":"stack","children":[
                    {"type":"slider","min":0,"max":60,"on_input":{"cell":"c"}},
                    {"type":"value","bind":"f"}]},
                "wires":[{"from":"c","to":"f"}]}}"#,
        )
        .unwrap();
        assert!(validate(&obj).is_ok());
    }

    #[test]
    fn bad_cell_script_rejected_with_compiler_error() {
        let obj: Value = serde_json::from_str(
            r#"{"surface":"ui","schema":{
                "cells":[{"id":"c","script":"fetch(x)"}],
                "tree":{"type":"stack","children":[]}}}"#,
        )
        .unwrap();
        let e = validate(&obj).unwrap_err();
        assert!(e.contains("failed to compile"), "{e}");
    }

    #[test]
    fn unknown_widget_and_unknown_cell_rejected() {
        let obj: Value = serde_json::from_str(
            r#"{"surface":"ui","schema":{
                "cells":[{"id":"c","script":"x"}],
                "tree":{"type":"iframe","children":[]}}}"#,
        )
        .unwrap();
        assert!(validate(&obj).unwrap_err().contains("vocabulary"));
        let obj2: Value = serde_json::from_str(
            r#"{"surface":"ui","schema":{
                "cells":[{"id":"c","script":"x"}],
                "tree":{"type":"button","text":"go","on_click":{"cell":"ghost"}}}}"#,
        )
        .unwrap();
        assert!(obj2.get("schema").is_some());
        assert!(validate(&obj2).unwrap_err().contains("unknown cell"));
    }

    #[test]
    fn charts_validate_and_reject_mismatches() {
        let ok: Value = serde_json::from_str(
            r#"{"surface":"ui","schema":{
                "cells":[{"id":"lvl","script":"set(0.0, x);\nx"}],
                "init":[{"cell":"lvl","arg":40}],
                "tree":{"type":"stack","children":[
                    {"type":"barchart","title":"Reservoirs","labels":["A","B"],"values":[40,73]},
                    {"type":"linechart","labels":["Mon","Tue"],"series":[{"name":"in","values":[1,2]}]},
                    {"type":"piechart","labels":["x","y"],"values":[3,4]},
                    {"type":"gauge","bind":"lvl","min":0,"max":100},
                    {"type":"slider","min":0,"max":100,"on_input":{"cell":"lvl"}}]}}}"#,
        )
        .unwrap();
        assert!(validate(&ok).is_ok(), "{:?}", validate(&ok));

        let bad: Value = serde_json::from_str(
            r#"{"surface":"ui","schema":{
                "cells":[{"id":"c","script":"x"}],
                "tree":{"type":"barchart","labels":["A","B","C"],"values":[1,2]}}}"#,
        )
        .unwrap();
        assert!(validate(&bad).unwrap_err().contains("3 labels but 2 values"));

        let ghost: Value = serde_json::from_str(
            r#"{"surface":"ui","schema":{
                "cells":[{"id":"c","script":"x"}],
                "tree":{"type":"gauge","bind":"ghost"}}}"#,
        )
        .unwrap();
        assert!(validate(&ghost).unwrap_err().contains("unknown cell"));
    }

    const MOUNTAIN: &str = "let y = 0.0;\nwhile y < gh {\n let x = 0.0;\n while x < gw {\n  let dx = (x - gw * 0.5) / gw;\n  let dy = (y - gh * 0.5) / gh;\n  let d = sqrt(dx * dx + dy * dy);\n  let h = max(0.0, 1.0 - d * 3.0);\n  fw(0.0, x, y, fr(0.0, x, y) + h * h * 90.0);\n  x = x + 1.0;\n }\n y = y + 1.0;\n}\n1.0";

    #[test]
    fn field_world_validates() {
        let obj = serde_json::json!({
            "surface": "field",
            "world": {
                "grid": 96,
                "cells": [
                    {"id":"mountain","mode":"once","order":1,"script": MOUNTAIN},
                    {"id":"rain","mode":"frame","order":2,"region":[10,10,80,80],
                     "script":"let y = 10.0;\nwhile y < 80.0 {\n let x = 10.0;\n while x < 80.0 {\n  if sin(x * 0.3 + t) > 0.7 { fw(1.0, x, y, min(fr(1.0, x, y) + 0.1, 5.0)); }\n  x = x + 1.0;\n }\n y = y + 1.0;\n}\n1.0"}
                ]
            }
        });
        assert!(validate(&obj).is_ok(), "{:?}", validate(&obj));
    }

    #[test]
    fn entities_validate_and_reject_unknown_skin() {
        let ok = serde_json::json!({
            "surface":"field","world":{"grid":96,
                "cells":[{"id":"terrain","mode":"once","script":"1.0"}],
                "entities":[
                    {"id":"zhou","type":"boat","at":[50,40],
                     "behavior":"mv(sin(t * 0.4) * 0.03, cos(t * 0.3) * 0.02);\n0.0"},
                    {"id":"weng","type":"fisherman","at":[50,39],"behavior":"0.0"}
                ]}});
        assert!(validate(&ok).is_ok(), "{:?}", validate(&ok));

        let riding = serde_json::json!({
            "surface":"field","world":{"grid":96,"cells":[{"id":"a","script":"1.0"}],
                "entities":[
                    {"id":"zhou","type":"boat","at":[50,40],"behavior":"mv(0.01, 0.0);\n0.0"},
                    {"id":"weng","type":"fisherman","at":[50,40],"on":"zhou","behavior":"0.0"}]}});
        assert!(validate(&riding).is_ok(), "{:?}", validate(&riding));

        let ghost_ride = serde_json::json!({
            "surface":"field","world":{"cells":[{"id":"a","script":"1.0"}],
                "entities":[{"id":"weng","type":"fisherman","at":[5,5],"on":"nothing"}]}});
        assert!(validate(&ghost_ride).unwrap_err().contains("unknown entity"));

        let cycle = serde_json::json!({
            "surface":"field","world":{"cells":[{"id":"a","script":"1.0"}],
                "entities":[
                    {"id":"p","type":"person","at":[5,5],"on":"q"},
                    {"id":"q","type":"boat","at":[5,5],"on":"p"}]}});
        assert!(validate(&cycle).unwrap_err().contains("cycle"));

        let ghost_type = serde_json::json!({
            "surface":"field","world":{"cells":[{"id":"a","script":"1.0"}],
                "entities":[{"id":"x","type":"dragon","at":[5,5]}]}});
        assert!(validate(&ghost_type).unwrap_err().contains("skin registry"));

        let out_of_field = serde_json::json!({
            "surface":"field","world":{"grid":96,"cells":[{"id":"a","script":"1.0"}],
                "entities":[{"id":"x","type":"boat","at":[500,5]}]}});
        assert!(validate(&out_of_field).unwrap_err().contains("out of the"));

        let bad_behavior = serde_json::json!({
            "surface":"field","world":{"cells":[{"id":"a","script":"1.0"}],
                "entities":[{"id":"x","type":"boat","at":[5,5],"behavior":"fetch(t)"}]}});
        assert!(validate(&bad_behavior).unwrap_err().contains("failed to compile"));
    }

    #[test]
    fn field_view_enum_validated() {
        let ok = serde_json::json!({
            "surface":"field","world":{"view":"first_person","cells":[{"id":"a","script":"1.0"}]}});
        assert!(validate(&ok).is_ok());
        let bad = serde_json::json!({
            "surface":"field","world":{"view":"drone","cells":[{"id":"a","script":"1.0"}]}});
        assert!(validate(&bad).unwrap_err().contains("view"));
    }

    #[test]
    fn field_rejects_bad_mode_region_and_script() {
        let bad_mode = serde_json::json!({
            "surface":"field","world":{"cells":[{"id":"a","mode":"forever","script":"1.0"}]}});
        assert!(validate(&bad_mode).unwrap_err().contains("mode"));

        let bad_region = serde_json::json!({
            "surface":"field","world":{"grid":96,"cells":[{"id":"a","region":[0,0,200,50],"script":"1.0"}]}});
        assert!(validate(&bad_region).unwrap_err().contains("out of bounds"));

        let bad_script = serde_json::json!({
            "surface":"field","world":{"cells":[{"id":"a","script":"fetch(t)"}]}});
        assert!(validate(&bad_script).unwrap_err().contains("failed to compile"));
    }

    #[test]
    fn draw_seed_validates() {
        let obj: Value = serde_json::from_str(
            r#"{"surface":"draw","seed":"hue(0.5);\ndisc(w * 0.5, h * 0.5, 50.0 + sin(t) * 10.0);\n0.0"}"#,
        )
        .unwrap();
        assert!(validate(&obj).is_ok());
    }
}
