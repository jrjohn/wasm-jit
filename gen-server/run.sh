#!/bin/sh
# Launch the live-generation demo. The generator container needs a long-lived
# Claude Code OAuth token (claude setup-token) exported as CLAUDE_CODE_OAUTH_TOKEN.
# Pass a .env file holding it as $1, or export the var yourself beforehand.
set -e
ENV_FILE="${1:-$HOME/Documents/projects/aaf-designer-catalog/.env}"
if [ -z "$CLAUDE_CODE_OAUTH_TOKEN" ] && [ -f "$ENV_FILE" ]; then
  set -a; . "$ENV_FILE"; set +a
fi
cd "$(dirname "$0")/.."
exec cargo run --release -p gen-server
