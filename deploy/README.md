# Deploying the world engine to arcana.boo

What runs in public, and the two decisions that shaped it.

## Layout

    /opt/arcana/app/            working directory (systemd WorkingDirectory)
      world-engine              the binary (aarch64, built on the host in a rust container)
      gen-server/live-gen.html  the page — read relative to the working directory
      worlds/ ledger/           what visitors create; the only writable paths
      skins-grown/ widgets-grown/
      pkg/ pkg-skins/ pkg-dust/ wasm the page loads
    /etc/arcana/world.env       GOOGLE_CLIENT_ID + CLAUDE_CODE_OAUTH_TOKEN, mode 0600
    /etc/systemd/system/arcana-world.service
    /data/www/home/nginx-home.locations   the `location ^~ /world/` block

## Why /world/ and not /api/

This vhost's `/api/` already proxies to :8088, and `location /` is behind
Authelia. The engine gets its own prefix, and `live-gen.html` asks for its API
with **relative** paths — so the same file works at `http://localhost:8646/`
and at `https://arcana.boo/world/` with no build step and no injected base tag.

## Rebuilding

    tar czf /tmp/src.tgz --exclude=target --exclude=.git .
    scp /tmp/src.tgz bluesea:/tmp/
    ssh bluesea 'cd /opt/arcana/build && tar xzf /tmp/src.tgz &&
      docker run --rm -v /opt/arcana/build:/w -w /w rust:1-slim-bookworm \
        cargo build --release -p gen-server &&
      sudo install -m 0755 target/release/gen-server /opt/arcana/app/world-engine &&
      sudo install -m 0644 gen-server/live-gen.html /opt/arcana/app/gen-server/ &&
      sudo systemctl restart arcana-world'

The host has no Rust toolchain; it has Docker and is the same architecture as
the target, so building in a container beats cross-compiling.

## Two things a reader should not have to discover

**SELinux.** Rocky denies nginx outbound connections by default, so the proxy
returns 502 with nothing obviously wrong in either config:

    sudo setsebool -P httpd_can_network_connect 1

**Generation is deliberately off.** `CLAUDE_CODE_OAUTH_TOKEN` in `world.env` is
a placeholder. Everything that costs nothing is live — compose a world from the
existing vocabulary, save it, load anyone else's, contribute a skin or widget.
Only "speak a new world into being" is dark, because it spends the operator's
model quota and there is **no quota mechanism yet**: sign-in makes that spend
attributable and revocable, which is not the same as bounded. Turning it on
means putting a live account credential on a public-facing host. That is a
decision for whoever owns the account, not a deployment detail.

    # when that decision is made:
    sudo sed -i 's|^CLAUDE_CODE_OAUTH_TOKEN=.*|CLAUDE_CODE_OAUTH_TOKEN=<real>|' /etc/arcana/world.env
    sudo systemctl restart arcana-world

## Rolling back

    sudo systemctl disable --now arcana-world
    sudo cp /data/www/home/nginx-home.locations.bak.<timestamp> /data/www/home/nginx-home.locations
    sudo nginx -t && sudo systemctl reload nginx

The static site is untouched by all of this: `/`, `/apps/*` and every existing
path keep their previous behaviour, because the engine only ever claimed a
prefix nothing else was using.

## The trap that cost the most time here

`sudo` inside the unit failed with *"effective uid is not 0"* while
`NoNewPrivileges=no` sat in the file looking effective. It was not: each of

    PrivateDevices · ProtectKernelTunables · ProtectKernelModules
    ProtectControlGroups · RestrictSUIDSGID · RestrictNamespaces · LockPersonality

**implies `NoNewPrivileges=yes`**, and `NoNewPrivileges` disables setuid — which
is how sudo elevates. So the hardening silently disabled the docker policy, and
the symptom surfaced three layers away as `generator container unavailable`.

Check what is actually in force, not what the file says:

    systemctl show arcana-world -p NoNewPrivileges --value

Those directives were dropped rather than the sudo policy, because they are the
ones that matter least here: they mostly protect against a *privileged* service,
and this one runs as an unprivileged account that lacks those capabilities
anyway. The docker socket, by contrast, really is root — so a policy allowing
exactly two docker subcommands is worth more than kernel hardening this account
could never have used.

## The being storehouse (阿賴耶, Phase 1) — PG-backed memory

Souled beings recall their own past from PostgreSQL instead of a lossy 12-line
journal. It reuses the engine the user's own archive rides on — PG + pg_jieba —
scoped HARD to (owner, soul).

