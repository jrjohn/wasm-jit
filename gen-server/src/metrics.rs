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
//! "how many distinct people" is answerable without the event log or the public
//! count holding anyone's identity. The salt lives beside the log and never leaves
//! the host; lose it and the old rows become permanently anonymous, which is the
//! correct direction for a file to fail in.
//!
//! The one exception, added by the operator's explicit choice: a SEPARATE private
//! directory (`people-identity.json`) maps that same hash to the email/name behind
//! it — a "who to thank" list for contact and, later, per-user quota. It is kept
//! deliberately apart from the hash-only event log and roster, and is **never served
//! by any endpoint** — it exists only on the host. The public numbers stay identity-
//! free; the private directory is a contact list, not part of the falsifiable count.
//!
//! Be clear about what this does NOT make private: a world records its author's
//! display name (that is the point of attribution), and the ālaya ledger stores
//! every ask verbatim, keyed by its cause. Prompts are retained — by the ledger,
//! deliberately, because replaying a cause is a feature. This log keeps only the
//! length and a hash, but it would be dishonest to present that as prompt
//! privacy when the ledger sits next to it.

use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::{Mutex, OnceLock};

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

/// The all-time roster of distinct people — one salted user_key per line, first-seen,
/// append-only. stats() also counts distinct people, but only over the days it still
/// keeps: prune the old files and that number falls. This roster never goes backward —
/// it is the durable answer to "how many people have ever built here". It holds only
/// the same hashes the log does, so it is exactly as private (no identity, ever).
///
/// Loaded once, lazily. On first load it also ABSORBS anyone the daily event files
/// remember but the roster doesn't yet — so shipping this does not reset the count to
/// zero (the people already in the logs are kept), and a deleted roster re-heals from
/// whatever events remain. Genuinely new keys are appended so the file stays complete.
fn roster() -> &'static Mutex<HashSet<String>> {
    static ROSTER: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    ROSTER.get_or_init(|| {
        let path = format!("{DIR}/people.log");
        let mut set: HashSet<String> = HashSet::new();
        if let Ok(txt) = std::fs::read_to_string(&path) {
            for line in txt.lines() {
                let k = line.trim();
                if !k.is_empty() {
                    set.insert(k.to_string());
                }
            }
        }
        // Backfill from the event log — every distinct non-anon user any daily file
        // still holds. New keys are appended to the roster so it becomes authoritative.
        let mut files: Vec<_> = std::fs::read_dir(DIR)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("events-"))
            .collect();
        files.sort_by_key(|e| e.file_name());
        let mut fresh: Vec<String> = Vec::new();
        for e in &files {
            if let Ok(txt) = std::fs::read_to_string(e.path()) {
                for line in txt.lines() {
                    if let Ok(v) = serde_json::from_str::<Value>(line) {
                        let u = v["user"].as_str().unwrap_or("anon");
                        if u != "anon" && set.insert(u.to_string()) {
                            fresh.push(u.to_string());
                        }
                    }
                }
            }
        }
        if !fresh.is_empty() {
            let _ = std::fs::create_dir_all(DIR);
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
                for k in &fresh {
                    let _ = writeln!(f, "{k}");
                }
            }
        }
        Mutex::new(set)
    })
}

/// Note a person into the all-time roster. Idempotent and cheap: the usual case is a
/// key already present (a lock and a set lookup); only a genuinely new person costs an
/// append. Called from record() for every authenticated event — anon is ignored.
fn note_person(user: &str) {
    if user == "anon" || user.is_empty() {
        return;
    }
    if let Ok(mut set) = roster().lock() {
        if set.insert(user.to_string()) {
            let _ = std::fs::create_dir_all(DIR);
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(format!("{DIR}/people.log"))
            {
                let _ = writeln!(f, "{user}");
            }
        }
    }
}

/// The durable all-time count of distinct people who have signed in — never decreases.
pub fn people_all_time() -> usize {
    roster().lock().map(|s| s.len()).unwrap_or(0)
}

/// The operator's private "who to thank" directory — the same salted user_key the
/// public count uses, mapped to the identity behind it (email, display name, the raw
/// Google sub, first/last seen). DELIBERATELY separate from the event log and roster,
/// which stay hash-only: those are the public, falsifiable count; this is a private
/// contact list the operator chose to keep. It is never served by any endpoint — read
/// it on the host, or not at all.
fn identities() -> &'static Mutex<HashMap<String, Value>> {
    static IDS: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    IDS.get_or_init(|| {
        let map = std::fs::read_to_string(format!("{DIR}/people-identity.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<HashMap<String, Value>>(&s).ok())
            .unwrap_or_default();
        Mutex::new(map)
    })
}

/// Record who a signed-in person is, keyed by the same salted user_key the count uses
/// so the private directory and the public number line up. Called from the auth-bearing
/// handlers, where the full identity is in hand. Cheap: it rewrites the file only when
/// something actually changed — a new person, a changed email/name, or the first sighting
/// of a new day — so after first contact it is at most one small write per person per day.
pub fn note_identity(sub: &str, email: &str, name: &str) {
    if sub.is_empty() {
        return;
    }
    let key = user_key(sub);
    let today = today();
    if let Ok(mut m) = identities().lock() {
        let changed = match m.get(&key) {
            Some(v) => {
                v["email"].as_str() != Some(email)
                    || v["name"].as_str() != Some(name)
                    || v["last_seen"].as_str() != Some(today.as_str())
            }
            None => true,
        };
        if !changed {
            return;
        }
        let first = m
            .get(&key)
            .and_then(|v| v["first_seen"].as_str().map(String::from))
            .unwrap_or_else(|| today.clone());
        m.insert(
            key,
            json!({ "email": email, "name": name, "sub": sub, "first_seen": first, "last_seen": today }),
        );
        let _ = std::fs::create_dir_all(DIR);
        if let Ok(s) = serde_json::to_string_pretty(&*m) {
            let _ = std::fs::write(format!("{DIR}/people-identity.json"), s);
        }
    }
}

/// Append one event. Never fails a request: if the log cannot be written the
/// request still completes, because losing a metric is not worth losing a world.
pub fn record(kind: &str, user: &str, mut fields: Value) {
    note_person(user); // an authenticated event means a distinct person, all-time
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
        // Durable, all-time distinct people — does not fall when old daily files are
        // pruned the way "people" (windowed to the kept days) does. This is the number
        // the homepage shows: "N people have built a world here".
        "people_all_time": people_all_time(),
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
