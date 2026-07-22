//! The being's storehouse (阿賴耶), Phase 1 — complete, non-lossy memory in PG.
//!
//! The old journal was ≤12 lines and lossy "by law" — which quietly contradicted
//! 種子不失 (seeds are never lost). This is the storehouse done right: every 念 a
//! souled being has is kept as a row, and recall surfaces the relevant past by the
//! present situation, rather than dropping the oldest.
//!
//! It reuses the SAME engine the user's own archive rides on — PG + pg_jieba (and,
//! in Phase 2, pgvector + bge-m3) — but scoped to ONE soul. This is osearch at the
//! being's level: a consciousness querying its own storehouse by 緣. The user
//! recalls their sessions; a being recalls its lives; same mechanism, isolated corpus.
//!
//! ## Isolation — the key is (owner, soul), never soul alone
//!
//! `soul` is a name the creator picks, so it is a GLOBAL namespace: two different
//! people (or two unrelated worlds) could both name a being "weng" and would then
//! collide. So the real key is **(owner, soul)** — owner being the signed-in user.
//! Same person + same soul = one consciousness (this is transmigration, intended);
//! a different person, or a different soul, is a fully separate storehouse that can
//! neither be read nor written across the boundary. A crowd can log in and populate
//! a hundred worlds, and every being recalls exactly its own.
//!
//! ## The fence (why this is safe as a "capability")
//!
//! A cell NEVER gets a database, a query string, or `crs`. Everything here is
//! host-mediated: the being's mind receives recalled TEXT the host fetched, scoped
//! HARD to (owner, soul). The scope is not a query-discipline convention that could
//! be forgotten — it is enforced four ways:
//!   - tenant level: every statement is `WHERE owner = $1 AND soul_id = $2`, both
//!     bound values coming ONLY from the request's session + the being's own record,
//!     never from model output or free user input;
//!   - query level: the situation text is tokenised through `to_tsvector` before it
//!     touches the query, so it cannot inject or widen the scope;
//!   - role level: the `beings` role is not a superuser and has SELECT on nothing in
//!     the archive — a connection to archive_main is refused permission on every table;
//!   - database level: a separate DB (arcana_beings), a separate role.
//! A being cannot recall another being's memory, another person's being, or the
//! user's archive.
//!
//! Failure is always silent-and-safe: if the storehouse is unreachable, a being
//! simply remembers nothing this beat — a lost recall must never break a heartbeat.

use serde_json::{json, Value};
use tokio_postgres::NoTls;

fn url() -> Option<String> {
    std::env::var("BEINGS_PG_URL").ok().filter(|s| !s.trim().is_empty())
}

/// Configured? When not, beings fall back to their in-world journal (no PG).
pub fn enabled() -> bool {
    url().is_some()
}

/// One connection. localhost, NoTls (same host, trusted); the driver future is
/// spawned so the client is usable, and dropped with it after the call.
async fn conn() -> Result<tokio_postgres::Client, String> {
    let u = url().ok_or("BEINGS_PG_URL unset")?;
    let (client, connection) = tokio_postgres::connect(&u, NoTls)
        .await
        .map_err(|e| e.to_string())?;
    tokio::spawn(async move { let _ = connection.await; });
    Ok(client)
}

/// Embed a query through the Mac's bge-m3, reached over a reverse tunnel the Mac
/// holds open to this host (BEINGS_OLLAMA_URL, default localhost:11434). bluesea
/// cannot run the embedder itself (too heavy), so the being's own body borrows the
/// Mac's — exactly the host-lends-what-the-cell-cannot-have pattern, one level up.
/// None if the tunnel/model is down, in which case recall simply skips the semantic
/// leg and stays lexical — graceful, never fatal.
async fn embed_query(text: &str) -> Option<String> {
    let url = std::env::var("BEINGS_OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".into());
    let model = std::env::var("BEINGS_EMBED_MODEL").unwrap_or_else(|_| "bge-m3".into());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()
        .ok()?;
    let resp: Value = client
        .post(format!("{url}/api/embed"))
        .json(&serde_json::json!({ "model": model, "input": text }))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let v = resp["embeddings"].get(0)?.as_array()?;
    if v.len() != 1024 {
        return None;
    }
    // pgvector literal: '[f,f,...]'
    let mut s = String::from("[");
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("{:.6}", x.as_f64().unwrap_or(0.0)));
    }
    s.push(']');
    Some(s)
}

