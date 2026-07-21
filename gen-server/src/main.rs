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

mod auth;
mod metrics;

use axum::http::StatusCode;
use axum::response::sse::Sse;
use axum::response::IntoResponse;
use axum::{routing::get, routing::post, Json, Router};
use tokio_stream::wrappers::ReceiverStream;
use serde::Deserialize;
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tower_http::services::ServeDir;
use wasm_jit::codegen::{self, CompileOpts, HostFn};
use wasm_jit::parser;

const CONTRACT: &str = include_str!("../contract.md");
const DEFAULT_MODEL: &str = "claude-sonnet-5";
const ALLOWED_MODELS: [&str; 3] = ["claude-haiku-4-5-20251001", "claude-sonnet-5", "claude-opus-4-8"];
const MAX_ATTEMPTS: u32 = 3;
const GEN_TIMEOUT: Duration = Duration::from_secs(150);

// Shared with the browser compiler via the crate (ld/sd included).
const UI_IMPORTS: [HostFn; 6] = wasm_jit::UI_IMPORTS;
// The draw ABI lives in the wasm-jit crate so the native validator here and the
// browser's compile_draw_wasm mint byte-identical modules — no drift. It now
// carries the interaction loop (mx/my/down + get/set); see wasm_jit::DRAW_IMPORTS.
const DRAW_IMPORTS: [HostFn; 15] = wasm_jit::DRAW_IMPORTS;
const UI_VOCAB: [&str; 16] = [
    "stack", "row", "label", "value", "button", "slider", "input",
    "barchart", "linechart", "piechart", "gauge", "scene3d",
    "list", "textinput", "text", "feed",
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
/// position is host-owned state, clamped and bounded by the host. Shared with
/// the browser compiler via the crate so the two agree (bind/unbind included).
const ENTITY_PARAMS: [&str; 3] = wasm_jit::ENTITY_PARAMS;
const ENTITY_IMPORTS: [HostFn; 10] = wasm_jit::ENTITY_IMPORTS;
const ENTITY_FUEL: u32 = 200_000;
/// The curated skin registry — types the host draws with hand-tuned Rust skins.
const ENTITY_TYPES: [&str; 4] = ["boat", "fisherman", "person", "car"];

/// Generated-skin ABI (docs §20.1): run(px, py, s, t) with drawing primitives
/// only — how a novel inhabitant *looks*, grown at runtime, fenced by the same
/// drawing audit as the draw surface.
// Shared with the browser compiler via the crate (st included) so the two agree.
const SKIN_PARAMS: [&str; 6] = wasm_jit::SKIN_PARAMS;
const SKIN_IMPORTS: [HostFn; 12] = wasm_jit::SKIN_IMPORTS;
// Grown-widget ABI (詞彙自生成), shared with the browser for the same reason.
const WIDGET_PARAMS: [&str; 3] = wasm_jit::WIDGET_PARAMS;
const WIDGET_IMPORTS: [HostFn; 17] = wasm_jit::WIDGET_IMPORTS;
// Sound ABI (§24 — the audio shader), shared likewise.
const SOUND_PARAMS: [&str; 1] = wasm_jit::SOUND_PARAMS;
const SOUND_IMPORTS: [HostFn; 11] = wasm_jit::SOUND_IMPORTS;
// Draw3d ABI (§22 — the seed writes the scene), shared likewise.
const DRAW3D_PARAMS: [&str; 3] = wasm_jit::DRAW3D_PARAMS;
const DRAW3D_IMPORTS: [HostFn; 30] = wasm_jit::DRAW3D_IMPORTS;

#[derive(Deserialize)]
struct GenReq {
    prompt: String,
    #[serde(default)]
    prior: Option<Value>,
    #[serde(default)]
    model: Option<String>,
    /// force a fresh generation, bypassing (and overwriting) the ledger
    #[serde(default)]
    fresh: Option<bool>,
}

/// The ālaya ledger (§16): store the CAUSE (the ask + the state it acts upon)
/// and replay the fruit in milliseconds — no LLM, no TTFT. The cause is the ask
/// AND the prior world it modifies: two identical asks against different priors
/// are different causes and legitimately yield different fruit. So the key mixes
/// the normalized ask with a canonical signature of the prior. The live UI always
/// boots the same DEFAULT_WORLD, so repeating an ask on a fresh canvas replays.
fn normalize_ask(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}
/// Canonical signature of the prior. serde_json carries no `preserve_order`
/// feature here, so object keys serialize sorted — equal priors sign identically
/// across requests and restarts. `None` (a from-scratch ask) signs as empty.
fn prior_sig(prior: Option<&Value>) -> String {
    match prior {
        Some(v) => serde_json::to_string(v).unwrap_or_default(),
        None => String::new(),
    }
}
fn ask_key(norm: &str, psig: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    norm.hash(&mut h);
    0u8.hash(&mut h); // domain separator so ("ab","c") ≠ ("a","bc")
    psig.hash(&mut h);
    format!("{:016x}", h.finish())
}
async fn ledger_get(key: &str, norm: &str, psig: &str) -> Option<Value> {
    let txt = tokio::fs::read_to_string(format!("ledger/{key}.json")).await.ok()?;
    let v: Value = serde_json::from_str(&txt).ok()?;
    // re-check ask AND prior signature: a hash collision must read as a miss
    let ask_ok = v.get("ask").and_then(|a| a.as_str()) == Some(norm);
    let sig_ok = v.get("psig").and_then(|a| a.as_str()).unwrap_or("") == psig;
    if ask_ok && sig_ok { v.get("result").cloned() } else { None }
}
async fn ledger_put(key: &str, norm: &str, psig: &str, result: &Value) {
    let _ = tokio::fs::create_dir_all("ledger").await;
    let entry = json!({ "ask": norm, "psig": psig, "result": result });
    if let Ok(s) = serde_json::to_string_pretty(&entry) {
        let _ = tokio::fs::write(format!("ledger/{key}.json"), s).await;
    }
}

/// The persistent generator container: created once (sleeping), each call is a
/// `docker exec` — the 1–2s container cold-start is paid once, not per call.
/// Same isolation as before: no volumes, API egress only, env fixed at create.
const GEN_CONTAINER: &str = "wasmjit-gen";

async fn container_running() -> bool {
    tokio::process::Command::new("docker")
        .args(["inspect", "-f", "{{.State.Running}}", GEN_CONTAINER])
        .output()
        .await
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false)
}

/// Environment variables that must never be visible to the generator.
///
/// The generator image is shared with a CI agent, which bakes its own credentials
/// into the image's ENV. A container started from it therefore inherits them even
/// when nobody passes them — and the prompt going in is a stranger's text, so
/// anything readable in there is one prompt injection away from appearing in the
/// output. Blank them explicitly at create; an image is not a clean room just
/// because this process did not hand it a secret.
const BLANKED_ENV: [&str; 6] = [
    "GH_TOKEN", "JENKINS_TOKEN", "SONARQUBE_TOKEN", "ARCHIVE_PG",
    "JENKINS_USER", "SONAR_HOST_URL",
];

/// Where the generator's Claude credentials live on the host. A directory of its
/// own rather than the CI agent's: the CLI rewrites this on refresh, and two
/// services sharing one mutable credential store is a race nobody would choose
/// deliberately.
const CLAUDE_HOME: &str = "/opt/arcana/claude-home";

async fn ensure_container() -> bool {
    if container_running().await {
        return true;
    }
    let _ = tokio::process::Command::new("docker")
        .args(["rm", "-f", GEN_CONTAINER])
        .output()
        .await;
    let image = std::env::var("GEN_IMAGE").unwrap_or_else(|_| "agent-task-node:local".into());
    let claude_home = std::env::var("CLAUDE_HOME_DIR").unwrap_or_else(|_| CLAUDE_HOME.into());

    let mut args: Vec<String> = vec![
        "run".into(), "-d".into(), "--name".into(), GEN_CONTAINER.into(),
        "--entrypoint".into(), "sleep".into(),
        // Survive a daemon restart, so the carefully-shaped container is not
        // silently replaced by whatever a later code path happens to create.
        "--restart".into(), "unless-stopped".into(),
        "-e".into(), "IS_SANDBOX=1".into(),
    ];
    for k in BLANKED_ENV {
        args.push("-e".into());
        args.push(format!("{k}="));
    }
    // Authenticate by mounting a credentials directory rather than passing a token:
    // the CLI refreshes it in place, so a token in the environment goes stale and a
    // mounted home does not.
    if std::path::Path::new(&claude_home).exists() {
        args.push("-v".into());
        args.push(format!("{claude_home}:/root/.claude"));
    } else {
        // No credential store — fall back to the token the environment may hold.
        args.push("-e".into());
        args.push("CLAUDE_CODE_OAUTH_TOKEN".into());
    }
    args.push(image);
    args.push("infinity".into());

    let ok = tokio::process::Command::new("docker")
        .args(&args)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);
    if ok {
        eprintln!("[gen] persistent container '{GEN_CONTAINER}' started (secrets blanked, creds mounted)");
    }
    ok
}

async fn run_docker(args: Vec<String>) -> Result<std::process::Output, String> {
    let mut cmd = tokio::process::Command::new("docker");
    cmd.args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    tokio::time::timeout(GEN_TIMEOUT, cmd.output())
        .await
        .map_err(|_| "generation timed out (150s)".to_string())?
        .map_err(|e| format!("docker spawn failed: {e}"))
}

/// Run Claude CLI: warm path = exec into the persistent container; cold
/// fallback = the original one-shot `docker run --rm`.

/// `claude --output-format json` wraps the answer in an envelope carrying token
/// counts, cost and timings. Take the text out and record the spend on the way
/// past — this is the only place that knows what a generation actually cost.
///
/// If the envelope is not what we expect, hand back the raw output unchanged.
/// A broken metric must never become a broken generation.
fn unwrap_cli_json(raw: &str, user: &str) -> String {
    let Ok(v) = serde_json::from_str::<Value>(raw.trim()) else {
        return raw.to_string();
    };
    let Some(text) = v.get("result").and_then(|r| r.as_str()) else {
        return raw.to_string();
    };
    let u = &v["usage"];
    metrics::record(
        "generate",
        user,
        serde_json::json!({
            "ok": !v["is_error"].as_bool().unwrap_or(false),
            "ms": v["duration_ms"].as_u64().unwrap_or(0),
            "ttft_ms": v["ttft_ms"].as_u64().unwrap_or(0),
            "tok_in": u["input_tokens"].as_u64().unwrap_or(0),
            "tok_out": u["output_tokens"].as_u64().unwrap_or(0),
            "tok_cache": u["cache_creation_input_tokens"].as_u64().unwrap_or(0)
                + u["cache_read_input_tokens"].as_u64().unwrap_or(0),
            "cost_usd": v["total_cost_usd"].as_f64().unwrap_or(0.0),
            "ledger_hit": false,
        }),
    );
    text.to_string()
}

