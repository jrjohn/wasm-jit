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
