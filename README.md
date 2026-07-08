# wasm-jit — scripts as seeds: runtime-generated WASM cells (borrow the browser's JIT) to run *code you don't have to trust*

**Core claim**: compile a script to WASM bytes at runtime → `WebAssembly.instantiate()` → the browser engine JITs it → **near-native speed + a capability sandbox + synchronous calls**, the only path where all three hold at once. Measured on par with the AOT ceiling and tied with hand-written JS on speed — but it buys a property JS can't give: **the manifested code need not be trusted** (the import table *is* its entire world).

> **Origin & acknowledgment**: this idea began from wanting a *sandboxed* runtime scripting language. We first prototyped against [Rhai](https://github.com/rhaiscript/rhai) and found that a tree-walking interpreter trades native speed for its sandbox — wasm-jit wants both. **Thank you to Rhai for the spark**; it shaped the whole direction. The project no longer contains Rhai (the comparison framing is gone too), returning to its own thesis: native speed *inside* a sandbox.

Theory: `docs/multidimensional-composition-architecture.md` (§16 execution layer, §17 the AI-era frontend).

## PoC overview

**Standalone pages:**
1. **`index.html` — benchmark**: one script run three ways (generated WASM / JS / AOT Rust); shows generated WASM = AOT ceiling and tied with JS.
2. **`canvas.html` — canvas**: 2000 components, each with its own independently generated WASM kernel cell, all run every frame at 60fps.
3. **`draw.html` — freeform draw**: 7 drawing primitives; a smiling Buddha / a full-body Guanyin on a lotus throne, manifested from DSL seeds (`examples/*.dsl`).

**`leptos-poc/` — pure-Rust CSR (Leptos 0.8), seven tabs** (zero hand-written JS, paired with `api-server`):
| Tab | What it proves |
|---|---|
| DynamicCell | behavior is dynamic: live-edit a script → cell drives a signal → reactive DOM update |
| Form | 9 widget kinds driven by a schema (server reads the JSON from disk per request — edit + reload to change it); validation/computed fields = DSL cells; departments/members via a real Axum API |
| Tokens | style as capability: SCSS emits `--tk-*` rails; a style spec may only reference tokens, raw CSS is rejected |
| Layout | layout as schema: the whole app shell (header / menu / profile / table) is manifested from a recursive JSON tree — the table's data source is data too |
| Freeform draw | pixel surface: Buddha/Guanyin read from DSL seeds; **+ "Buddha — AssemblyScript" = a real 637-byte asc-compiled seed running through the same import audit** |
| **3D voxel** | **a playable Minecraft-style world**: ←→ turn, ↑↓ walk, Space jump; true perspective + chase camera + infinite terrain + distance fog, **the renderer and physics all live in a ~2.4KB seed**; interaction via `key`/`get`/`set` (state itself is a granted capability) |
| Seed-language spectrum | **the fence is language-agnostic**: Tier 1 (DSL codegen) vs Tier 2 (external WASM via `Cell::from_wasm_bytes`'s import audit) — same ABI, bit-identical value; an over-reaching external seed (extra `env::fetch` import) is rejected |
| **LiveUI** | **the live-manifestation loop** (docs §18, implemented): one schema declares cells + widget tree + patches + wires; events run cells, outputs cascade along a **budgeted event bus** (a wiring cycle degrades into a report, not a hang), and a cell's verdict **gates structural patches** (vocabulary-validated before touching the tree). Every cell is **fuel-metered** and **supervised**: an injected runaway loop traps → degrades → quarantines → restartable — the page never freezes. Plus the **memory capability** (host writes an array into the cell's own exported memory, the cell reduces it via `load()`, cross-checked bit-exact) and live module-cache stats |

**Three surfaces, three complete vocabularies**: pixels (7 primitives), forms (9 widgets), layout (9 layout cells) — generation never creates vocabulary, it only composes it.

**Seed-language spectrum** (§16): the fence is in the import table, not the grammar — so one sandbox holds many seed languages:
| Tier | Language | Fence entry | Proof |
|---|---|---|---|
| 1 | home DSL (f64-scalar: let/while/**if-else**/arithmetic+%/comparison + built-in min/max/abs/sqrt/floor) | codegen rejects ungranted functions | every DSL seed |
| 2 | **AssemblyScript** (TS syntax) / Rust→wasm / hand-written WAT | `Cell::from_wasm_bytes` audits import section ⊆ grants before instantiate | `assembly/buddha.ts` compiled by asc, drawn in the freeform-draw tab |

## The power of wasm-jit (vs JS, honest version)

Speed: on a pure f64 kernel it ties JS (V8's home turf). Ergonomics: JS still wins slightly (the DSL added if/%/built-in math but still has no functions/arrays/strings). The power is in five properties JS structurally cannot give:

| Property | JS `new Function` | wasm-jit cell |
|---|---|---|
| boundary of its world | ambient authority (whole page: fetch/document/cookies) | **the import table = the whole world** (the 3D game grants 12 capabilities; `fetch()` rejected at compile time) |
| memory | any closure / global | even state is granted (get/set, 32 slots) |
| determinism | by discipline | by construction: same input → bit-identical output → replayable/auditable |
| escape | prototype pollution + a history of sandbox escapes | memory isolation is a VM guarantee |
| cold code / frame time | must warm the JIT; GC/deopt tail | instantiate → near-native; zero allocation inside the cell |

The three ways to get isolation in JS each miss a corner: iframe (async), a sandboxable interpreter (one-to-two orders of magnitude slower), SES (immature). **"fast + isolated + synchronous" — only runtime-generated WASM gets all three.** One more layer: **grammar as fence** — the DSL can only express f64 math + granted calls, so "what this code can touch" is a compile-time-enumerable list. **JS runs code you trust; wasm-jit runs code you don't have to trust** — AI-generated / user-pasted / schema-carried manifestations can be *allowed* to come alive precisely because they live inside cells.

```
src (textarea, a small f64 seed language)
  → parser (Rust, recursive descent) → AST
      ├→ wasm-encoder → ~hundred-byte .wasm module → WebAssembly.instantiate() (engine JIT)
      ├→ same AST transpiled to JS → new Function (engine JS-JIT reference)
      └→ same kernel hand-written in Rust, AOT-compiled into the main module (ceiling reference)
```

Same source, three execution lanes, bit-identical values (identical f64 op order).

## Measured (2026-07-07, Apple Silicon Mac, Chrome headless=new)

Default kernel: `sum = sum + i*i - sum/(i+1)`, looped N times. exec = adaptive inner loop + median of samples.

| N | **generated WASM (engine JIT)** | JS `new Function` | AOT Rust (ceiling) | WASM vs AOT |
|---|---|---|---|---|
| 1e4 | **0.033 ms** | 0.055 ms | 0.033 ms | **1.0×** |
| 1e6 | **3.27 ms** | 3.33 ms | 3.27 ms | **1.0×** |
| 1e7 | **32.6 ms** | 32.3 ms | 31.2 ms | **1.04×** |

Compile cost (one-off): codegen 0.4–2.2 ms + instantiate ~0.6 ms; generated module **~117 bytes**.

**Conclusions:**
1. **Generated WASM = AOT ceiling (≈1.0×)** — the engine gives this numeric kernel a full-speed JIT; "borrowing the engine's JIT" has zero overhead.
2. **Tied with hand-written JS** (a pure f64 kernel is V8's JS-JIT home turf) — so the value of this path **is not speed**, it's: ① the capability sandbox (a generated module can only touch its imports + its own memory; `fetch()` rejected at compile time); ② deterministic replay (bit-identical values); ③ no GC, predictable frame time.
3. Compile cost ~1–3 ms, amortized once; only hot paths (called repeatedly) are worth compiling, run-once code can use other means → tiering: don't compile cold code, compile hot code to WASM.

## Canvas PoC — measured (canvas.html, 2026-07-07, same machine, headless Chrome)

Each component gets its **own unique script source** (4 templates × constants baked from the index, simulating "AI generates a behavior per component"), each compiled to its own WASM cell (**capability imports only `sin`/`cos`/`out` — the import table is the capability list**); kernel weight = number of while substeps. Both modes eat the same batch of scripts.

| Config | **generated WASM cells** | JS new Function |
|---|---|---|
| N=500 × 200 substeps (100k iters/frame) | **60fps · 1.17 ms · 7% of frame budget** | 60fps · 0.84 ms · 5% |
| N=2000 × 1000 (20× load, 2M iters/frame) | **60fps · 4.8 ms · 29%** (2000 cells / 78ms compile / 613KB) | 60fps · 4.4 ms · 27% |

**Canvas conclusions:**
1. **"every component is an agent with generated behavior" is routine engineering on WASM cells** (2000 components at 20× load still 60fps, 29% budget) — the live proof of the §16 "feasibility switch"; 2000 unique modules compile in 78ms total (~0.04ms each), so "AI generates code per component and compiles it on the spot" costs next to nothing.
2. JS ties on speed as always — the reason to choose WASM is the capability sandbox (each cell can only sin/cos/out, can't even call fetch — codegen rejects ungranted capabilities), not speed.

## Running it

```bash
# needs: rustup target add wasm32-unknown-unknown + wasm-pack + trunk

# A) standalone pages (benchmark / canvas / draw)
wasm-pack build --target web --release
python3 -m http.server 8642              # open http://localhost:8642/{index,canvas,draw}.html

# B) leptos-poc seven tabs + Rust API (Form/Layout/Draw/3D/Spectrum need it)
cd assemblyscript && npm install && npm run build && cd ..   # Tier 2: asc compiles the Buddha seed (optional)
cd leptos-poc && trunk build --release && cd ..
cargo run --release -p api-server -- leptos-poc/dist
# → http://127.0.0.1:8645 (same-origin: serves dist + /api/*; edit a schema/seed file on disk and reload — zero rebuild)

cargo test                          # native tests
cargo run --example audit_as --no-default-features   # dogfood: audit a real asc output with our own tool
```

**Files you can play with (edit + reload, no Rust)**: `api-server/form-schema.json` (form), `api-server/layout-schema.json` (layout), `examples/*.dsl` (Buddha / Guanyin / isometric voxel / third-person voxel), `assemblyscript/assembly/buddha.ts` (Tier 2 AS seed, active after `npm run build`).

## The §18 substrate (implemented)

The six gaps named in docs §18 ("from PoC to live UI manifestation") are now built and e2e-verified (19/19 in headless Chrome):

1. **Fuel metering** — `CompileOpts{fuel}` / `CellBuilder::fuel(budget)`: every loop iteration burns one unit of an exported i32 gauge; zero traps (`unreachable`) instead of hanging. **Measured tax on the benchmark kernel: ≈0%** (interleaved medians, 3.6ms plain vs 3.5ms fueled at N=1e6 — V8 absorbs the check into the f64 pipeline; the 10–30% textbook estimate turned out pessimistic here). Applies to Tier 1 codegen; Tier 2 external artifacts still need a Worker + `terminate()`.
2. **Patch grammar + declarative event ABI** — `patch.rs` (`add/remove/update`, vocabulary-validated pre-mutation) + schema events (`on_click`/`on_input` → cell, `arg_from`, verdict-gated `patch`). The LiveUI tab is this loop, live.
3. **Module cache + supervision** — content-hash → `WebAssembly.Module` cache (identical seed = zero recompile); `Supervised` cells serve last-good values on a trap, rebuild, and quarantine after 3 consecutive failures (restartable).
4. **Memory capability** — `CompileOpts{memory_pages}` grants the cell its OWN fixed-size linear memory (exported, never imported — the audit still rejects memory imports); `load(i)`/`store(i,v)` builtins are bounds-checked (trap, never wrap-aliasing); host `write_mem`/`read_mem` complete the buffer ABI.
5. **Event bus** — `bus.rs`: cells never call cells; outputs cascade along schema-declared wires, breadth-first, under a hard dispatch budget (a cycle reports overflow instead of hanging).
6. **Durable state + replay** — the 3D world records `(t, keys)` per frame and replays from a zeroed world: **121-frame replay verified bit-identical** (f64 `to_bits` equality across all 32 state slots); world state persists to localStorage.

## Known limits (still deliberate)

- The DSL remains f64-scalar: no functions/strings; arrays only via the granted memory (`load`/`store`) — strings and rich structures stay in the host as vocabulary (formatter capabilities), by design.
- Tier 2 (external WASM) is not fuel-metered — its containment story is a Worker + `terminate()` (not built).
- Strict CSP needs `'wasm-unsafe-eval'` (instantiating bytes at runtime is treated as eval-class).
- Single flat scope (a duplicate `let` of the same name errors); module cache eviction is clear-at-capacity, not LRU.

## Files

- `src/parser.rs` — lexer + recursive-descent parser (let/while/if-else/%/calls/comparison + built-in min/max/abs/sqrt/floor) + AST→JS transpile
- `src/codegen.rs` — AST→WASM (wasm-encoder; `compile_with(params, imports)` — the import table is the capability list)
- `src/audit.rs` — the foundation of the seed-language spectrum: `imports_of()` / `audit(bytes, grants)` scan a module's import section, rejecting any import not in the grant list (or any memory/table/global import) — the module-level twin of codegen's "fetch() rejection", language-agnostic
- `assemblyscript/` — Tier 2 seed (`assembly/buddha.ts` → asc → 637B .wasm); produced by `npm run build`, served by api-server
- `src/lib.rs` — wasm-bindgen exports (`compile_to_wasm`/`compile_kernel_wasm`/`compile_draw_wasm`/`transpile_to_js`/`native_kernel`); feature `js-api` — downstream with `default-features = false` gets the pure compiler + audit, zero wasm-bindgen exports
- `leptos-poc/` — the eight-tab app; `src/cell.rs` = the only module that touches js-sys (CellBuilder: the grant list generates both the codegen import table and the JS env, so they can't drift; closure lifetimes encoded in the type; module cache + fuel gauge + write/read_mem live here too)
- `leptos-poc/src/{patch,bus,supervisor,live_tab}.rs` — the §18 substrate: patch grammar, budgeted event bus, per-cell supervision, and the LiveUI tab assembling them
- `api-server/` — Axum: static dist + `/api/{departments,members,form-schema,layout-schema,live-schema,examples,as,as-src}` (schemas/seeds read from disk per request)
- `examples/*.dsl` — buddha / guanyin / minecraft (isometric) / mc3p (playable third-person)
- `docs/multidimensional-composition-architecture.md` — the full theory essay (§0–§17, in Chinese)