- **DB**: `arcana_beings` in the existing `pg-archive-test` container (pgvector +
  pg_jieba). Role `beings` (not superuser); tables `being_memory`, `being_orient`.
  Connection in `world.env` as `BEINGS_PG_URL` (localhost, NoTls). The `beings`
  role has SELECT on nothing in `archive_main` — a connection there is refused
  permission on every table (verified).
- **Isolation**: the key is `(owner, soul)`, never soul alone. owner is the
  signed-in user (a salted hash — the beings DB holds no raw identity). Two people
  who both name a being "weng" reach different storehouses; a being can recall its
  own memory only. Enforced at tenant (WHERE owner=$1 AND soul=$2, both host-set),
  role, and database levels. Verified end-to-end: two same-named souls stayed
  separate through the real mind API.
- **Recall** = temporal (recent) + lexical (jieba OR over the present situation).
  The semantic leg (bge-m3 embeddings) is Phase 2, on the Mac mini (bluesea cannot
  run the embedder — proven too heavy). Rows carry `embedding NULL` until then.

### Two infra traps hit while wiring this (both about sudo under the sandbox)

The generator reaches its container via `sudo` (narrow policy, no docker group).
The hardened unit broke that twice:

1. `ProtectSystem=strict` makes /var and /run read-only, so sudo/PAM cannot write
   its timestamp — it worked only while an old timestamp was still valid, then
   failed with "cannot open /run/sudo". Fixed: `ProtectSystem=full` (still protects
   /usr, /boot, /etc) + `/etc/sudoers.d/arcana-docker-defaults`:
   `Defaults:arcana !requiretty, !authenticate` (arcana still runs ONLY the two
   whitelisted docker commands, just without a tty or password).
   NB: systemd does NOT accept an inline `# comment` on a directive line — it
   silently voids the value. Put comments on their own line.

2. **The generator credential is a static copy that expires.** `/opt/arcana/
   claude-home/.credentials.json` was copied from the CI agent; OAuth tokens age
   out (401), and a static copy is never refreshed, so generation dies after a
   day or two. Stop-gap: re-copy from `/data/projects/daily-ci-agent/claude-home/`
   (which the CI agent keeps fresh) and `docker restart wasmjit-gen`. Durable fix
   is a decision for the account owner — a long-lived token, or a shared+refreshed
   credential mount (accepting the write race). This is the same "who pays for
   generation" question flagged at deploy.

## The semantic leg (Phase 2) — the Mac mini embeds, over a reverse tunnel

bluesea cannot run the embedder (proven too heavy), and the always-on **Mac mini**
already has Ollama + bge-m3 for the archive. So the being's semantic recall borrows
the mini's body, one level up from how the host lends a cell what it cannot have:

- **Row embedding** — `~/bin/arcana/being-embed.py` on the mini (launchd
  `com.jrjohn.arcana-being-embed`, every 600s) reads being_memory rows with
  embedding NULL from arcana_beings (over TLS), embeds via the mini's bge-m3,
  writes the 1024-d vectors back. Config in `~/.config/arcana/beings.env` (0600).
- **Query embedding** — a reverse SSH tunnel the mini holds open to bluesea
  (launchd `com.jrjohn.arcana-being-tunnel`, KeepAlive) exposes the mini's Ollama
  on bluesea's `127.0.0.1:11434`. gen-server (`BEINGS_OLLAMA_URL`) embeds the
  present situation through it at recall time, then KNN (`embedding <=> query`)
  over being_memory scoped to (owner, soul). Proven: "又冷又餓" finds "飢寒交迫"
  at cosine 0.30 — a concept match with zero shared words, which jieba cannot do.
- **Fusion** — recall returns semantic-nearest first, then lexical (jieba OR),
  then recent, deduped. If the tunnel/mini is down, embed_query returns None and
  recall silently falls back to lexical — graceful, never a broken beat.
- **Mini → bluesea access** — the mini's own ed25519 key is authorized for
  `rocky@arcana.boo` (no private key copied); it reaches arcana_beings:5432 the
  same way pgsearchd does. Reproduce the tunnel:
  `ssh -N -R 127.0.0.1:11434:127.0.0.1:11434 rocky@arcana.boo`.
- **Distill (orient / 精練)** — `being-distill.py` folds a soul's memories into an
  essence via the mini's `claude` CLI, written to being_orient, read by gen-server
  into the mind. Present but not yet scheduled (the CLI was slow on the mini —
  timeout raised to 180s); query-time semantic recall covers most of its value.
