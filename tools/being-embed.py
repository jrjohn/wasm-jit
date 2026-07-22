#!/usr/bin/env python3
"""being-embed — the semantic leg of the being storehouse (Phase 2).

bluesea cannot run the embedder (proven too heavy), so it writes being_memory rows
with embedding NULL; this runs on the Mac (which has Ollama + bge-m3), embeds the
NULL rows, and writes the 1024-d vectors back. Same topology as the archive: the
Mac embeds, the shared PG stores. Read-only of everything except the embedding it
fills. Idempotent — only touches NULL rows, safe to run every few minutes.
"""
import os, json, sys, urllib.request
import psycopg2

def cfg():
    env = {}
    p = os.path.expanduser("~/.config/arcana/beings.env")
    for line in open(p):
        line = line.strip()
        if line and not line.startswith("#") and "=" in line:
            k, v = line.split("=", 1); env[k] = v
    return env

def embed(url, model, text):
    req = urllib.request.Request(f"{url}/api/embed",
        data=json.dumps({"model": model, "input": text}).encode(),
        headers={"content-type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.load(r)["embeddings"][0]

def main():
    e = cfg()
    conn = psycopg2.connect(host=e["BEINGS_PG_HOST"], port=e["BEINGS_PG_PORT"],
        dbname=e["BEINGS_PG_DB"], user=e["BEINGS_PG_USER"], password=e["BEINGS_PG_PASSWORD"],
        sslmode="require", connect_timeout=10)
    conn.autocommit = True
    cur = conn.cursor()
    batch = int(sys.argv[1]) if len(sys.argv) > 1 else 200
    cur.execute("SELECT id, content FROM being_memory WHERE embedding IS NULL ORDER BY id LIMIT %s", (batch,))
    rows = cur.fetchall()
    done = 0
    for rid, content in rows:
        try:
            v = embed(e["OLLAMA_URL"], e["EMBED_MODEL"], content)
            if len(v) != 1024:
                print(f"  skip id={rid}: got {len(v)} dims"); continue
            lit = "[" + ",".join(f"{x:.6f}" for x in v) + "]"
            cur.execute("UPDATE being_memory SET embedding = %s::vector WHERE id = %s", (lit, rid))
            done += 1
        except Exception as ex:
            print(f"  id={rid} failed: {str(ex)[:80]}")
    print(f"being-embed: embedded {done}/{len(rows)} rows (of NULL backlog)")
    conn.close()

if __name__ == "__main__":
    main()
