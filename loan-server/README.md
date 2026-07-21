# loan-server — one vocabulary, one fence, three deployments

A worked demonstration that wasm-jit's capability model is a property of **how a cell is
compiled and run**, not of *where* it runs or *whether anyone can read it*. The same loan
(mortgage) calculator — the same cells, the same import-table fence — is deployed three
ways, driven by the same inputs, computing identical numbers. What differs is only **where
the formula runs** and **whether a reader can recover it**.

Open **`/compare`** to see all three side by side.

---

## The vocabulary and the fence

The calculator is six capability-fenced UI cells written in the wasm-jit DSL, kept in one
place — [`apps/assets/loan_schema.json`](../apps/assets/loan_schema.json) — the single
source of truth every deployment reads:

| cell | what it does |
|---|---|
| `setP` / `setR` / `setY` | write the principal / rate / years into the host data root (`set(n, x)`) |
| `monthly` | amortization: `M = P·r(1+r)ⁿ / ((1+r)ⁿ−1)`, with `(1+r)ⁿ` raised by a bounded `while` loop; `set(3.0, M)` |
| `totalPaid` | `get(3.0) * get(2.0) * 12.0` |
| `interest` | `get(3.0) * get(2.0) * 12.0 - get(0.0)` |

**The fence** is the UI cell's entire import table — six host functions and nothing else:

```
env.sin  env.cos  env.get  env.set  env.ld  env.sd
```

A cell that reached for a socket, the filesystem, or `fetch` **could not compile** (the
codegen only knows these six) and **could not link** (the runtime — browser or wasmi —
offers only these six). This is the capability fence: the import table is the module's
entire world.

---

## The three deployments

| | where the formula runs | what crosses the wire | can a reader recover it? | route |
|---|---|---|---|---|
| **① 明碼 DSL** | browser (compiled in-page via `/pkg`) | the **DSL source** + a 288 KB compiler | trivially — view-source | `/compare` panel ①, uses `/pkg/*` |
| **② client wasm** | browser (`WebAssembly.instantiate`) | **770 bytes of precompiled `.wasm`** (no source, no compiler) | only by disassembly | `/compare` panel ②, `GET /api/wasm/{id}` |
| **③ server** | server (native `wasm-jit` + `wasmi`) | only `{p, r, y}` out, numbers back | no — the formula never leaves | `/`, `POST /api/loan` |

All three run their cells through the **same six-function fence** (JS-side for ① and ②, a
`wasmi` `Linker` for ③). They are equally *safe*. They differ only in *readability*:

```
明碼 (readable)  →  難讀 (obscured)  →  真藏 (hidden)
   ①  DSL             ②  wasm bytes        ③  server-side
```

### Why ② is "obscured", not "hidden"

Shipping the `.wasm` to the browser removes the DSL but does **not** hide the logic. wasm is
a compile target the browser executes, not an encryption — **downloaded means possessed, and
a possessed binary disassembles.** [`examples/reveal.rs`](examples/reveal.rs) proves it on
the actual 394-byte `monthly` cell: walking its code section straight out of the bytes, ten
`f64` ops spell out the whole amortization formula, and every constant decodes back to the
source —

```
1.0 · 1200.0 · 2.0 · 12.0 · 1e-7 · 3.0     (= get(1)/1200, get(2)*12, the r<1e-7 branch, set(3.0,…))
```

Only **not sending it** (deployment ③) actually hides logic. For a public formula like an
amortization schedule, none of this matters — it is a demonstration of the trade-off, not a
recommendation to hide public math.

### Safety is orthogonal to secrecy

The fence is a **compile-time** property. Whether the cell travels as DSL, as wasm bytes, or
stays on the server, the reach is the same closed set of six host functions — a cell cannot
gain `fetch` by being shipped in a different form. wasm-jit's "safety" means *the cell can't
harm the host*, **not** *the cell is secret*. ① and ② are readable **and** safe; ③ is hidden
**and** safe. Reading a cell and containing a cell are different axes.

---

## Verification (2026-07-21)

Run from the repo root so `./pkg` resolves for panel ①:

```
cargo run -p loan-server          # serves on :8787 (override with PORT=)
cargo test -p loan-server         # the fence + amortization tests
cargo run -p loan-server --example reveal   # disassemble the monthly cell
```

**Correctness — identical across all three deployments** (verified live in a headless browser):

| input | monthly | total | interest |
|---|---|---|---|
| 300 000 / 5 % / 30 y | **1 610.46** | 579 767.35 | 279 767.35 |
| 300 000 / 3 % / 30 y | **1 264.81** | 455 332.36 | 155 332.36 |

`/compare` shows "三欄一致 ✓" — the three panels agree to the cent, because it is one
vocabulary.

**Fence — machine-checked** (`cargo test`, 3 passed):

- `over_reaching_cell_is_rejected_at_compile` — a cell calling `mv()` (an entity capability
  the UI fence does not grant) **fails to compile**. The fence is a compile error, not a
  runtime check.
- `loan_cells_stay_within_fence` — each compiled cell's own import table, read back out of
  the bytes, is ⊆ the six-function fence.
- `amortization_is_correct` — the fenced `monthly` cell computes 1 610.46 for 300k/5%/30y.

**Routes** (verified responding):

```
GET  /                      server-side page (variant ③, formula hidden)  · 200
GET  /compare               the three deployments side by side            · 200, 12 603 B
POST /api/loan              {p,r,y} → {monthly,total,interest} + fence receipt
GET  /api/wasm/monthly      the precompiled cell bytes (variant ②)        · 200, 394 B, application/wasm
GET  /api/wasm/totalPaid    · 182 B      GET /api/wasm/interest · 194 B    (770 B total)
GET  /api/wasm/nope         · 404
GET  /api/source            proves contains_amortization_formula:false over the whole client (7 401 B)
GET  /pkg/wasm_jit.js       in-browser compiler for variant ① · 20 142 B  (+ wasm_jit_bg.wasm 287 553 B)
```

**Cell sizes**: `monthly` 394 B · `totalPaid` 182 B · `interest` 194 B. Each imports exactly
`env.{sin,cos,get,set,ld,sd}` (the codegen emits the full declared table; the strong fence
proof is the compile-time rejection above, not this per-cell list).

---

## Files

```
loan-server/
  src/main.rs          Axum server: native compile at boot (asserts each import ⊆ fence),
                       wasmi runtime for ③, /api/wasm bytes for ②, static /pkg for ①
  index.html           variant ③ — the server-side page (dumb client + round-trip inspector)
  compare.html         the three deployments side by side, one shared set of inputs
  examples/reveal.rs   disassembles the monthly cell — "shipping wasm obscures, not hides"
apps/assets/loan_schema.json   the single DSL source every deployment reads
```

Sibling: [`apps/form.html`](../apps/form.html) (deployed at `arcana.boo/apps/form.html`) is
the standalone version of variant ① — the same loan calculator, DSL in the page, compiled
in the browser.

---

*Commits (branch `worktree-canvas-poc`, PR #21): `62ccf42` loan-server · `5856317` reveal
example · `7a17812` /compare tri-panel.*
