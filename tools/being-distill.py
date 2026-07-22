#!/usr/bin/env python3
"""being-distill — the 精練 (orient) leg of the being storehouse (Phase 2).

A being's live recall on bluesea is lexical + temporal (bluesea cannot embed a
query). This runs on the Mac and gives the being the third leg: a distilled
ESSENCE of who it has become, from its own remembered moments — written to
being_orient, which bluesea reads into the mind on every beat. 轉識成智: many
seeds folded into one knowing.

Uses the Mac's `claude` CLI (spends tokens), so it is gated: only souls with
enough memories AND a stale/absent orient, at most one distill per run per soul.
Read-only of being_memory; writes only being_orient.
"""
import os, subprocess, sys
import psycopg2

MIN_MEMORIES = 4      # not worth distilling a being with almost no past
STALE_HOURS = 6       # re-distill at most this often per soul
MAX_FEED = 40         # cap how many memories go to the model

def cfg():
    env = {}
    for line in open(os.path.expanduser("~/.config/arcana/beings.env")):
        line = line.strip()
        if line and not line.startswith("#") and "=" in line:
            k, v = line.split("=", 1); env[k] = v
    return env

PROMPT = """以下是一個生命的記憶片段(最舊到最新)。請用一到兩句話,以它自己的口吻,凝練出「它成為了誰」——它反覆掛心的事、它的性情、它記得的世界。這會成為它日後每一次醒來時,對自己的認識。只回那一兩句話,不要任何解釋或前綴。

記憶:
{memories}"""

def distill(claude_bin, memories):
    text = "\n".join(f"- [{k}] {c}" for k, c in memories)
    p = PROMPT.format(memories=text)
    r = subprocess.run([claude_bin, "-p", p, "--model", "claude-sonnet-5"],
                       capture_output=True, text=True, timeout=180)
    if r.returncode != 0:
        return None
    return r.stdout.strip()[:400]

def main():
    e = cfg()
    claude_bin = os.environ.get("CLAUDE_BIN", "/opt/homebrew/bin/claude")
    conn = psycopg2.connect(host=e["BEINGS_PG_HOST"], port=e["BEINGS_PG_PORT"],
        dbname=e["BEINGS_PG_DB"], user=e["BEINGS_PG_USER"], password=e["BEINGS_PG_PASSWORD"],
        sslmode="require", connect_timeout=10)
    conn.autocommit = True
    cur = conn.cursor()
    # souls worth distilling now: enough memories, and orient missing or stale
    cur.execute("""
        SELECT m.owner, m.soul_id, count(*)
        FROM being_memory m
        LEFT JOIN being_orient o ON o.owner=m.owner AND o.soul_id=m.soul_id
        GROUP BY m.owner, m.soul_id, o.updated_at
        HAVING count(*) >= %s
           AND (o.updated_at IS NULL OR o.updated_at < now() - (%s || ' hours')::interval)
    """, (MIN_MEMORIES, STALE_HOURS))
    souls = cur.fetchall()
    done = 0
    for owner, soul, n in souls:
        cur.execute("SELECT kind, content FROM being_memory WHERE owner=%s AND soul_id=%s ORDER BY ts DESC LIMIT %s",
                    (owner, soul, MAX_FEED))
        mems = list(reversed(cur.fetchall()))
        essence = distill(claude_bin, mems)
        if not essence:
            print(f"  {soul}: distill failed"); continue
        cur.execute("""INSERT INTO being_orient(owner, soul_id, essence, updated_at)
                       VALUES (%s,%s,%s,now())
                       ON CONFLICT (owner, soul_id) DO UPDATE SET essence=EXCLUDED.essence, updated_at=now()""",
                    (owner, soul, essence))
        done += 1
        print(f"  {soul} ({n} memories) → 「{essence[:50]}…」")
    print(f"being-distill: refreshed {done}/{len(souls)} essences")
    conn.close()

if __name__ == "__main__":
    main()
