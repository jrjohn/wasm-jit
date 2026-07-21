//! Who is asking — Google Sign-In, verified here, never taken on trust.
//!
//! Until now every write door on this server was anonymous: anyone who could
//! reach `/api/worlds` could write any JSON to disk under any name. That is fine
//! for a machine only its author can reach, and unacceptable the moment the
//! server is public. Identity is what makes the difference between "a stranger
//! may contribute" and "a stranger may overwrite".
//!
//! ## Why verify the token ourselves
//!
//! The browser hands us a Google ID token — a JWT signed by Google with RS256.
//! It arrives through the client, so the client could have written anything in
//! it. What makes it trustworthy is *only* the signature, so we check:
//!
//!   1. the signature, against Google's published keys (fetched, cached, keyed by `kid`)
//!   2. `aud` — the token was minted for OUR client id, not someone else's site
//!   3. `iss` — Google issued it
//!   4. `exp` — it has not expired (handled by the verifier)
//!
//! Skipping (2) is the classic mistake: a valid Google token from *any* other
//! application would otherwise be accepted here as proof of identity.
//!
//! ## What this deliberately is NOT
//!
//! No sessions, no cookies, no user table. The ID token IS the credential and it
//! is verified on every write. That costs one signature check per request and
//! removes an entire class of session bugs. Tokens last about an hour; the
//! browser silently renews them.

use serde::Deserialize;
use std::sync::Mutex;

/// Google's signing keys, cached. Fetched on first use and refreshed when a token
/// arrives bearing a `kid` we have not seen — that is exactly when Google rotates.
static JWKS: Mutex<Option<(std::time::Instant, serde_json::Value)>> = Mutex::new(None);

const JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const JWKS_TTL: std::time::Duration = std::time::Duration::from_secs(3600);

/// A verified human. `sub` is Google's stable id for them — it never changes and
/// is what we attribute work to; email/name are for display only and may change.
#[derive(Debug, Clone, serde::Serialize)]
pub struct User {
    pub sub: String,
    pub email: String,
    pub name: String,
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    name: String,
}

/// The client id this deployment accepts tokens for. Unset = local development:
/// writes stay open so the author's own machine keeps working unchanged, and we
/// say so loudly at boot rather than failing open in silence.
pub fn client_id() -> Option<String> {
    std::env::var("GOOGLE_CLIENT_ID").ok().filter(|s| !s.trim().is_empty())
}

pub fn is_open() -> bool {
    client_id().is_none()
}

async fn jwks(force: bool) -> Result<serde_json::Value, String> {
    if !force {
        if let Ok(g) = JWKS.lock() {
            if let Some((at, v)) = g.as_ref() {
                if at.elapsed() < JWKS_TTL {
                    return Ok(v.clone());
                }
            }
        }
    }
    let v: serde_json::Value = reqwest::get(JWKS_URL)
        .await
        .map_err(|e| format!("cannot reach Google's key endpoint: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Google's key endpoint returned something unreadable: {e}"))?;
    if let Ok(mut g) = JWKS.lock() {
        *g = Some((std::time::Instant::now(), v.clone()));
    }
    Ok(v)
}

/// Find the key Google says it signed this token with. A `kid` we do not hold is
/// the normal signal that keys rotated, so we refetch once before giving up.
async fn key_for(kid: &str) -> Result<jsonwebtoken::DecodingKey, String> {
    for force in [false, true] {
        let set = jwks(force).await?;
        if let Some(k) = set["keys"].as_array().and_then(|ks| {
            ks.iter().find(|k| k["kid"].as_str() == Some(kid))
        }) {
            let (n, e) = (
                k["n"].as_str().ok_or("key lacks n")?,
                k["e"].as_str().ok_or("key lacks e")?,
            );
            return jsonwebtoken::DecodingKey::from_rsa_components(n, e)
                .map_err(|e| format!("malformed Google key: {e}"));
        }
    }
    Err("Google has no signing key with that id — the token is not one of theirs".into())
}

/// Verify a Google ID token and return who it belongs to.
pub async fn verify(token: &str) -> Result<User, String> {
    let aud = client_id().ok_or("this server accepts no logins (GOOGLE_CLIENT_ID unset)")?;
    let header = jsonwebtoken::decode_header(token).map_err(|e| format!("not a JWT: {e}"))?;
    let kid = header.kid.ok_or("token names no signing key")?;
    let key = key_for(&kid).await?;

    let mut val = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    // (2) and (3): the token must have been minted FOR US, BY Google.
    val.set_audience(&[aud]);
    val.set_issuer(&["accounts.google.com", "https://accounts.google.com"]);

    let data = jsonwebtoken::decode::<Claims>(token, &key, &val)
        .map_err(|e| format!("token rejected: {e}"))?;
    let c = data.claims;
    if c.sub.is_empty() {
        return Err("token carries no subject".into());
    }
    Ok(User {
        name: if c.name.is_empty() { c.email.clone() } else { c.name.clone() },
        email: c.email,
        sub: c.sub,
    })
}

