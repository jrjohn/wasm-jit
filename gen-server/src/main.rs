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
const UI_VOCAB: [&str; 7] = ["stack", "row", "label", "value", "button", "slider", "input"];

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

fn validate_tree(node: &Value, cell_ids: &[String]) -> Result<(), String> {
    let t = node
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or("tree node lacks \"type\"")?;
    if !UI_VOCAB.contains(&t) {
        return Err(format!("node type '{t}' not in vocabulary [{}]", UI_VOCAB.join(", ")));
    }
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
        _ => Err("\"surface\" must be \"ui\" or \"draw\"".into()),
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
        prompt.push_str("\n\nCURRENT UI (the user wants to modify this — return the FULL updated schema):\n");
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
        .route_service("/", ServeFile::new("gen-server/live-gen.html"))
        .nest_service("/pkg", ServeDir::new("pkg"));
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
    fn draw_seed_validates() {
        let obj: Value = serde_json::from_str(
            r#"{"surface":"draw","seed":"hue(0.5);\ndisc(w * 0.5, h * 0.5, 50.0 + sin(t) * 10.0);\n0.0"}"#,
        )
        .unwrap();
        assert!(validate(&obj).is_ok());
    }
}