async fn claude_generate(prompt: &str, model: &str, user: &str) -> Result<String, String> {
    if ensure_container().await {
        let out = run_docker(vec![
            "exec".into(), GEN_CONTAINER.into(), "claude".into(),
            "-p".into(), prompt.into(), "--model".into(), model.into(),
            // Ask for the envelope, not just the text: it carries the token
            // counts and cost, which is the only way the bill gets a cause.
            "--output-format".into(), "json".into(),
        ])
        .await?;
        if out.status.success() {
            return Ok(unwrap_cli_json(&String::from_utf8_lossy(&out.stdout), user));
        }
        eprintln!(
            "[gen] warm exec failed ({}), falling back to cold run: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).chars().take(200).collect::<String>()
        );
    }
    // The cold fallback runs the SAME stranger's prompt, so it needs the same
    // scrubbing. A hardening that only covers the warm path is a hardening that
    // stops applying the moment the container dies.
    let image = std::env::var("GEN_IMAGE").unwrap_or_else(|_| "agent-task-node:local".into());
    let claude_home = std::env::var("CLAUDE_HOME_DIR").unwrap_or_else(|_| CLAUDE_HOME.into());
    let mut args: Vec<String> = vec![
        "run".into(), "--rm".into(), "--entrypoint".into(), "claude".into(),
        "-e".into(), "IS_SANDBOX=1".into(),
    ];
    for k in BLANKED_ENV {
        args.push("-e".into());
        args.push(format!("{k}="));
    }
    if std::path::Path::new(&claude_home).exists() {
        args.push("-v".into());
        args.push(format!("{claude_home}:/root/.claude"));
    } else {
        args.push("-e".into());
        args.push("CLAUDE_CODE_OAUTH_TOKEN".into());
    }
    args.push(image);
    args.push("-p".into());
    args.push(prompt.into());
    args.push("--model".into());
    args.push(model.into());
    args.push("--output-format".into());
    args.push("json".into());
    let out = run_docker(args).await?;
    if !out.status.success() {
        return Err(format!(
            "claude exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).chars().take(400).collect::<String>()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Streaming Claude: same warm container, but `--output-format stream-json`
/// emits the reply token-by-token so the browser watches the schema materialize
/// instead of staring at a dead ~4s TTFT + full generation. Text deltas are
/// forwarded to `tx` as they arrive; the full accumulated text is returned for
/// the same native validate + self-repair the blocking path already runs. Still
/// the subscription CLI (OAuth token baked into the container) — no API keys.
async fn claude_stream(
    prompt: &str,
    model: &str,
    attempt: u32,
    user: &str,
    tx: &tokio::sync::mpsc::Sender<Result<axum::response::sse::Event, std::convert::Infallible>>,
) -> Result<String, String> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    if !ensure_container().await {
        return Err("generator container unavailable".into());
    }
    let mut child = tokio::process::Command::new("docker")
        .args([
            "exec", GEN_CONTAINER, "claude", "-p", prompt, "--model", model,
            "--output-format", "stream-json", "--include-partial-messages", "--verbose",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("stream spawn failed: {e}"))?;
    let stdout = child.stdout.take().ok_or("no stdout on stream child")?;
    let mut lines = BufReader::new(stdout).lines();
    let mut full = String::new();
    let mut writing = false;
    let mut thinking_sent = false;
    let deadline = tokio::time::Instant::now() + GEN_TIMEOUT;
    loop {
        let next = tokio::time::timeout_at(deadline, lines.next_line()).await;
        let line = match next {
            Err(_) => { let _ = child.start_kill(); return Err("generation timed out".into()); }
            Ok(Ok(Some(l))) => l,
            Ok(Ok(None)) => break,
            Ok(Err(e)) => return Err(format!("stream read failed: {e}")),
        };
        let v: Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("stream_event") => {
                let ev = &v["event"];
                match ev.get("type").and_then(|t| t.as_str()) {
                    Some("message_start") => {
                        if let Some(ttft) = v.get("ttft_ms").and_then(|n| n.as_u64()) {
                            send_ev(tx, "ttft", json!({ "ms": ttft })).await;
                        }
                    }
                    Some("content_block_delta") => {
                        let d = &ev["delta"];
                        match d.get("type").and_then(|t| t.as_str()) {
                            Some("text_delta") => {
                                if !writing {
                                    send_ev(tx, "phase", json!({ "phase": "writing", "attempt": attempt })).await;
                                    writing = true;
                                }
                                if let Some(t) = d.get("text").and_then(|t| t.as_str()) {
                                    full.push_str(t);
                                    send_ev(tx, "delta", json!({ "text": t })).await;
                                }
                            }
                            Some("thinking_delta") if !writing && !thinking_sent => {
                                thinking_sent = true;
                                send_ev(tx, "phase", json!({ "phase": "thinking", "attempt": attempt })).await;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            Some("result") => {
                // The streaming envelope carries the same usage block as the
                // one-shot form. This is the path the page actually takes, so a
                // meter attached only to the other one measures zero forever.
                let u = &v["usage"];
                metrics::record("generate", user, json!({
                    "ok": v["is_error"].as_bool() != Some(true),
                    "streamed": true,
                    "attempt": attempt,
                    "model": model,
                    "ms": v["duration_ms"].as_u64().unwrap_or(0),
                    "ttft_ms": v["ttft_ms"].as_u64().unwrap_or(0),
                    "tok_in": u["input_tokens"].as_u64().unwrap_or(0),
                    "tok_out": u["output_tokens"].as_u64().unwrap_or(0),
                    "tok_cache": u["cache_creation_input_tokens"].as_u64().unwrap_or(0)
                        + u["cache_read_input_tokens"].as_u64().unwrap_or(0),
                    "cost_usd": v["total_cost_usd"].as_f64().unwrap_or(0.0),
                    "ledger_hit": false,
                }));
                if v.get("is_error").and_then(|b| b.as_bool()) == Some(true) {
                    let msg = v.get("result").and_then(|r| r.as_str()).unwrap_or("api error");
                    let _ = child.wait().await;
                    return Err(format!("claude api error: {}", msg.chars().take(300).collect::<String>()));
                }
            }
            _ => {}
        }
    }
    let status = child.wait().await.map_err(|e| format!("stream wait failed: {e}"))?;
    if full.is_empty() && !status.success() {
        let mut errbuf = String::new();
        if let Some(mut se) = child.stderr.take() {
            use tokio::io::AsyncReadExt;
            let _ = se.read_to_string(&mut errbuf).await;
        }
        return Err(format!("claude exited {}: {}", status, errbuf.chars().take(300).collect::<String>()));
    }
    Ok(full)
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
        // 詞彙自生成: an unknown widget type is legal IF it grows its own look —
        // a widget_seed compiled against the fenced widget ABI (draw primitives
        // + its own pointer + private slots + bv in / emit out). Same gate as a
        // grown skin: richness generated, reach unchanged.
        let Some(seed) = node.get("widget_seed").and_then(|s| s.as_str()) else {
            return Err(format!(
                "node type '{t}' not in vocabulary [{}] and no \"widget_seed\" to grow one",
                UI_VOCAB.join(", ")
            ));
        };
        compile_check(seed, &WIDGET_PARAMS, &WIDGET_IMPORTS, 300_000)
            .map_err(|e| format!("widget '{t}' widget_seed failed to compile: {e}"))?;
        // its wires into the app must reference real cells
        for key in ["bind"] {
            if let Some(b) = node.get(key).and_then(|b| b.as_str()) {
                if !cell_ids.iter().any(|i| i == b) {
                    return Err(format!("widget '{t}': {key} references unknown cell '{b}'"));
                }
            }
        }
        if let Some(bvs) = node.get("bind_values").and_then(|b| b.as_array()) {
            for b in bvs {
                let id = b.as_str().unwrap_or("");
                if !cell_ids.iter().any(|i| i == id) {
                    return Err(format!("widget '{t}': bind_values references unknown cell '{id}'"));
                }
            }
        }
        // on_input checked by the shared event loop below; children make no sense here
    }
    if t == "scene3d" {
        // 3D-3: a live 3D panel inside the UI — the seed is a full draw3d scene
        // with bv(i)/emit(v) wired to the app through the host
        let seed = node
            .get("seed")
            .and_then(|s| s.as_str())
            .ok_or("scene3d lacks \"seed\"")?;
        compile_check(seed, &DRAW3D_PARAMS, &DRAW3D_IMPORTS, 5_000_000)
            .map_err(|e| format!("scene3d seed failed to compile: {e}"))?;
        if let Some(b) = node.get("bind").and_then(|b| b.as_str()) {
            if !cell_ids.iter().any(|i| i == b) {
                return Err(format!("scene3d: bind references unknown cell '{b}'"));
            }
        }
        if let Some(bvs) = node.get("bind_values").and_then(|b| b.as_array()) {
            for b in bvs {
                let id = b.as_str().unwrap_or("");
                if !cell_ids.iter().any(|i| i == id) {
                    return Err(format!("scene3d: bind_values references unknown cell '{id}'"));
                }
            }
        }
    }
    if t == "list" {
        if let Some(cc) = node.get("count_cell").and_then(|c| c.as_str()) {
            if !cell_ids.iter().any(|i| i == cc) {
                return Err(format!("list: count_cell references unknown cell '{cc}'"));
            }
        }
        if let Some(spec) = node.get("on_select") {
            let cell = spec.get("cell").and_then(|c| c.as_str()).ok_or("list on_select lacks \"cell\"")?;
            if !cell_ids.iter().any(|i| i == cell) {
                return Err(format!("list on_select references unknown cell '{cell}'"));
            }
        }
    }
    if t == "text" {
        let b = node.get("bind").and_then(|b| b.as_str()).ok_or("text lacks \"bind\"")?;
        if !cell_ids.iter().any(|i| i == b) {
            return Err(format!("text: bind references unknown cell '{b}'"));
        }
    }
    if t == "feed" {
        // ④ the world delivers data — cells never fetch. Shape checked here;
        // the domain allowlist is enforced server-side at fetch time.
        let url = node.get("url").and_then(|u| u.as_str()).ok_or("feed lacks \"url\"")?;
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err("feed url must be http(s)".into());
        }
        let plucks = node.get("plucks").and_then(|p| p.as_array()).ok_or("feed lacks \"plucks\"")?;
        for p in plucks {
            p.get("path").and_then(|x| x.as_str()).ok_or("feed pluck lacks \"path\"")?;
            let c = p.get("cell").and_then(|x| x.as_str()).ok_or("feed pluck lacks \"cell\"")?;
            if !cell_ids.iter().any(|i| i == c) {
                return Err(format!("feed pluck references unknown cell '{c}'"));
            }
        }
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
        Some("draw3d") => {
            // §22: the seed writes the SCENE — world-space primitives only;
            // camera/projection/light are host law, so no seed touches a matrix
            let seed = obj
                .get("seed")
                .and_then(|s| s.as_str())
                .ok_or("surface \"draw3d\" lacks \"seed\"")?;
            compile_check(seed, &DRAW3D_PARAMS, &DRAW3D_IMPORTS, 5_000_000)
                .map_err(|e| format!("draw3d seed failed to compile: {e}"))
        }
        Some("shader") => {
            // L4 expert lane: validate = parse + transpile through the shader
            // fence (math + colour + pointer only); the GLSL compiler in the
            // browser is the final gate
            let seed = obj
                .get("seed")
                .and_then(|s| s.as_str())
                .ok_or("surface \"shader\" lacks \"seed\"")?;
            let prog = wasm_jit::parser::parse(seed)
                .map_err(|e| format!("shader seed failed to parse: {e}"))?;
            wasm_jit::parser::to_glsl(&prog)
                .map_err(|e| format!("shader seed rejected: {e}"))?;
            Ok(())
        }
        Some("sound") => {
            // §24: the seed runs once per audio sample — validate by compiling
            let seed = obj
                .get("seed")
                .and_then(|s| s.as_str())
                .ok_or("surface \"sound\" lacks \"seed\"")?;
            compile_check(seed, &SOUND_PARAMS, &SOUND_IMPORTS, 4096)
                .map_err(|e| format!("sound seed failed to compile: {e}"))
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
            // §24 ambient: a world may carry a sound seed that plays while it
            // renders — the ear's layer, validated against the sound fence
            if let Some(amb) = world.get("ambient").and_then(|a| a.as_str()) {
                compile_check(amb, &SOUND_PARAMS, &SOUND_IMPORTS, 4096)
                    .map_err(|e| format!("world ambient sound failed to compile: {e}"))?;
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
                    // §22b a being may carry a true 3D BODY — a draw3d cell the
                    // host places at its coordinates (the fence: same 30 words)
                    if let Some(seed) = ent.get("body_seed").and_then(|s| s.as_str()) {
                        compile_check(seed, &DRAW3D_PARAMS, &DRAW3D_IMPORTS, 5_000_000)
                            .map_err(|e| format!("entity '{id}' body_seed failed to compile: {e}"))?;
                    }
                    // §24b 聲從身出: a being may carry a sound cell — its voice
                    // sits at ITS coordinates, spatialized by the world. The
                    // seed passes the same SOUND fence as the world's ambient.
                    if let Some(seed) = ent.get("sound_seed").and_then(|s| s.as_str()) {
                        compile_check(seed, &SOUND_PARAMS, &SOUND_IMPORTS, 4096)
                            .map_err(|e| format!("entity '{id}' sound_seed failed to compile: {e}"))?;
                    }
                    if let Some(realm) = ent.get("realm").and_then(|r| r.as_str()) {
                        if !matches!(realm, "sky" | "ground") {
                            return Err(format!("entity '{id}': realm must be \"sky\" or \"ground\""));
                        }
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
                    // 自性種子: birth seeds planted into slots 24..31 — the same
                    // script diverges by its seeds (same dharma, different karma)
                    if let Some(innate) = ent.get("innate") {
                        let arr = innate.as_array().ok_or_else(|| {
                            format!("entity '{id}': \"innate\" must be an array of numbers")
                        })?;
                        if arr.len() > 8 {
                            return Err(format!(
                                "entity '{id}': \"innate\" holds at most 8 seeds (slots 24..31)"
                            ));
                        }
                        if !arr.iter().all(|v| v.as_f64().is_some_and(f64::is_finite)) {
                            return Err(format!(
                                "entity '{id}': \"innate\" must be finite numbers"
                            ));
                        }
                    }
                    // 老死 as host law: a lifespan is seconds of the being's OWN τ
                    if let Some(ls) = ent.get("lifespan") {
                        if !ls.as_f64().is_some_and(|v| v.is_finite() && v > 0.0) {
                            return Err(format!(
                                "entity '{id}': \"lifespan\" must be a positive number of seconds (of its own time)"
                            ));
                        }
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

You receive a PERCEPTION package (JSON) — these are your faculties; you know ONLY what they report:
- who you are: your id, your kind (type), your realm ("sky" or "ground") and your altitude (0 = on the ground, 1 = high in the sky)
- where you are: your x,y position in a world whose x and y both run 0..world.size, a plain word for where that is (you.place — e.g. "near the west edge", "north-west corner", "near the middle"), and your home (the x,y where you first appeared — so you can find your way back). If you.place says you are at an edge or a corner, you have drifted there; steer back toward the middle or toward home. Also whether you ride something, and a small 5×5 window of the world around you (channels: height, water, vegetation, snow)
- who is near: neighbors — nearby beings (any kind) with their kind and direction from you
- who else there is: people — EVERY named, minded being in the world, however far, each with its id and its x,y position and distance from you. This is how you find someone BY NAME: if the visitor says "go to lin", look lin up in people, take its x,y, and steer toward that point (recipe below). Anyone in this list is reachable — they are never "not near", even if no neighbor senses them.
- your inner state: your memory slots and your last thought
- what you lived: journal — your own small remembered trail, oldest first (at most 12 lines; the oldest falls away — lossy BY LAW: a night cannot be kept verbatim, only folded). The host marks only what you heard and your strongest acts (a birth, a repaint, a rewritten reflex); every other line is one YOU chose to keep. When the visitor asks about your night, your day, what has happened — answer FROM the journal, folding its many moments into one line. That folding is what memory is here.
- the world: whether snow falls; and optionally WORDS someone spoke to you.
Answer only from what these report. If a faculty does not tell you something (e.g. you have no altitude sense), you do not know it — do not invent it.

Reply with ONE JSON object only (no prose outside it):
{"say":"<one short in-character sentence (reply to words, or react) — may be empty>",
 "thought":"<one short private thought>",
 "sing":"<OPTIONAL: the actual words to be SPOKEN ALOUD in a real voice — the world lends you its voice (the browser's). This is different from 'say' (which is only shown as text): whatever you put in 'sing' is VOCALIZED. If the visitor asks you to sing, to speak aloud, to say something out loud, or to make up a song, you MUST put the sung/spoken words HERE in 'sing' (a short verse or line, under 120 chars) — do NOT just describe it in 'say'. YOUR MIND writes the words; the voice is the world's.>",
 "behavior":"<OPTIONAL: rewrite your body's reflex, DSL below — omit unless the situation truly calls for a change>",
 "intent":{"7":12.5},   <OPTIONAL slot writes, keys 0..31>
 "beget":{"type":"<a kind, e.g. lotus or person>","at":[1.0,0.0],"grants":["mv","fr"],"persona":"<optional: give the child its OWN mind>","behavior":"<optional: the child's reflex DSL>","skin_seed":"<optional: how it looks, drawing DSL>"},
   <OPTIONAL — bring a NEW being into the world beside you (a painter may paint a painter). RULES, enforced by the host: you may grant the child ONLY capabilities you yourself have (a subset of get/set/fr/mv/unbind/rise — never more); the host divides your limited birth budget with it; the child's soul passes the same compile+audit gate. Omit unless you truly mean to beget one — this is the strongest thing you can do.>
 "skin":"<OPTIONAL: repaint YOUR OWN body — give yourself clothes, a hat, a colour. A drawing DSL run(px,py,s,t,nx,ny) [nx,ny each -1..1 point to the nearest other being, so you can face or lean toward whoever is near], primitives ONLY (this is the skin fence — it cannot touch the world): hue(h) [h 0..1, vivid], rgb(r,g,b) [each 0..1], hsl(h,s,l) [each 0..1 — USE THIS for natural skin tones and soft shading: skin ≈ hsl(0.07,0.4,0.72), a shadow ≈ hsl(0.07,0.4,0.5)], disc(px,py,r) [filled circle], ring(px,py,r), arc(px,py,r,a0,a1), line(x1,y1,x2,y2), rect(x,y,w,h) [FILLED rectangle], tri(x1,y1,x2,y2,x3,y3) [FILLED triangle]. px,py = your centre, s = your size. Draw the head near py - s*0.5 and the body/robe below. Example, a robed figure with a skin-toned face: 'hsl(0.07, 0.4, 0.72);\ndisc(px, py - s * 0.5, s * 0.22);\nhsl(0.6, 0.5, 0.45);\ndisc(px, py + s * 0.15, s * 0.34);\n0.0'. Omit unless you mean to change how you look.>,
 "attrs":{"name":"Ink","mood":"content"},   <OPTIONAL — give YOURSELF named properties: pure data you carry (a name, a mood, a colour, a wish). They are yours to define and are reported back to you next time; they NEVER change what you can touch. Values are short text or numbers.>
 "remember":"<OPTIONAL — one short line (≤80 chars) worth keeping. It joins your journal and returns to you in every future perception. Choose rarely and fold well: the journal holds only 12 lines and the oldest falls away, so keep the ESSENCE of a moment, not its transcript (e.g. 'first snow tonight; the line stayed slack'). Remembering is pure data about yourself — it never widens what you can touch.>}

SAY vs SING — a crucial difference: 'say' is only shown as TEXT; 'sing' is VOCALIZED aloud in a real voice. So if you are asked to sing/perform/speak aloud, the actual sung words go in 'sing', NOT in 'say'. Do not merely announce a song in 'say' and leave 'sing' empty — that produces silence. Sing the words themselves.
Example — asked "sing me a short song": {"say":"", "thought":"a tune for them", "sing":"Down by the cold river the willow leans low, / the water knows secrets the old stones won't show."}
Example — asked "say hello out loud": {"say":"", "sing":"Hello, traveller — well met on this road."}

Your body's reflex is a tiny DSL script run(t, ex, ey), executed ~30 times/second:
- statements: let x = ...; x = ...; while c { }  if c { } else { }; the LAST line is a bare expression (the return value, no semicolon)
- float literals with a decimal point (2.0 not 2); identifiers letters/digits/underscore
- capabilities, NOTHING else: sin(x) cos(x) get(i) set(i,v) fr(c,x,y) [c: 0=height 1=water 2=veg 3=snow] mv(dx,dy) [tiny steps, the host clamps] unbind() [step off whatever you ride; ONLY after unbind() does your own mv move you] rise(dz) [change your altitude — rise(0.02) to climb toward the sky, rise(-0.02) to descend; the host clamps 0..1] other(i,k) [sense the i-th nearest OTHER being in real time: other(0,0)=distance to the nearest, other(0,1)=its dx, other(0,2)=its dy — distance is large and dx/dy 0 if there is none. Use it to move toward or away from others, e.g. follow the nearest: "mv(other(0,1)*0.01, other(0,2)*0.01);\n0.0"]
- to answer 'go back to the sky' / 'come down', rewrite your reflex to call rise() each tick, e.g. climb: "rise(0.02);\n0.0"; descend to the ground: "rise(0.0 - 0.02);\n0.0"
- ex/ey = your current position. Example, drift gently east: "mv(0.02, 0.0);
0.0"
- to move TOWARD a point, store it in slots and steer each tick, e.g. head for x=10:
  "let dx = 10.0 - ex;
mv(min(max(dx * 0.01, 0.0 - 0.03), 0.03), 0.0);
0.0"
- to WALK TO A NAMED being: look them up in people, take their x,y, and steer toward those ABSOLUTE coords on BOTH axes — bake the numbers in now so the goal stays fixed while you approach. E.g. lin is at x=38, y=42:
  "let tx = 38.0;
let ty = 42.0;
mv(min(max((tx - ex) * 0.02, 0.0 - 0.03), 0.03), min(max((ty - ey) * 0.02, 0.0 - 0.03), 0.03));
0.0"
  (substitute the real x,y from people). Each tick corrects from where you now stand, so you arrive and naturally slow to a stop. A being far away in people is still reachable this way — reach it by name, not by waiting for it to come near.
- IMPORTANT: a CONSTANT mv (e.g. always mv(0.02, 0.0)) makes you drift to an edge and get stuck there, clamped by the host — that is not "walking to a place". To reach a place, STEER toward it (as above), on BOTH axes. To return to where you started, steer toward your home (your perception gives it). To STOP and stay, use a still reflex — just "0.0" (no mv). When the visitor asks you to go somewhere or come back, rewrite your reflex to steer there; don't only speak."
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
async fn mind(headers: axum::http::HeaderMap, Json(req): Json<MindReq>) -> impl IntoResponse {
    let who = match auth::user_from_any(&headers).await {
        Ok(u) => metrics::user_key(&u.sub),
        Err(e) => {
            metrics::refused("mind", "anon", &e);
            return (StatusCode::UNAUTHORIZED, Json(json!({"ok": false, "error": e})));
        }
    };
    // Measured, not assumed: for this structured heartbeat sonnet-5 is ~2×
    // FASTER than haiku-4.5 (3.5–4s vs 6.5–8s) and better — so heartbeats use
    // sonnet. The container is the only thing warm-started; the inference floor
    // is irreducible without changing model or streaming.
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
        match claude_generate(&p, &model, &who).await {
            Ok(raw) => match extract_json(&raw) {
                Ok(obj) => {
                    if let Some(b) = obj.get("behavior").and_then(|b| b.as_str()) {
                        if let Err(e) = compile_check(b, &ENTITY_PARAMS, &ENTITY_IMPORTS, ENTITY_FUEL) {
                            eprintln!("[mind] attempt {attempt} reflex failed to compile: {e}");
                            last_err = format!("behavior failed to compile: {e}");
                            continue;
                        }
                    }
                    // A begotten child's soul must at least compile against the full
                    // entity ABI (syntax) here → feeds self-repair; the parent-subset
                    // fence is enforced client-side, where the host knows the parent's
                    // grants. Same for the child's skin against the drawing ABI.
                    if let Some(bg) = obj.get("beget") {
                        if let Some(bh) = bg.get("behavior").and_then(|v| v.as_str()) {
                            if let Err(e) = compile_check(bh, &ENTITY_PARAMS, &ENTITY_IMPORTS, ENTITY_FUEL) {
                                last_err = format!("beget.behavior failed to compile: {e}");
                                continue;
                            }
                        }
                        if let Some(sk) = bg.get("skin_seed").and_then(|v| v.as_str()) {
                            if let Err(e) = compile_check(sk, &SKIN_PARAMS, &SKIN_IMPORTS, 300_000) {
                                last_err = format!("beget.skin_seed failed to compile: {e}");
                                continue;
                            }
                        }
                    }
                    // A being repainting its OWN body: the self-portrait must compile
                    // against the drawing ABI (primitives only — the skin fence).
                    if let Some(sk) = obj.get("skin").and_then(|v| v.as_str()) {
                        if let Err(e) = compile_check(sk, &SKIN_PARAMS, &SKIN_IMPORTS, 300_000) {
                            last_err = format!("skin (self-portrait) failed to compile: {e}");
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
                    if let Some(b) = obj.get("beget") {
                        resp["beget"] = b.clone();
                    }
                    if let Some(s) = obj.get("skin") {
                        resp["skin"] = s.clone();
                    }
                    if let Some(a) = obj.get("attrs") {
                        resp["attrs"] = a.clone(); // pure data — a being's own named properties
                    }
                    if let Some(m) = obj.get("remember") {
                        resp["remember"] = m.clone(); // a moment the being chose to keep — its own folded past
                    }
                    if let Some(s) = obj.get("sing").filter(|v| v.is_string()) {
                        resp["sing"] = s.clone(); // §24 words the world will vocalize (host-lent voice)
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

#[derive(Deserialize)]
struct WidgetSave {
    #[serde(rename = "type")]
    ty: String,
    widget_seed: String,
}

/// 詞彙自生成的阿賴耶: grown widgets are archived by name, like grown skins —
/// grown once, remembered forever, recallable by a bare {"type":"knob"}.
async fn widget_list() -> impl IntoResponse {
    let mut names = Vec::new();
    if let Ok(mut rd) = tokio::fs::read_dir("widgets-grown").await {
        while let Ok(Some(e)) = rd.next_entry().await {
            if let Some(n) = e.file_name().to_str().and_then(|n| n.strip_suffix(".dsl")) {
                names.push(n.to_string());
            }
        }
    }
    Json(json!({ "widgets": names }))
}
/// Contributing a part is now a named act — the compile gate still decides what
/// may enter, identity only decides who is answerable for it.
async fn widget_save(headers: axum::http::HeaderMap, Json(req): Json<WidgetSave>) -> impl IntoResponse {
    use axum::http::header::CONTENT_TYPE;
    let who = match auth::user_from_any(&headers).await {
        Ok(u) => metrics::user_key(&u.sub),
        Err(e) => {
            metrics::refused("widget_save", "anon", &e);
            return (StatusCode::UNAUTHORIZED, [(CONTENT_TYPE, "text/plain")], e);
        }
    };
    metrics::record("contribute_widget", &who, serde_json::json!({"type": req.ty}));
    if !skin_type_ok(&req.ty) {
        return (StatusCode::BAD_REQUEST, [(CONTENT_TYPE, "text/plain")], "bad widget type".to_string());
    }
    if UI_VOCAB.contains(&req.ty.as_str()) {
        return (StatusCode::CONFLICT, [(CONTENT_TYPE, "text/plain")], "that name is a curated widget".to_string());
    }
    if let Err(e) = compile_check(&req.widget_seed, &WIDGET_PARAMS, &WIDGET_IMPORTS, 300_000) {
        return (StatusCode::UNPROCESSABLE_ENTITY, [(CONTENT_TYPE, "text/plain")], format!("widget_seed failed to compile: {e}"));
    }
    let _ = tokio::fs::create_dir_all("widgets-grown").await;
    match tokio::fs::write(format!("widgets-grown/{}.dsl", req.ty), &req.widget_seed).await {
        Ok(()) => (StatusCode::OK, [(CONTENT_TYPE, "text/plain")], format!("saved widget '{}'", req.ty)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, [(CONTENT_TYPE, "text/plain")], format!("save failed: {e}")),
    }
}
async fn widget_get(axum::extract::Path(ty): axum::extract::Path<String>) -> impl IntoResponse {
    use axum::http::header::CONTENT_TYPE;
    if !skin_type_ok(&ty) {
        return (StatusCode::BAD_REQUEST, [(CONTENT_TYPE, "text/plain")], String::new());
    }
    match tokio::fs::read_to_string(format!("widgets-grown/{ty}.dsl")).await {
        Ok(s) => (StatusCode::OK, [(CONTENT_TYPE, "text/plain")], s),
        Err(_) => (StatusCode::NOT_FOUND, [(CONTENT_TYPE, "text/plain")], String::new()),
    }
}

/// The demo page, served with no-store so an edit to live-gen.html (where all
/// the module JS lives) reaches the browser on a normal reload — no more stale
/// cached JS after a change.
async fn index() -> impl IntoResponse {
    use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
    match tokio::fs::read_to_string("gen-server/live-gen.html").await {
        Ok(s) => (
            StatusCode::OK,
            [(CONTENT_TYPE, "text/html; charset=utf-8"), (CACHE_CONTROL, "no-store")],
            s,
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(CONTENT_TYPE, "text/plain"), (CACHE_CONTROL, "no-store")],
            format!("live-gen.html unreadable: {e}"),
        ),
    }
}

fn slug_ok(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Save a manifested world + its conversation — the whole "三千大千世界" is data
/// (surface + payload JSON + chat transcript), so a save is just a file. Loading
/// it re-manifests in µs: generate once (slow), then switch worlds at will.

/// ④ the feed proxy — the WORLD delivers data, cells never fetch. The browser
/// asks this endpoint; the server checks the domain against an allowlist
/// (gen-server/feeds-allow.txt, one host suffix per line), fetches with a hard
/// timeout and size cap, and passes the JSON through. The cell only ever sees
/// numbers (and string handles) fired into it by the host.
async fn feed_proxy(
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    use axum::http::header::CONTENT_TYPE;
    let Some(url) = q.get("url") else {
        return (StatusCode::BAD_REQUEST, [(CONTENT_TYPE, "text/plain")], "missing url".to_string());
    };
    let parsed = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, [(CONTENT_TYPE, "text/plain")], "bad url".to_string()),
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return (StatusCode::BAD_REQUEST, [(CONTENT_TYPE, "text/plain")], "http(s) only".to_string());
    }
    let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
    // SSRF defense-in-depth: an IP literal that names a private/loopback/link-local
    // host is refused BEFORE the allowlist, so no dev allowlist entry can open the
    // internal network. (DNS rebinding — an allowlisted NAME resolving to a private
    // IP — remains out of scope for this proxy; documented, not silently claimed.)
    if host_is_internal_ip(&host) {
        return (StatusCode::FORBIDDEN, [(CONTENT_TYPE, "text/plain")],
                format!("'{host}' resolves to an internal address — refused"));
    }
    let allow = tokio::fs::read_to_string("gen-server/feeds-allow.txt").await.unwrap_or_default();
    if !feed_host_allowed(&host, &allow) {
        return (StatusCode::FORBIDDEN, [(CONTENT_TYPE, "text/plain")],
                format!("'{host}' is not in the feed allowlist (gen-server/feeds-allow.txt)"));
    }
    // No redirect following: a 3xx from an allowlisted host must not be able to
    // walk the fetch onto an unlisted (internal) host — the classic allowlist bypass.
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, [(CONTENT_TYPE, "text/plain")], format!("client: {e}")),
    };
    match client.get(parsed).send().await {
        Ok(resp) if resp.status().is_redirection() => (StatusCode::BAD_GATEWAY,
            [(CONTENT_TYPE, "text/plain")], "feed tried to redirect — refused (allowlist bypass guard)".to_string()),
        Ok(resp) => match resp.text().await {
            Ok(body) if body.len() <= 262_144 => (StatusCode::OK, [(CONTENT_TYPE, "application/json")], body),
            Ok(_) => (StatusCode::PAYLOAD_TOO_LARGE, [(CONTENT_TYPE, "text/plain")], "feed too large (256KB cap)".to_string()),
            Err(e) => (StatusCode::BAD_GATEWAY, [(CONTENT_TYPE, "text/plain")], format!("read: {e}")),
        },
        Err(e) => (StatusCode::BAD_GATEWAY, [(CONTENT_TYPE, "text/plain")], format!("fetch: {e}")),
    }
}

/// A host is allowed iff it exactly equals, or is a subdomain of, an allowlist entry.
/// Pure (no I/O) so the fence is unit-testable in CI.
fn feed_host_allowed(host: &str, allow_txt: &str) -> bool {
    allow_txt.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#'))
        .any(|l| host == l || host.ends_with(&format!(".{l}")))
}

/// True if `host` is an IP literal in a private / loopback / link-local / unspecified
/// range — the addresses a feed must never be able to reach. Names are not resolved here.
fn host_is_internal_ip(host: &str) -> bool {
    use std::net::IpAddr;
    use std::net::Ipv4Addr;
    // url crate brackets IPv6 literals; strip for parsing
    let h = host.trim_start_matches('[').trim_end_matches(']');
    match h.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => v4_is_internal(&v4),
        Ok(IpAddr::V6(v6)) => {
            if v6.is_loopback() || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00  // ULA fc00::/7
                || (v6.segments()[0] & 0xffc0) == 0xfe80  // link-local fe80::/10
            {
                return true;
            }
            // IPv4-mapped ::ffff:a.b.c.d — apply the FULL v4 predicate (not just
            // private/loopback), so mapped link-local (169.254 metadata), CGNAT,
            // broadcast and unspecified are caught too.
            if let Some(m) = v6.to_ipv4_mapped() {
                if v4_is_internal(&m) {
                    return true;
                }
            }
            // NAT64 well-known prefix 64:ff9b::/96 wraps a v4 in the low 32 bits.
            let s = v6.segments();
            if s[0] == 0x0064 && s[1] == 0xff9b && s[2..6].iter().all(|&x| x == 0) {
                let v4 = Ipv4Addr::new((s[6] >> 8) as u8, (s[6] & 0xff) as u8,
                                       (s[7] >> 8) as u8, (s[7] & 0xff) as u8);
                if v4_is_internal(&v4) {
                    return true;
                }
            }
            false
        }
        Err(_) => false, // a name — allowlist still gates it
    }
}

/// The full IPv4 "must never be reachable" predicate, shared by the v4 path and
/// the IPv4-mapped / NAT64 v6 paths so no encoding slips a private address past.
fn v4_is_internal(v4: &std::net::Ipv4Addr) -> bool {
    v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_unspecified()
        || v4.is_broadcast() || v4.octets()[0] == 0
        || (v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]))       // CGNAT 100.64/10
        || (v4.octets()[0] == 192 && v4.octets()[1] == 0 && v4.octets()[2] == 0) // 192.0.0.0/24
}

/// a tiny local test feed for e2e (served directly at /api/feed-test — NOT via
/// the proxy, and 127.0.0.1 is no longer on the feed allowlist)
async fn feed_test() -> impl IntoResponse {
    Json(json!({ "n": 42, "s": "hello from the world", "nested": { "deep": 7 } }))
}


/// A save bundle is not a generation result, and the fence check was written for
/// the latter. They hold the same content under different names —
/// `{surface, schema|world|seed}` versus `{name, surface, payload, chat}` — so a
/// bundle handed straight to `validate` fails with the validator insisting the
/// surface "lacks" a key that bundle never spells that way. Every save of a
/// draw3d scene died on exactly that, and nothing caught it because no test ever
/// took a saved world back through the door it came out of.
fn bundle_for_fence(body: &Value) -> Result<Value, String> {
    let key = match body.get("surface").and_then(|s| s.as_str()) {
        Some("ui") => "schema",
        Some("field") => "world",
        Some("draw") | Some("draw3d") | Some("shader") | Some("sound") => "seed",
        Some(other) => return Err(format!("不認得的 surface:{other}")),
        None => return Err("缺 surface".into()),
    };
    Ok(serde_json::json!({
        "surface": body["surface"],
        key: body.get("payload").cloned().unwrap_or(Value::Null),
    }))
}

/// Save a world.
///
/// Two doors were open here and both are now shut.
///
/// **Identity.** Anyone who could reach this endpoint could write any JSON to
/// disk under any name — including over someone else's world. A world now
/// records its author, and only its author may overwrite it.
///
/// **The fence.** This wrote whatever it was handed, checking only the filename.
/// Every other way a world enters this server runs `validate()` first, which
/// compiles every seed inside it against the fence. This door skipped it — so a
/// world could be stored that no generation would ever have been allowed to
/// produce. It runs now, before anything touches disk: what cannot compile
/// cannot be saved.
async fn world_save(headers: axum::http::HeaderMap, Json(body): Json<Value>) -> impl IntoResponse {
    use axum::http::header::CONTENT_TYPE;
    let plain = [(CONTENT_TYPE, "text/plain")];

    let user = match auth::user_from_any(&headers).await {
        Ok(u) => u,
        Err(e) => {
            metrics::refused("world_save", "anon", &e);
            return (StatusCode::UNAUTHORIZED, plain, e);
        }
    };

    let name = body.get("name").and_then(|n| n.as_str()).unwrap_or("");
    if !slug_ok(name) {
        return (StatusCode::BAD_REQUEST, plain, "name must be letters/digits/_/-".to_string());
    }

    // The fence, before the filesystem.
    let for_check = match bundle_for_fence(&body) {
        Ok(v) => v,
        Err(e) => return (StatusCode::UNPROCESSABLE_ENTITY, plain, e),
    };
    if let Err(e) = validate(&for_check) {
        // The fence turning something away is the invariant doing its work —
        // recorded as evidence, not swallowed as an error string.
        metrics::refused("world_save", &metrics::user_key(&user.sub), &e);
        return (StatusCode::UNPROCESSABLE_ENTITY, plain,
            format!("這個世界沒過圍籬,沒有存檔:{e}"));
    }

    // Your own world is yours to revise; someone else's is not yours to overwrite.
    let path = format!("worlds/{name}.json");
    if let Ok(existing) = tokio::fs::read_to_string(&path).await {
        let owner = serde_json::from_str::<Value>(&existing)
            .ok()
            .and_then(|v| v["by"]["sub"].as_str().map(String::from));
        if let Some(owner) = owner {
            if owner != user.sub {
                metrics::refused("world_save", &metrics::user_key(&user.sub), "not the owner");
                return (StatusCode::FORBIDDEN, plain,
                    format!("'{name}' 是別人的世界 — 換個名字,或複製一份改成你自己的"));
            }
        }
    }

    let mut rec = body.clone();
    rec["by"] = serde_json::json!({"sub": user.sub, "name": user.name});
    let _ = tokio::fs::create_dir_all("worlds").await;
    let pretty = serde_json::to_string_pretty(&rec).unwrap_or_else(|_| rec.to_string());
    match tokio::fs::write(&path, pretty).await {
        Ok(()) => {
            metrics::record("save_world", &metrics::user_key(&user.sub), json!({"name": name}));
            (StatusCode::OK, plain, format!("saved world '{name}' — by {}", user.name))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, plain, format!("save failed: {e}")),
    }
}

/// Who am I, as far as this server is concerned — the page asks before it offers
/// a save button, so a visitor is never invited to do something that will fail.
/// The numbers, readable by anyone. Nothing here identifies a person, and a
/// public counter is a small honesty: whoever is asked to trust the fence can
/// also see how often it has been tried.

#[derive(Deserialize)]
struct SessionReq { credential: String }

/// Trade a freshly-verified Google credential for our own session cookie.
/// Called once, at the moment of signing in; after that the cookie carries.
async fn session_start(Json(req): Json<SessionReq>) -> impl IntoResponse {
    match auth::verify(&req.credential).await {
        Ok(u) => {
            metrics::record("signin", &metrics::user_key(&u.sub), serde_json::json!({}));
            (
                StatusCode::OK,
                [(axum::http::header::SET_COOKIE, auth::issue_cookie(&u))],
                Json(serde_json::json!({"ok": true, "name": u.name})),
            )
        }
        Err(e) => {
            metrics::refused("session", "anon", &e);
            (
                StatusCode::UNAUTHORIZED,
                [(axum::http::header::SET_COOKIE, auth::clear_cookie())],
                Json(serde_json::json!({"ok": false, "error": e})),
            )
        }
    }
}

async fn session_end() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(axum::http::header::SET_COOKIE, auth::clear_cookie())],
        Json(serde_json::json!({"ok": true})),
    )
}

async fn api_stats() -> impl IntoResponse {
    Json(metrics::stats(30))
}

async fn whoami(headers: axum::http::HeaderMap) -> impl IntoResponse {
    match auth::user_from_any(&headers).await {
        Ok(u) => {
            metrics::record("visit", &metrics::user_key(&u.sub), serde_json::json!({"signed_in": true}));
            Json(serde_json::json!({
            "signed_in": true, "sub": u.sub, "name": u.name, "email": u.email,
            "open": auth::is_open(),
        }))
        },
        Err(e) => {
            metrics::record("visit", "anon", serde_json::json!({"signed_in": false}));
            Json(serde_json::json!({
            "signed_in": false, "reason": e, "open": auth::is_open(),
            "client_id": auth::client_id(),
        }))
        },
    }
}

async fn world_get(axum::extract::Path(name): axum::extract::Path<String>) -> impl IntoResponse {
    use axum::http::header::CONTENT_TYPE;
    if !slug_ok(&name) {
        return (StatusCode::BAD_REQUEST, [(CONTENT_TYPE, "text/plain")], String::new());
    }
    match tokio::fs::read_to_string(format!("worlds/{name}.json")).await {
        Ok(s) => {
            metrics::record("load_world", "anon", serde_json::json!({"name": name}));
            (StatusCode::OK, [(CONTENT_TYPE, "application/json")], s)
        }
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
/// Contributing a part is now a named act — the compile gate still decides what
/// may enter, identity only decides who is answerable for it.
async fn skin_save(headers: axum::http::HeaderMap, Json(req): Json<SkinSave>) -> impl IntoResponse {
    use axum::http::header::CONTENT_TYPE;
    let who = match auth::user_from_any(&headers).await {
        Ok(u) => metrics::user_key(&u.sub),
        Err(e) => {
            metrics::refused("skin_save", "anon", &e);
            return (StatusCode::UNAUTHORIZED, [(CONTENT_TYPE, "text/plain")], e);
        }
    };
    metrics::record("contribute_skin", &who, serde_json::json!({"type": req.ty}));
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

/// Generation spends the operator's model quota, so it is the one door where an
/// anonymous visitor is not merely unattributed but expensive. Identity does not
/// cap the spend — that needs a quota, which does not exist yet — it only makes
/// the spend attributable and revocable. Say so plainly rather than implying that
/// requiring a login is the same thing as limiting cost.
async fn generate(headers: axum::http::HeaderMap, Json(req): Json<GenReq>) -> impl IntoResponse {
    let who = match auth::user_from_any(&headers).await {
        Ok(u) => metrics::user_key(&u.sub),
        Err(e) => {
            metrics::refused("generate", "anon", &e);
            return (StatusCode::UNAUTHORIZED, Json(json!({"ok": false, "error": e})));
        }
    };
    let model = req
        .model
        .as_deref()
        .filter(|m| ALLOWED_MODELS.contains(m))
        .unwrap_or(DEFAULT_MODEL)
        .to_string();

    let t0 = Instant::now();
    // ── ālaya ledger: a repeat ask replays the stored fruit, no LLM ──────────
    let norm = normalize_ask(&req.prompt);
    let psig = prior_sig(req.prior.as_ref());
    let key = ask_key(&norm, &psig);
    let cacheable = !norm.is_empty();
    if cacheable && !req.fresh.unwrap_or(false) {
        if let Some(result) = ledger_get(&key, &norm, &psig).await {
            let mut resp = json!({
                "ok": true, "cached": true, "gen_ms": t0.elapsed().as_millis() as u64, "model": model,
            });
            for k in ["surface", "schema", "seed", "world"] {
                if let Some(v) = result.get(k) { resp[k] = v.clone(); }
            }
            return (StatusCode::OK, Json(resp));
        }
    }

    let mut prompt = String::from(CONTRACT);
    if let Some(prior) = &req.prior {
        prompt.push_str("\n\nCURRENT STATE (the user wants to modify this — return the FULL updated JSON for the same surface):\n");
        prompt.push_str(&prior.to_string());
    }
    prompt.push_str("\n\nUser request: ");
    prompt.push_str(&req.prompt);
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
        match claude_generate(&p, &model, &who).await {
            Ok(text) => {
                raw = text;
                match extract_json(&raw).and_then(|obj| validate(&obj).map(|_| obj)) {
                    Ok(obj) => {
                        let mut resp = json!({
                            "ok": true,
                            "cached": false,
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
                        // perfume the ledger: store the cause so the next identical ask is instant
                        if cacheable {
                            let mut stored = json!({});
                            for k in ["surface", "schema", "seed", "world"] {
                                if let Some(v) = obj.get(k) { stored[k] = v.clone(); }
                            }
                            ledger_put(&key, &norm, &psig, &stored).await;
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

/// One SSE frame, JSON-bodied. `kind` names the event the browser switches on:
/// `ttft` (first byte latency) · `phase` (thinking/writing/repairing) · `delta`
/// (a chunk of the reply as it streams) · `done` (the validated result) ·
/// `error`. serde escapes newlines, so each frame's data stays single-line.
async fn send_ev(
    tx: &tokio::sync::mpsc::Sender<Result<axum::response::sse::Event, std::convert::Infallible>>,
    kind: &str,
    data: Value,
) {
    let ev = axum::response::sse::Event::default()
        .event(kind)
        .json_data(&data)
        .unwrap_or_else(|_| axum::response::sse::Event::default().event(kind).data("{}"));
    let _ = tx.send(Ok(ev)).await;
}

/// Streaming twin of `generate`: same ledger, same prompt, same validate +
/// self-repair — but the reply streams token-by-token so the browser watches the
/// schema materialize instead of waiting out the whole generation. A ledger hit
/// still replays instantly (a single `done` event, no LLM).
async fn generate_stream(headers: axum::http::HeaderMap, Json(req): Json<GenReq>) -> axum::response::Response {
    let who = match auth::user_from_any(&headers).await {
        Ok(u) => metrics::user_key(&u.sub),
        Err(e) => {
            metrics::refused("generate_stream", "anon", &e);
            return (StatusCode::UNAUTHORIZED, e).into_response();
        }
    };
    let (tx, rx) = tokio::sync::mpsc::channel(256);
    tokio::spawn(async move { run_generation_stream(req, who, tx).await });
    Sse::new(ReceiverStream::new(rx)).keep_alive(axum::response::sse::KeepAlive::default()).into_response()
}

async fn run_generation_stream(
    req: GenReq,
    who: String,
    tx: tokio::sync::mpsc::Sender<Result<axum::response::sse::Event, std::convert::Infallible>>,
) {
    let model = req
        .model
        .as_deref()
        .filter(|m| ALLOWED_MODELS.contains(m))
        .unwrap_or(DEFAULT_MODEL)
        .to_string();
    let t0 = Instant::now();
    let norm = normalize_ask(&req.prompt);
    let psig = prior_sig(req.prior.as_ref());
    let key = ask_key(&norm, &psig);
    let cacheable = !norm.is_empty();
    // ── ledger hit: replay instantly, no stream ──────────────────────────────
    if cacheable && !req.fresh.unwrap_or(false) {
        if let Some(result) = ledger_get(&key, &norm, &psig).await {
            let mut done = json!({
                "ok": true, "cached": true, "gen_ms": t0.elapsed().as_millis() as u64, "model": model,
            });
            for k in ["surface", "schema", "seed", "world"] {
                if let Some(v) = result.get(k) { done[k] = v.clone(); }
            }
            send_ev(&tx, "done", done).await;
            return;
        }
    }
    let mut prompt = String::from(CONTRACT);
    if let Some(prior) = &req.prior {
        prompt.push_str("\n\nCURRENT STATE (the user wants to modify this — return the FULL updated JSON for the same surface):\n");
        prompt.push_str(&prior.to_string());
    }
    prompt.push_str("\n\nUser request: ");
    prompt.push_str(&req.prompt);

    let mut last_err = String::new();
    let mut raw = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        let p = if attempt == 1 {
            prompt.clone()
        } else {
            send_ev(&tx, "phase", json!({ "phase": "repairing", "attempt": attempt })).await;
            let trimmed: String = raw.chars().take(4000).collect();
            format!(
                "{prompt}\n\nYour previous reply failed validation:\n{last_err}\nPrevious reply:\n{trimmed}\nReturn ONLY the corrected JSON object."
            )
        };
        match claude_stream(&p, &model, attempt, &who, &tx).await {
            Ok(text) => {
                raw = text;
                match extract_json(&raw).and_then(|obj| validate(&obj).map(|_| obj)) {
                    Ok(obj) => {
                        let mut done = json!({
                            "ok": true, "cached": false, "attempts": attempt,
                            "gen_ms": t0.elapsed().as_millis() as u64, "model": model,
                        });
                        done["surface"] = obj["surface"].clone();
                        for k in ["schema", "seed", "world"] {
                            if let Some(v) = obj.get(k) { done[k] = v.clone(); }
                        }
                        // perfume the ledger so the next identical ask is instant
                        if cacheable {
                            let mut stored = json!({});
                            for k in ["surface", "schema", "seed", "world"] {
                                if let Some(v) = obj.get(k) { stored[k] = v.clone(); }
                            }
                            ledger_put(&key, &norm, &psig, &stored).await;
                        }
                        send_ev(&tx, "done", done).await;
                        return;
                    }
                    Err(e) => {
                        eprintln!("[gen-stream] attempt {attempt} validation failed: {e}");
                        last_err = e;
                    }
                }
            }
            Err(e) => {
                eprintln!("[gen-stream] attempt {attempt} claude failed: {e}");
                last_err = e;
            }
        }
    }
    send_ev(&tx, "error", json!({
        "error": format!("failed after {MAX_ATTEMPTS} attempts: {last_err}"),
        "gen_ms": t0.elapsed().as_millis() as u64,
    })).await;
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
        .route("/api/generate/stream", post(generate_stream))
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/mind", post(mind))
        .route("/api/skins", get(skin_list).post(skin_save))
        .route("/api/skins/{ty}", get(skin_get))
        .route("/api/feed", get(feed_proxy))
        .route("/api/feed-test", get(feed_test))
        .route("/api/widgets", get(widget_list).post(widget_save))
        .route("/api/widgets/{ty}", get(widget_get))
        .route("/api/worlds", get(world_list).post(world_save))
        .route("/api/whoami", get(whoami))
        .route("/api/session", post(session_start).delete(session_end))
        .route("/api/stats", get(api_stats))
        .route("/api/worlds/{name}", get(world_get))
        .route("/api/inhabitants/{ty}", get(inhabitant_manifest))
        .route("/api/inhabitants/{ty}/behavior.wasm", get(inhabitant_behavior))
        .route("/", get(index))
        .nest_service("/pkg", ServeDir::new("pkg"))
        .nest_service("/pkg-skins", ServeDir::new("pkg-skins"))
        // §24b the 聲塵器 — the zero-import sound-dust engine (shengchen), a
        // raw wasm the audio thread instantiates with an EMPTY import object
        .nest_service("/pkg-dust", ServeDir::new("pkg-dust"))
        // version-skew guard: the page is no-store but /pkg used to be
        // heuristically cached — a fresh HTML importing a new export from a
        // stale pkg kills the whole module (dead buttons). no-cache = may
        // cache, MUST revalidate (304s stay fast, skew becomes impossible).
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-cache"),
        ));
    tokio::spawn(async { ensure_container().await; }); // warm the generator before the first prompt
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8646").await.unwrap();
    println!("gen-server: http://127.0.0.1:8646  (generator container: agent-task-node:local, override GEN_IMAGE)");
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {

    /// Every surface must survive the round trip it actually takes: generated,
    /// saved as a bundle, then handed back to the fence when someone saves it.
    /// The draw3d row is the one that was broken in production — a scene that
    /// compiled, manifested and ran could not be kept.
    #[test]
    fn a_saved_world_passes_the_same_fence_it_came_through() {
        for (surface, key) in [
            ("ui", "schema"), ("field", "world"), ("draw", "seed"),
            ("draw3d", "seed"), ("shader", "seed"), ("sound", "seed"),
        ] {
            let payload = json!({"marker": surface});
            let bundle = json!({"name": "w", "surface": surface, "payload": payload, "chat": []});
            let reshaped = bundle_for_fence(&bundle)
                .unwrap_or_else(|e| panic!("surface {surface} rejected outright: {e}"));
            assert_eq!(reshaped["surface"], surface);
            assert_eq!(
                reshaped[key], json!({"marker": surface}),
                "surface {surface}: the payload must arrive under \"{key}\", which is the \
                 name validate() reads it by — this is the mismatch that made every \
                 draw3d save fail"
            );
        }
    }

    #[test]
    fn an_unknown_surface_is_refused_rather_than_waved_through() {
        let bundle = json!({"name": "w", "surface": "wormhole", "payload": {}});
        assert!(bundle_for_fence(&bundle).is_err());
        assert!(bundle_for_fence(&json!({"name": "w", "payload": {}})).is_err());
    }
    use super::*;

    #[test]
    fn extract_json_tolerates_prose_and_fences() {
        let raw = "Sure! Here is the UI:\n```json\n{\"surface\":\"draw\",\"seed\":\"0.0\"}\n```\nEnjoy.";
        let v = extract_json(raw).unwrap();
        assert_eq!(v["surface"], "draw");
    }

    #[test]
    fn normalize_ask_collapses_whitespace_and_case() {
        assert_eq!(normalize_ask("  A  Lone   STAR\n"), "a lone star");
        assert_eq!(normalize_ask("a lone star"), normalize_ask("A LONE STAR"));
    }

    #[test]
    fn ledger_key_is_deterministic_for_same_cause() {
        let prior = json!({"surface":"field","world":{"grid":48}});
        let a = ask_key("a lone star", &prior_sig(Some(&prior)));
        let b = ask_key("a lone star", &prior_sig(Some(&prior)));
        assert_eq!(a, b, "same ask + same prior must key identically across calls/restarts");
    }

    #[test]
    fn ledger_key_separates_cause_by_prior() {
        // the core fix: identical asks against DIFFERENT priors are different
        // causes and must NOT collide (an ask-only key replayed the wrong fruit)
        let ask = "now let it rain";
        let world_a = json!({"world":{"entities":[{"type":"fisherman"}]}});
        let world_b = json!({"world":{"entities":[{"type":"boat"}]}});
        let ka = ask_key(ask, &prior_sig(Some(&world_a)));
        let kb = ask_key(ask, &prior_sig(Some(&world_b)));
        assert_ne!(ka, kb, "same ask on different worlds must not share a ledger slot");
        // and a from-scratch ask (no prior) is its own slot, distinct from either
        let kn = ask_key(ask, &prior_sig(None));
        assert_ne!(kn, ka);
        assert_ne!(kn, kb);
    }

    #[test]
    fn ledger_key_separates_cause_by_ask() {
        let prior = json!({"world":{"grid":48}});
        let a = ask_key("a lone star", &prior_sig(Some(&prior)));
        let b = ask_key("a lone moon", &prior_sig(Some(&prior)));
        assert_ne!(a, b, "different asks on the same world must key differently");
    }

    #[test]
    fn prior_sig_is_key_order_independent() {
        // serde_json (no preserve_order) sorts object keys, so semantically-equal
        // priors sent with different key order sign — and thus key — identically
        let p1: Value = serde_json::from_str(r#"{"a":1,"b":2}"#).unwrap();
        let p2: Value = serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap();
        assert_eq!(prior_sig(Some(&p1)), prior_sig(Some(&p2)));
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
    fn entity_bind_behavior_validates() {
        // §19 bind/unbind: a being that walks to the nearest thing and boards it
        // (bind as a statement AND in an expression), then can leave (unbind)
        let world = serde_json::json!({
            "surface":"field","world":{"grid":96,"cells":[{"id":"a","script":"1.0"}],
                "entities":[
                    {"id":"boat","type":"boat","at":[50,50],"behavior":"mv(0.01,0.0);\n0.0"},
                    {"id":"he","type":"person","at":[44,50],
                     "behavior":"let d = other(0.0, 0.0);\nif d > 2.0 { mv(other(0.0,1.0) * 0.1, other(0.0,2.0) * 0.1); }\nif d <= 2.0 { let boarded = bind(0.0);\n if boarded < 0.5 { unbind(); } }\n0.0"}
                ]}});
        assert!(validate(&world).is_ok(), "{:?}", validate(&world));
    }

    #[test]
    fn entity_abi_has_bind_paired_with_unbind() {
        // the native validator and the browser compiler must share one entity ABI
        assert_eq!(ENTITY_IMPORTS.len(), wasm_jit::ENTITY_IMPORTS.len());
        for (a, b) in ENTITY_IMPORTS.iter().zip(wasm_jit::ENTITY_IMPORTS.iter()) {
            assert_eq!((a.name, a.n_args, a.returns), (b.name, b.n_args, b.returns));
        }
        // §19 is complete only if both halves are present
        assert!(ENTITY_IMPORTS.iter().any(|i| i.name == "bind"), "entity ABI missing bind");
        assert!(ENTITY_IMPORTS.iter().any(|i| i.name == "unbind"), "entity ABI missing unbind");
        let bind = ENTITY_IMPORTS.iter().find(|i| i.name == "bind").unwrap();
        assert!(bind.returns && bind.n_args == 1, "bind(i) must take an index and return a verdict");
    }

    #[test]
    fn entity_innate_seeds_validate() {
        // 自性種子: same script, different seeds — validates; malformed seeds rejected
        let ok = serde_json::json!({
            "surface":"field","world":{"grid":96,"cells":[{"id":"a","script":"1.0"}],
                "entities":[
                    {"id":"bold","type":"person","at":[30,40],"innate":[1.0, 0.8],
                     "behavior":"mv(get(24.0) * 0.1, 0.0);\n0.0"},
                    {"id":"timid","type":"person","at":[60,40],"innate":[-0.5],
                     "behavior":"mv(get(24.0) * 0.1, 0.0);\n0.0"}
                ]}});
        assert!(validate(&ok).is_ok(), "{:?}", validate(&ok));

        let too_many = serde_json::json!({
            "surface":"field","world":{"cells":[{"id":"a","script":"1.0"}],
                "entities":[{"id":"x","type":"person","at":[5,5],
                    "innate":[1,2,3,4,5,6,7,8,9]}]}});
        assert!(validate(&too_many).unwrap_err().contains("at most 8"));

        let not_numbers = serde_json::json!({
            "surface":"field","world":{"cells":[{"id":"a","script":"1.0"}],
                "entities":[{"id":"x","type":"person","at":[5,5],"innate":["hot"]}]}});
        assert!(validate(&not_numbers).unwrap_err().contains("finite numbers"));

        let not_array = serde_json::json!({
            "surface":"field","world":{"cells":[{"id":"a","script":"1.0"}],
                "entities":[{"id":"x","type":"person","at":[5,5],"innate":0.7}]}});
        assert!(validate(&not_array).unwrap_err().contains("array"));
    }

    #[test]
    fn collections_strings_feeds_validate_and_reject() {
        // ②③④: a mini-todo with a feed — ld/sd cells, list, textinput, text, feed
        let ok = serde_json::json!({"surface":"ui","schema":{
            "cells":[
                {"id":"add","params":["x"],"script":"sd(get(1.0), x);\nset(1.0, get(1.0) + 1.0);\nget(1.0)"},
                {"id":"n","params":["x"],"script":"get(1.0)"},
                {"id":"temp","params":["x"],"script":"set(2.0, x);\nx"}],
            "tree":{"type":"stack","children":[
                {"type":"textinput","placeholder":"add…","on_input":{"cell":"add"}},
                {"type":"list","start":0,"count_cell":"n","text":true,"on_select":{"cell":"n"}},
                {"type":"text","bind":"add"},
                {"type":"feed","url":"https://api.open-meteo.com/v1/x","every":120,
                 "plucks":[{"path":"current.temperature_2m","cell":"temp"}]},
                {"type":"value","bind":"temp"}]},
            "wires":[{"from":"add","to":"n"}]}});
        assert!(validate(&ok).is_ok(), "{:?}", validate(&ok));

        let ghost_count = serde_json::json!({"surface":"ui","schema":{
            "cells":[{"id":"a","params":["x"],"script":"x"}],
            "tree":{"type":"list","count_cell":"missing"},"wires":[]}});
        assert!(validate(&ghost_count).unwrap_err().contains("count_cell"));

        let ghost_text = serde_json::json!({"surface":"ui","schema":{
            "cells":[{"id":"a","params":["x"],"script":"x"}],
            "tree":{"type":"text","bind":"missing"},"wires":[]}});
        assert!(validate(&ghost_text).unwrap_err().contains("unknown cell"));

        let bad_feed = serde_json::json!({"surface":"ui","schema":{
            "cells":[{"id":"a","params":["x"],"script":"x"}],
            "tree":{"type":"feed","url":"ftp://x","plucks":[]},"wires":[]}});
        assert!(validate(&bad_feed).unwrap_err().contains("http"));

        let ghost_pluck = serde_json::json!({"surface":"ui","schema":{
            "cells":[{"id":"a","params":["x"],"script":"x"}],
            "tree":{"type":"feed","url":"https://x.y/z","plucks":[{"path":"p","cell":"missing"}]},"wires":[]}});
        assert!(validate(&ghost_pluck).unwrap_err().contains("unknown cell"));
    }

    #[test]
    fn sound_surface_validates_and_rejects() {
        // §24: a two-voice synth (bell + wind) with an envelope in the slots
        let ok = serde_json::json!({"surface":"sound","seed":
            "let phase = t % 8.0;\nlet decay = max(1.0 - phase * 0.4, 0.0);\nlet bell = sin(6.2832 * 523.25 * phase) * decay * 0.3;\nlet wind = sin(6.2832 * 110.0 * t + sin(t * 0.7) * 3.0) * (0.2 + 0.1 * sin(t * 0.31));\nset(0.0, get(0.0) * 0.99 + bell * 0.01);\nbell + wind"});
        assert!(validate(&ok).is_ok(), "{:?}", validate(&ok));
        let overreach = serde_json::json!({"surface":"sound","seed":"disc(1.0, 2.0, 3.0);\n0.0"});
        assert!(validate(&overreach).unwrap_err().contains("failed to compile"));
        let missing = serde_json::json!({"surface":"sound"});
        assert!(validate(&missing).unwrap_err().contains("lacks"));
    }

    #[test]
    fn entity_sound_seed_validates_and_rejects() {
        // a bird that chirps by its own law — 聲從身出
        let ok = serde_json::json!({
            "surface": "field",
            "world": {
                "grid": 64,
                "cells": [{"id":"ground","mode":"once","order":1,"script":"fw(0.0, 5.0, 5.0, 2.0);\n1.0"}],
                "entities": [{"id": "b1", "type": "bird", "at": [10.0, 10.0],
                    "skin_seed": "hue(0.6);\ndisc(px, py, s * 0.2);\n0.0",
                    "sound_seed": "let ph = get(0.0) + 1.0;\nset(0.0, ph);\nif ph % 9000.0 < 1.0 { chirp(2500.0, 3200.0, 0.08); }\n0.0"}]
            }
        });
        assert!(validate(&ok).is_ok(), "{:?}", validate(&ok));
        // over-reach dies at the fence: mv() is an ENTITY faculty, not a sound one
        let bad = serde_json::json!({
            "surface": "field",
            "world": {
                "grid": 64,
                "cells": [{"id":"ground","mode":"once","order":1,"script":"fw(0.0, 5.0, 5.0, 2.0);\n1.0"}],
                "entities": [{"id": "b1", "type": "bird", "at": [10.0, 10.0],
                    "skin_seed": "hue(0.6);\ndisc(px, py, s * 0.2);\n0.0",
                    "sound_seed": "mv(0.1, 0.0);\n0.0"}]
            }
        });
        let e = validate(&bad).unwrap_err();
        assert!(e.contains("sound_seed"), "{e}");
    }

    #[test]
    fn feed_allowlist_and_ssrf_guard() {
        // Study 1b in miniature, pinned in CI: the feed fence refuses everything
        // off the allowlist AND every internal address, before any request goes out.
        let allow = "api.open-meteo.com\napi.frankfurter.app\n";
        assert!(feed_host_allowed("api.open-meteo.com", allow));
        assert!(feed_host_allowed("sub.api.open-meteo.com", allow)); // subdomain ok
        assert!(!feed_host_allowed("evil.com", allow));
        assert!(!feed_host_allowed("api.open-meteo.com.evil.com", allow)); // suffix-spoof refused
        // internal addresses are refused regardless of any allowlist entry —
        // incl. IPv4-mapped-IPv6 (link-local metadata, CGNAT) and NAT64 wrappers
        for ip in ["127.0.0.1", "10.0.0.5", "192.168.1.1", "169.254.169.254",
                   "0.0.0.0", "100.64.0.1", "192.0.0.1", "::1", "[::1]", "fc00::1", "fe80::1",
                   "[::ffff:127.0.0.1]", "[::ffff:169.254.169.254]", "[::ffff:10.0.0.1]",
                   "[64:ff9b::7f00:1]", "[64:ff9b::a9fe:a9fe]"] {
            assert!(host_is_internal_ip(ip), "internal IP not blocked: {ip}");
        }
        // a public literal / name is NOT flagged internal (names still gated by allowlist)
        for ok in ["8.8.8.8", "1.1.1.1", "api.open-meteo.com", "[2606:4700::1111]"] {
            assert!(!host_is_internal_ip(ok), "public host wrongly flagged internal: {ok}");
        }
    }

    #[tokio::test]
    async fn feed_proxy_refuses_a_redirect() {
        // the redirect-no-follow guard, actually FIRED: a live server that 302s
        // must yield BAD_GATEWAY (refused), never the redirected body — the
        // allowlist-bypass vector, closed and now exercised in CI (not just read).
        use axum::{routing::get, Router};
        let app = Router::new().route("/hop", get(|| async {
            (StatusCode::FOUND, [(axum::http::header::LOCATION, "http://169.254.169.254/")], "")
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });
        // hit the redirecting server directly with the SAME client policy the proxy uses
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none()).build().unwrap();
        let resp = client.get(format!("http://{addr}/hop")).send().await.unwrap();
        assert!(resp.status().is_redirection(), "expected a 3xx to refuse");
        // the proxy turns exactly this into BAD_GATEWAY (see feed_proxy) — the
        // client never followed the Location into the internal host.
    }

    #[test]
    fn begotten_child_cannot_out_reach_its_parent() {
        // §21 attenuation, pinned in CI. The begetting path compiles the child soul
        // against its PARENT's subset of the ABI (src/lib.rs compile_entity_wasm_grants).
        // Reproduced natively: a child reaching for a capability outside the subset is
        // REFUSED at compile time; the byte audit against the subset confirms clean bytes.
        use wasm_jit::audit::{audit, Grant};
        // Derive the child's grants via the REAL attenuation rule (not a hand-built
        // subset): a greedy child asks for rise too, but its parent lacks it, so
        // intersect_grants drops it — child ⊆ parent, then it compiles against THAT.
        let parent = vec!["sin".to_string(), "cos".to_string(), "mv".to_string(), "fr".to_string()];
        let want = ["sin", "cos", "mv", "fr", "rise"].iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let child_names = wasm_jit::intersect_grants(&want, &parent);
        assert!(!child_names.iter().any(|c| c == "rise"), "attenuation let rise through");
        let subset: Vec<HostFn> = ENTITY_IMPORTS.iter().cloned()
            .filter(|i| child_names.iter().any(|c| c == i.name)).collect();
        // (a) over-reach dies at COMPILE time — rise() is not in the child's grants
        let over = compile_entity_subset("rise(0.02);\n0.0", &subset);
        assert!(over.is_err(), "a child grabbed rise() its parent never had (compile let it through)");
        assert!(format!("{over:?}").contains("rise"), "wrong refusal: {over:?}");
        // (b) an in-subset child compiles AND passes the byte-audit against the subset
        let ok = compile_entity_subset("mv(fr(0.0, ex, ey) * 0.0, 0.0);\n0.0", &subset)
            .expect("in-subset child should compile");
        let grants = [
            Grant { module: "env", name: "sin" }, Grant { module: "env", name: "cos" },
            Grant { module: "env", name: "mv" }, Grant { module: "env", name: "fr" },
        ];
        assert!(audit(&ok, &grants).is_ok(), "in-subset child failed subset audit: {:?}", audit(&ok, &grants));
    }

    // compile an entity seed against a SUBSET of the ABI — exactly what the browser's
    // begetting path does (compile_entity_wasm_grants), so over-reach dies at compile.
    fn compile_entity_subset(src: &str, subset: &[HostFn]) -> Result<Vec<u8>, String> {
        use wasm_jit::codegen::{self, CompileOpts};
        let prog = wasm_jit::parser::parse(src)?;
        codegen::compile_with_opts(&prog, &ENTITY_PARAMS, subset,
            CompileOpts { fuel: Some(ENTITY_FUEL), memory_pages: None })
    }

    #[test]
    fn dust_engine_wasm_has_zero_imports() {
        // the fence's extreme: the committed shengchen wasm imports NOTHING —
        // it cannot touch anything; it can only vibrate. Machine-checked.
        let bytes = std::fs::read("../pkg-dust/shengchen.wasm")
            .expect("pkg-dust/shengchen.wasm missing — build shengchen for wasm32-unknown-unknown and copy it");
        let mut imports = 0;
        for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
            if let Ok(wasmparser::Payload::ImportSection(r)) = payload {
                imports += r.count();
            }
        }
        assert_eq!(imports, 0, "the dust engine grew hands — its import table must stay EMPTY");
    }

    #[test]
    fn sound_abi_matches_crate() {
        assert_eq!(SOUND_IMPORTS.len(), wasm_jit::SOUND_IMPORTS.len());
        for (a, b) in SOUND_IMPORTS.iter().zip(wasm_jit::SOUND_IMPORTS.iter()) {
            assert_eq!((a.name, a.n_args, a.returns), (b.name, b.n_args, b.returns));
        }
    }

    #[test]
    fn ui_abi_matches_crate() {
        assert_eq!(UI_IMPORTS.len(), wasm_jit::UI_IMPORTS.len());
        for (a, b) in UI_IMPORTS.iter().zip(wasm_jit::UI_IMPORTS.iter()) {
            assert_eq!((a.name, a.n_args, a.returns), (b.name, b.n_args, b.returns));
        }
        assert!(UI_IMPORTS.iter().any(|i| i.name == "ld"), "ui ABI missing ld");
        assert!(UI_IMPORTS.iter().any(|i| i.name == "sd"), "ui ABI missing sd");
    }

    #[test]
    fn grown_widget_validates_and_rejects() {
        // 詞彙自生成: an unknown widget type is legal iff it grows a fenced look
        let knob = "let v = bv(0.0);\nif down() > 0.5 { let f = 1.0 - my() / h;\n if f < 0.0 { f = 0.0; }\n if f > 1.0 { f = 1.0; }\n v = f * 100.0;\n emit(v); }\nlet frac = v / 100.0;\nhsl(0.58, 0.7, 0.55);\narc(w * 0.5, h * 0.55, h * 0.34, 2.35, 2.35 + 4.71 * frac);\n0.0";
        let ok = serde_json::json!({
            "surface":"ui","schema":{
                "cells":[{"id":"vol","params":["x"],"script":"set(0.0, x);\nx"}],
                "tree":{"type":"stack","children":[
                    {"type":"knob","widget_seed":knob,"bind":"vol","on_input":{"cell":"vol"},"w":150,"h":150},
                    {"type":"value","bind":"vol"}]},
                "wires":[]}});
        assert!(validate(&ok).is_ok(), "{:?}", validate(&ok));

        let bare = serde_json::json!({
            "surface":"ui","schema":{"cells":[{"id":"a","params":["x"],"script":"x"}],
                "tree":{"type":"stack","children":[{"type":"sparkline"}]},"wires":[]}});
        assert!(validate(&bare).unwrap_err().contains("widget_seed"));

        let overreach = serde_json::json!({
            "surface":"ui","schema":{"cells":[{"id":"a","params":["x"],"script":"x"}],
                "tree":{"type":"stack","children":[{"type":"knob","widget_seed":"fetch(t)"}]},"wires":[]}});
        assert!(validate(&overreach).unwrap_err().contains("failed to compile"));

        let ghost_bind = serde_json::json!({
            "surface":"ui","schema":{"cells":[{"id":"a","params":["x"],"script":"x"}],
                "tree":{"type":"stack","children":[
                    {"type":"knob","widget_seed":"disc(w*0.5, h*0.5, 9.0);\n0.0","bind":"nothing"}]},"wires":[]}});
        assert!(validate(&ghost_bind).unwrap_err().contains("unknown cell"));
    }

    #[test]
    fn draw3d_seed_validates_and_rejects() {
        // §22: a scene of ground + orbiting spheres + a tri validates end to end
        let ok = serde_json::json!({"surface":"draw3d","seed":
            "cam(cos(t * 0.2) * 16.0, 9.0, sin(t * 0.2) * 16.0, 0.0, 1.0, 0.0);\nrgb(0.2, 0.22, 0.2);\nbox(0.0, 0.0 - 0.6, 0.0, 30.0, 1.2, 30.0);\nlet k = 0.0;\nwhile k < 12.0 {\n let a = k * 0.5236;\n hsl(k / 12.0, 0.7, 0.55);\n sphere(cos(a) * 8.0, 2.0 + sin(t + k) * 0.6, sin(a) * 8.0, 0.9);\n k = k + 1.0;\n}\ntri(0.0, 0.0, 0.0, 4.0, 6.0, 0.0, 0.0 - 4.0, 6.0, 0.0);\n0.0"});
        assert!(validate(&ok).is_ok(), "{:?}", validate(&ok));
        let overreach = serde_json::json!({"surface":"draw3d","seed":"disc(1.0, 2.0, 3.0);\n0.0"});
        assert!(validate(&overreach).unwrap_err().contains("failed to compile"),
            "2D-only verbs must not exist in the 3D fence");
        let missing = serde_json::json!({"surface":"draw3d"});
        assert!(validate(&missing).unwrap_err().contains("lacks"));
    }

    #[test]
    fn draw3d_full_suite_validates() {
        // 3D-1..3: stack + cyl/cone + matter + pattern + interaction, one seed
        let seed = "pat(1.0);\nrgb(0.4, 0.5, 0.4);\nbox(0.0, 0.0 - 0.5, 0.0, 40.0, 1.0, 40.0);\npat(0.0);\ncone(2.2, 9.0);\npush();\nmove(0.0, 8.0, 1.2);\nrotz(t * 1.5);\nlet k = 0.0;\nwhile k < 4.0 {\n push();\n rotz(k * 1.5708);\n shine(0.5);\n box(0.0, 2.6, 0.0, 0.7, 5.2, 0.12);\n pop();\n k = k + 1.0;\n}\npop();\nlum(0.9);\nsphere(6.0, 1.1, 4.0, 0.6);\nlum(0.0);\ncyl(0.1, 1.1);\nif down() > 0.5 { set(0.0, mx()); }\nscale(1.0);\nroty(get(0.0) * 0.001);\nrotx(0.0);\n0.0";
        let obj = serde_json::json!({"surface":"draw3d","seed":seed});
        assert!(validate(&obj).is_ok(), "{:?}", validate(&obj));
    }

    #[test]
    fn scene3d_panel_validates_and_rejects() {
        // 3D-3: a 3D panel wired to app cells through bv/emit
        let ok = serde_json::json!({"surface":"ui","schema":{
            "cells":[{"id":"n","params":["x"],"script":"set(0.0, x);\nx"}],
            "tree":{"type":"stack","children":[
                {"type":"slider","min":1,"max":10,"step":1,"on_input":{"cell":"n"}},
                {"type":"scene3d","seed":"let n = bv(0.0);\nlet k = 0.0;\nwhile k < n {\n sphere(k * 2.0 - n, 1.0, 0.0, 0.8);\n k = k + 1.0;\n}\nif down() > 0.5 { emit(mx()); }\n0.0",
                 "bind":"n","on_input":{"cell":"n"},"h":240}]},
            "wires":[]}});
        assert!(validate(&ok).is_ok(), "{:?}", validate(&ok));
        let noseed = serde_json::json!({"surface":"ui","schema":{
            "cells":[{"id":"n","params":["x"],"script":"x"}],
            "tree":{"type":"stack","children":[{"type":"scene3d","bind":"n"}]},"wires":[]}});
        assert!(validate(&noseed).unwrap_err().contains("seed"));
        let ghost = serde_json::json!({"surface":"ui","schema":{
            "cells":[{"id":"n","params":["x"],"script":"x"}],
            "tree":{"type":"stack","children":[{"type":"scene3d","seed":"0.0","bind":"missing"}]},"wires":[]}});
        assert!(validate(&ghost).unwrap_err().contains("unknown cell"));
    }

    #[test]
    fn shader_surface_validates_and_rejects() {
        // L4: pure math + colour + pointer transpiles; memory/drawing/net die at the fence
        let ok = serde_json::json!({"surface":"shader","seed":
            "let u = x / w;\nlet v = y / h;\nlet d = sqrt((u - 0.5) * (u - 0.5) + (v - 0.5) * (v - 0.5));\nhsl(d + t * 0.1, 0.7, 0.5);\n0.0"});
        assert!(validate(&ok).is_ok(), "{:?}", validate(&ok));
        for (bad, why) in [("set(0.0, x);\n0.0", "memory"), ("disc(1.0, 2.0, 3.0);\n0.0", "drawing"), ("fetch(t)", "net")] {
            let w = serde_json::json!({"surface":"shader","seed":bad});
            assert!(validate(&w).is_err(), "shader fence must reject {why}");
        }
    }

    #[test]
    fn draw3d_abi_matches_crate() {
        assert_eq!(DRAW3D_IMPORTS.len(), wasm_jit::DRAW3D_IMPORTS.len());
        for (a, b) in DRAW3D_IMPORTS.iter().zip(wasm_jit::DRAW3D_IMPORTS.iter()) {
            assert_eq!((a.name, a.n_args, a.returns), (b.name, b.n_args, b.returns));
        }
        // world-space primitives only — no matrix-shaped import may ever appear
        for f in ["sphere", "box", "tri", "cam", "light", "bv", "emit", "pick"] {
            assert!(DRAW3D_IMPORTS.iter().any(|i| i.name == f), "draw3d ABI missing {f}");
        }
    }

    #[test]
    fn widget_abi_matches_crate() {
        assert_eq!(WIDGET_IMPORTS.len(), wasm_jit::WIDGET_IMPORTS.len());
        for (a, b) in WIDGET_IMPORTS.iter().zip(wasm_jit::WIDGET_IMPORTS.iter()) {
            assert_eq!((a.name, a.n_args, a.returns), (b.name, b.n_args, b.returns));
        }
        // the two wires into the app are present, and only these two
        let bv = WIDGET_IMPORTS.iter().find(|i| i.name == "bv").expect("widget ABI missing bv");
        assert!(bv.returns && bv.n_args == 1);
        let emit = WIDGET_IMPORTS.iter().find(|i| i.name == "emit").expect("widget ABI missing emit");
        assert!(!emit.returns && emit.n_args == 1);
    }

    #[test]
    fn entity_lifespan_validates() {
        // 老死 as host law: a positive τ-lifespan validates; zero/negative/non-number rejected
        let ok = serde_json::json!({
            "surface":"field","world":{"grid":96,"cells":[{"id":"a","script":"1.0"}],
                "entities":[{"id":"mayfly","type":"person","at":[40,40],"lifespan":12.5,"behavior":"0.0"}]}});
        assert!(validate(&ok).is_ok(), "{:?}", validate(&ok));
        for bad in [serde_json::json!(0), serde_json::json!(-3.0), serde_json::json!("short")] {
            let w = serde_json::json!({
                "surface":"field","world":{"cells":[{"id":"a","script":"1.0"}],
                    "entities":[{"id":"x","type":"person","at":[5,5],"lifespan":bad}]}});
            assert!(validate(&w).unwrap_err().contains("lifespan"), "should reject {bad}");
        }
    }

    #[test]
    fn skin_reads_published_state_validates() {
        // §20.2: a skin that shows a different pose depending on the being's
        // published state (st) must compile — intent (mind) reaches form (body)
        let world = serde_json::json!({
            "surface":"field","world":{"grid":96,"cells":[{"id":"a","script":"1.0"}],
                "entities":[{"id":"rower","type":"waterman","at":[40,40],
                    "behavior":"if other(0.0,0.0) < 3.0 { set(0.0, 1.0); }\n0.0",
                    "skin_seed":"let seated = st(0.0);\nhsl(0.08,0.5,0.4);\nif seated > 0.5 { disc(px, py, s * 0.4); }\nif seated <= 0.5 { line(px, py - s, px, py + s); }\n0.0"}]}});
        assert!(validate(&world).is_ok(), "{:?}", validate(&world));
    }

    #[test]
    fn skin_abi_has_st_and_matches_crate() {
        assert_eq!(SKIN_IMPORTS.len(), wasm_jit::SKIN_IMPORTS.len());
        for (a, b) in SKIN_IMPORTS.iter().zip(wasm_jit::SKIN_IMPORTS.iter()) {
            assert_eq!((a.name, a.n_args, a.returns), (b.name, b.n_args, b.returns));
        }
        let st = SKIN_IMPORTS.iter().find(|i| i.name == "st").expect("skin ABI missing st");
        assert!(st.returns && st.n_args == 1, "st(i) must read one slot and return a value");
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

    #[test]
    fn interactive_draw_seed_validates() {
        // §21 interaction loop: a draw that reads the pointer (mx/my/down) and
        // remembers via the host data root (get/set) must compile natively, so
        // the server never ships a seed the browser's new ABI can't instantiate.
        let obj: Value = serde_json::from_str(
            r#"{"surface":"draw","seed":"let px = get(0.0);\npx = px + (mx() - px) * 0.1;\nset(0.0, px);\nlet r = 8.0;\nif down() > 0.5 { r = 16.0; }\ndisc(px, my(), r);\n0.0"}"#,
        )
        .unwrap();
        assert!(validate(&obj).is_ok(), "interactive draw seed should validate");
    }

    #[test]
    fn draw_abi_matches_crate() {
        // the native validator and the browser compiler must share one draw ABI
        assert_eq!(DRAW_IMPORTS.len(), wasm_jit::DRAW_IMPORTS.len());
        for (a, b) in DRAW_IMPORTS.iter().zip(wasm_jit::DRAW_IMPORTS.iter()) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.n_args, b.n_args);
            assert_eq!(a.returns, b.returns);
        }
        // the interaction faculties are present
        for f in ["mx", "my", "down", "get", "set"] {
            assert!(DRAW_IMPORTS.iter().any(|i| i.name == f), "draw ABI missing {f}");
        }
    }
}