/// Semantic recall — the memories nearest in MEANING to the present situation, even
/// with no shared words (又冷又餓 → 飢寒交迫). Scoped to (owner, soul); only rows the
/// Mac has already embedded. [] if the query cannot be embedded.
async fn recall_semantic(owner: &str, soul: &str, situation: &str, limit: i64) -> Vec<Value> {
    if situation.trim().is_empty() {
        return vec![];
    }
    let Some(qvec) = embed_query(situation).await else { return vec![] };
    let Ok(c) = conn().await else { return vec![] };
    let sql = "SELECT kind, content, to_char(ts, 'YYYY-MM-DD') AS day \
               FROM being_memory \
               WHERE owner = $1 AND soul_id = $2 AND embedding IS NOT NULL \
               ORDER BY embedding <=> $3::vector LIMIT $4";
    match c.query(sql, &[&owner, &soul, &qvec, &limit]).await {
        Ok(rows) => rows
            .iter()
            .map(|r| {
                json!({
                    "day": r.get::<_, String>(2),
                    "kind": r.get::<_, String>(0),
                    "content": r.get::<_, String>(1),
                })
            })
            .collect(),
        Err(_) => vec![],
    }
}

/// Recall a soul's past. Fuses three legs the way osearch does: semantic (nearest
/// in meaning, if the query embeds), then lexical (shared jieba words), then recent.
/// `soul` is the ONLY scope; the situation is tokenised/embedded before it touches a
/// query, so it can never inject or widen. [] on any error.
pub async fn recall(owner: &str, soul: &str, situation: &str, limit: i64) -> Vec<Value> {
    // The semantic leg first — concept matches lexical cannot reach. Falls to [] if
    // the Mac tunnel is down, and the lexical leg below still carries recall.
    let semantic = recall_semantic(owner, soul, situation, (limit / 2).max(2)).await;

    let Ok(c) = conn().await else { return semantic };
    // The situation's jieba tokens, OR-joined, become the relevance query. Empty
    // situation → a token that matches nothing → pure recency order.
    // Build the OR relevance query robustly: jieba can emit an empty lexeme, and a
    // stray '' or an operator char in the OR string makes to_tsquery raise a syntax
    // error (which would silently degrade every recall to nothing). So filter to
    // clean tokens — no empties, none carrying tsquery operators — before OR-joining.
    let sql = "SELECT kind, content, to_char(ts, 'YYYY-MM-DD') AS day \
               FROM being_memory, \
                 (SELECT to_tsquery('jiebacfg', \
                    COALESCE(NULLIF(string_agg(lexeme, ' | '), ''), 'zzznomatch')) AS q \
                  FROM (SELECT DISTINCT lexeme FROM unnest(to_tsvector('jiebacfg', $3)) \
                        WHERE lexeme <> '' AND lexeme !~ '[ &|!():*]') w) s \
               WHERE owner = $1 AND soul_id = $2 \
               ORDER BY (content_tsv @@ s.q) DESC, ts_rank(content_tsv, s.q) DESC, ts DESC \
               LIMIT $4";
    let lexical: Vec<Value> = match c.query(sql, &[&owner, &soul, &situation, &limit]).await {
        Ok(rows) => rows
            .iter()
            .map(|r| {
                json!({
                    "day": r.get::<_, String>(2),
                    "kind": r.get::<_, String>(0),
                    "content": r.get::<_, String>(1),
                })
            })
            .collect(),
        Err(_) => vec![],
    };

    // Fuse: semantic first (concept matches), then lexical, deduped by content,
    // capped at the caller's limit. A memory that both legs surface appears once.
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for m in semantic.into_iter().chain(lexical.into_iter()) {
        let key = m["content"].as_str().unwrap_or("").to_string();
        if seen.insert(key) {
            out.push(m);
            if out.len() as i64 >= limit {
                break;
            }
        }
    }
    out
}

/// The distilled essence (orient) — who this soul is, refined. None until the
/// Mac-side distill pass (Phase 2) has run; a being without it just has no summary yet.
pub async fn orient(owner: &str, soul: &str) -> Option<String> {
    let c = conn().await.ok()?;
    let row = c
        .query_opt(
            "SELECT essence FROM being_orient WHERE owner = $1 AND soul_id = $2",
            &[&owner, &soul],
        )
        .await
        .ok()??;
    Some(row.get::<_, String>(0))
}

/// 現行熏種子 — keep this beat's 念s. One connection, several inserts; the embedding
/// stays NULL (the Mac fills it later). A failed write must never fail the heartbeat.
pub async fn ingest_many(owner: &str, soul: &str, items: &[(&str, &str)]) {
    let items: Vec<_> = items
        .iter()
        .filter(|(_, c)| !c.trim().is_empty())
        .collect();
    if items.is_empty() {
        return;
    }
    let Ok(c) = conn().await else { return };
    for (kind, content) in items {
        let trimmed: String = content.chars().take(400).collect();
        let _ = c
            .execute(
                "INSERT INTO being_memory(owner, soul_id, kind, content) VALUES ($1, $2, $3, $4)",
                &[&owner, &soul, kind, &trimmed],
            )
            .await;
    }
}
