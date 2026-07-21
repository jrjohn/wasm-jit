//! What actually happened here — an append-only event log, and the numbers read back from it.
//!
//! ## What is worth recording
//!
//! Visitor counts are the least interesting thing this can measure. The claim
//! this whole substrate makes is that a fence holds regardless of what the
//! generator writes, so the number that carries evidence is **how often the
//! fence refused something** — which door, and why. Those rows are the record
//! of the invariant doing work, and they are exactly the corpus the red-team
//! study needs. So refusals are first-class events, not error-log noise.
//!
//! After that, in order of what a reader learns per row:
//!   - refusals: a fence turned something away (the invariant, observed)
//!   - spend: tokens and cost per generation, so the bill has a cause
//!   - creation: worlds kept, words contributed — did anyone actually build
//!   - reach: distinct people, visits
//!
//! ## On identity
//!
//! People are counted by a **salted hash** of their Google subject id, so
//! "how many distinct people" is answerable without this file holding anyone's
//! identity. The salt lives beside the log and never leaves the host; lose it
//! and the old rows become permanently anonymous, which is the correct
//! direction for a file to fail in.
//!
//! Be clear about what this does NOT make private: a world records its author's
//! display name (that is the point of attribution), and the ālaya ledger stores
//! every ask verbatim, keyed by its cause. Prompts are retained — by the ledger,
//! deliberately, because replaying a cause is a feature. This log keeps only the
//! length and a hash, but it would be dishonest to present that as prompt
//! privacy when the ledger sits next to it.

use serde_json::{json, Value};
use std::io::Write;

const DIR: &str = "metrics";

/// Per-install salt for subject hashing. Created on first use; if it cannot be
/// written we hash with an empty salt rather than fail a request — a degraded
/// count beats a dropped event.
fn salt() -> String {
    let path = format!("{DIR}/.salt");
    if let Ok(s) = std::fs::read_to_string(&path) {
        if !s.trim().is_empty() {
            return s.trim().to_string();
        }
    }
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let s = format!("{seed:x}{:x}", std::process::id());
    let _ = std::fs::create_dir_all(DIR);
    let _ = std::fs::write(&path, &s);
    s
}

/// FNV-1a over salt + subject, truncated. Enough to distinguish people in a log
/// of this size; deliberately not a password hash, because it is not guarding a
/// password — it is avoiding storing an identifier we have no use for.
pub fn user_key(sub: &str) -> String {
    if sub.is_empty() {
        return "anon".into();
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in salt().bytes().chain(sub.bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    format!("u{h:012x}")
}

fn today() -> String {
    // Days since epoch, formatted as a date. No chrono dependency for one label.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let (mut y, mut d) = (1970i64, days);
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let len = if leap { 366 } else { 365 };
        if d < len { break; }
        d -= len; y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let ml = [31, if leap {29} else {28}, 31,30,31,30,31,31,30,31,30,31];
    let mut m = 0;
    while d >= ml[m] { d -= ml[m]; m += 1; }
    format!("{y:04}-{:02}-{:02}", m + 1, d + 1)
}

/// Append one event. Never fails a request: if the log cannot be written the
/// request still completes, because losing a metric is not worth losing a world.
pub fn record(kind: &str, user: &str, mut fields: Value) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Some(o) = fields.as_object_mut() {
        o.insert("ts".into(), json!(ts));
        o.insert("kind".into(), json!(kind));
        o.insert("user".into(), json!(user));
    }
    let _ = std::fs::create_dir_all(DIR);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(format!("{DIR}/events-{}.jsonl", today()))
    {
        let _ = writeln!(f, "{fields}");
    }
    // Also to the journal, so `journalctl -u arcana-world` tells the same story.
    eprintln!("[event] {kind} {fields}");
}

/// A refusal is the fence doing its job — logged as its own kind so it can be
/// counted, read, and turned into regression tests.
pub fn refused(door: &str, user: &str, reason: &str) {
    record("refused", user, json!({ "door": door, "reason": reason }));
}

/// Read the log back. Small site, small files — aggregate on read rather than
/// keeping counters that can drift from the events that produced them.
pub fn stats(days: usize) -> Value {
    let mut people = std::collections::HashSet::new();
    let (mut visits, mut gens, mut gen_ok, mut saves, mut loads, mut words, mut refusals) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut tok_in, mut tok_out, mut tok_cache, mut cost, mut ledger_hits, mut gen_ms) =
        (0u64, 0u64, 0u64, 0f64, 0u64, 0u64);
    let mut by_reason: std::collections::HashMap<String, u64> = Default::default();
    let mut by_day: std::collections::HashMap<String, u64> = Default::default();

    let mut files: Vec<_> = std::fs::read_dir(DIR)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("events-"))
        .collect();
    files.sort_by_key(|e| e.file_name());
    for e in files.iter().rev().take(days.max(1)) {
        let Ok(txt) = std::fs::read_to_string(e.path()) else { continue };
        let day = e.file_name().to_string_lossy().replace("events-", "").replace(".jsonl", "");
        for line in txt.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            let kind = v["kind"].as_str().unwrap_or("");
            let user = v["user"].as_str().unwrap_or("anon");
            if user != "anon" { people.insert(user.to_string()); }
            *by_day.entry(day.clone()).or_default() += 1;
            match kind {
                "visit" => visits += 1,
                "generate" => {
                    gens += 1;
                    if v["ok"].as_bool().unwrap_or(false) { gen_ok += 1; }
                    if v["ledger_hit"].as_bool().unwrap_or(false) { ledger_hits += 1; }
                    tok_in += v["tok_in"].as_u64().unwrap_or(0);
                    tok_out += v["tok_out"].as_u64().unwrap_or(0);
                    tok_cache += v["tok_cache"].as_u64().unwrap_or(0);
                    cost += v["cost_usd"].as_f64().unwrap_or(0.0);
                    gen_ms += v["ms"].as_u64().unwrap_or(0);
                }
                "save_world" => saves += 1,
                "load_world" => loads += 1,
                "contribute_skin" | "contribute_widget" => words += 1,
                "refused" => {
                    refusals += 1;
                    let r = v["reason"].as_str().unwrap_or("?");
                    // Group by the first clause; the tail is usually a cell id.
                    let key = r.split(&['—', ':'][..]).next().unwrap_or(r).trim();
                    *by_reason.entry(key.chars().take(72).collect()).or_default() += 1;
                }
                _ => {}
            }
        }
    }
    let mut reasons: Vec<_> = by_reason.into_iter().collect();
    reasons.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    reasons.truncate(12);

    json!({
        "people": people.len(),
        "visits": visits,
        "created": { "worlds_saved": saves, "worlds_loaded": loads, "words_contributed": words },
        "generation": {
            "attempts": gens, "succeeded": gen_ok,
            "ledger_hits": ledger_hits,
            "avg_ms": if gens > 0 { gen_ms / gens } else { 0 },
        },
        // The fence, observed. If this is zero it does not mean the fence is
        // useless — it means nothing has tried it yet.
        "fence": { "refusals": refusals, "top_reasons": reasons },
        "spend": {
            "input_tokens": tok_in, "output_tokens": tok_out,
            "cache_tokens": tok_cache,
            "usd": (cost * 10_000.0).round() / 10_000.0,
        },
        "by_day": by_day,
        "note": "people are counted by a salted hash of the Google subject id; this file holds no identity"
    })
}