/// Pull the bearer token out of the request and verify it.
///
/// When no client id is configured this is local development, and we return an
/// obviously-fake local user rather than blocking the author's own workflow.
pub async fn user_from(headers: &axum::http::HeaderMap) -> Result<User, String> {
    if is_open() {
        return Ok(User {
            sub: "local".into(),
            email: "local@localhost".into(),
            name: "本機".into(),
        });
    }
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or("請先用 Google 登入(缺 Authorization: Bearer <id_token>)")?;
    verify(raw.trim()).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Staying signed in.
//
// The first version kept the Google ID token in a JavaScript variable and leaned
// on `auto_select` to sign a returning visitor back in silently. That reasoning
// was sound about storage and wrong about reality: auto_select is conditional on
// third-party cookie policy, a dismissal cool-down, and there being exactly one
// Google session — so in practice people came back to a login button every time.
//
// The fix is not to park the Google token in localStorage, which really would
// hand a live credential to anything that can run script on the page. It is for
// the server to mint its OWN session after verifying Google once, and hand it
// back as an HttpOnly cookie: unreadable from JavaScript, carried automatically
// on every request, and expiring on a schedule we choose rather than Google's.
// ─────────────────────────────────────────────────────────────────────────────

const SESSION_DAYS: u64 = 30;
const COOKIE: &str = "arcana_session";

/// The key this server signs its own sessions with. Generated on first use and
/// kept beside the app; rotating it (or losing it) signs everybody out, which is
/// the correct blast radius for a file like this.
/// The key this server signs its own sessions with.
///
/// Cached in memory as well as on disk, and that is not an optimisation — the
/// first version read the file, and on failure generated a fresh key and
/// swallowed the write error with `let _ =`. The service account could not
/// write into its root-owned working directory, so EVERY call minted a
/// different key: cookies were signed with one and verified against another,
/// and no session could ever be valid. Nothing errored; sign-in simply never
/// stuck.
///
/// So: one key per process at most, and a loud complaint if it cannot be
/// persisted, because "sessions silently reset on restart" should not be
/// something a reader has to discover from behaviour.
fn session_key() -> Vec<u8> {
    static CACHE: Mutex<Option<Vec<u8>>> = Mutex::new(None);
    if let Ok(g) = CACHE.lock() {
        if let Some(k) = g.as_ref() {
            return k.clone();
        }
    }
    let path = std::env::var("SESSION_KEY_FILE").unwrap_or_else(|_| ".session-key".into());
    let key = match std::fs::read(&path) {
        Ok(k) if k.len() >= 32 => k,
        _ => {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let mut k = Vec::new();
            let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ (seed as u64);
            for _ in 0..6 {
                h = h.wrapping_mul(0x100_0000_01b3) ^ (h >> 29);
                k.extend_from_slice(&h.to_le_bytes());
            }
            if let Err(e) = std::fs::write(&path, &k) {
                eprintln!(
                    "[auth] WARNING: cannot persist the session key to {path}: {e}\n\
                     [auth] sessions will work but every restart signs everyone out. \
                     Point SESSION_KEY_FILE at a directory this service can write."
                );
            }
            k
        }
    };
    if let Ok(mut g) = CACHE.lock() {
        *g = Some(key.clone());
    }
    key
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Session {
    sub: String,
    email: String,
    name: String,
    exp: u64,
}

/// Exchange a verified Google identity for our own session cookie.
pub fn issue_cookie(u: &User) -> String {
    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        + SESSION_DAYS * 86_400;
    let claims = Session {
        sub: u.sub.clone(), email: u.email.clone(), name: u.name.clone(), exp,
    };
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(&session_key()),
    )
    .unwrap_or_default();
    // HttpOnly: script cannot read it, so an injected script cannot steal it.
    // Secure + SameSite=Lax: not sent over plain http, not sent on cross-site
    // POSTs — the shape that makes a cookie a session rather than a liability.
    format!(
        "{COOKIE}={token}; Path=/; Max-Age={}; HttpOnly; Secure; SameSite=Lax",
        SESSION_DAYS * 86_400
    )
}

pub fn clear_cookie() -> String {
    format!("{COOKIE}=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax")
}

fn session_from_cookies(raw: &str) -> Option<User> {
    let token = raw
        .split(';')
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == COOKIE)
        .map(|(_, v)| v)?;
    let mut val = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    val.set_required_spec_claims::<&str>(&[]);   // our own claims, not an OIDC token
    val.validate_aud = false;
    let data = jsonwebtoken::decode::<Session>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(&session_key()),
        &val,
    )
    .ok()?;
    Some(User { sub: data.claims.sub, email: data.claims.email, name: data.claims.name })
}

/// Identity for a request: our own session cookie first (the common case for a
/// returning visitor), then a Google bearer token (the moment of signing in).
pub async fn user_from_any(headers: &axum::http::HeaderMap) -> Result<User, String> {
    if is_open() {
        return Ok(User { sub: "local".into(), email: "local@localhost".into(), name: "本機".into() });
    }
    if let Some(raw) = headers.get(axum::http::header::COOKIE).and_then(|v| v.to_str().ok()) {
        if let Some(u) = session_from_cookies(raw) {
            return Ok(u);
        }
    }
    user_from(headers).await
}
