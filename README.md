# wasm-jit — Runtime WASM codegen(借 V8 JIT)vs Rhai tree-walk 直譯

三個 PoC:
1. **`index.html` — benchmark**:同一段腳本四路執行(Rhai / 生成 WASM / JS / AOT Rust),量化差距。
2. **`canvas.html` — 畫布**:N 個元件各掛一顆「獨立生成、獨立編譯」的 WASM kernel 細胞,每 frame 全跑,驗證 60fps 可行性(§16「腳本即種子」執行層的活證明)。
3. **`leptos-poc/` — DynamicCell**:純 Rust CSR(Leptos 0.8)裡的 runtime 動態元件——「JS 元件動態」的 Rust 替身:**行為動態** = 腳本即時編輯 → wasm-jit 當場編成細胞 → 細胞經 `out()` capability 驅動 Leptos signal → DOM 反應式更新(細胞零 DOM 權限);**結構動態** = 元件樹即 schema 資料(JSON,Apply 即重組);**沙箱** = 腳本寫 `fetch()` 在 codegen 即被拒並列出 granted capabilities。全程零手寫 JS(僅 js-sys 做 instantiate)。`cd leptos-poc && trunk build --release`,serve `dist/`。CDP 驗證 4/4:渲染 / slider 反應 / fetch 拒絕 / schema 重組。

驗證主張:**腳本在 runtime 編成 WASM bytes → `WebAssembly.instantiate()` → V8 幫你 JIT → 近原生執行**,對比 Rhai(tree-walking 直譯器)。

```
src(textarea,DSL = Rhai 嚴格子集)
  → parser(Rust,遞迴下降)→ AST
      ├→ wasm-encoder → 117-byte .wasm 模組 → WebAssembly.instantiate()(V8 JIT)
      ├→ Rhai engine.compile → eval_ast_with_scope(tree-walk 直譯)
      ├→ 同 AST 轉譯 JS → new Function(V8 JS-JIT 參照)
      └→ 同 kernel 手寫 Rust,AOT 編進主模組(天花板參照)
```

同一段原始碼、四條執行路徑、值位元級一致(相同 f64 運算順序)。

## 實測(2026-07-07,Apple Silicon Mac,Chrome headless=new)

預設 kernel:`sum = sum + i*i - sum/(i+1)` 迴圈 N 次。exec 為自適應內圈 + 多輪中位。

| N | Rhai 直譯 | **生成 WASM(V8 JIT)** | JS `new Function` | AOT Rust | WASM vs Rhai |
|---|---|---|---|---|---|
| 1e4 | 4.9 ms | **0.033 ms** | 0.055 ms | 0.033 ms | **147×** |
| 1e6 | 434 ms | **3.27 ms** | 3.33 ms | 3.27 ms | **133×** |
| 1e7 | 4335 ms | **32.6 ms** | 32.3 ms | 31.2 ms | **133×** |

compile 成本(一次性):codegen 0.4–2.2 ms + instantiate ~0.6 ms;生成模組 **117 bytes**。

**結論:**
1. **生成 WASM = AOT 天花板**(1.0×)——V8 對這種數值 kernel 給出滿速 JIT,「借 V8 的 JIT」完全成立。
2. **比 Rhai 快 ~133–147×**——落在 tree-walker 預期劣勢帶(50–200×)。
3. **誠實註記:JS(V8 JS JIT)在純數值 kernel 上與 WASM 打平**——這條路的價值不是「贏 JS」,而是:①贏過*可沙箱的直譯器* 130×;②WASM 的 capability 沙箱(生成模組只能碰 import 給它的東西 + 自己的記憶體);③無 GC、幀時可預測。
4. 編譯成本 ~1–3 ms,一次攤提;熱路徑(重複呼叫)才值得編,run-once 用直譯器即可 → 正統 tiering:冷走直譯、熱編 WASM。

## 畫布 PoC 實測(canvas.html,2026-07-07,同機 headless Chrome)

每個元件一段**獨一份的腳本原文**(4 種模板 × 依 index 烘焙常數,模擬 AI 對每元件各生成一段行為),各自編成一顆 WASM 細胞(**capability imports 僅 `sin`/`cos`/`out`——import 表即能力清單**);kernel 重量 = while substep 數。三模式吃同一批腳本。

**N=500 元件 × 200 substeps(每 frame 10 萬次迭代):**

| 模式 | fps | kernel/frame | 幀預算佔用 | 編譯 |
|---|---|---|---|---|
| **生成 WASM 細胞** | **60** | 1.17 ms(p95 1.6) | **7%** | 500 顆 / 20ms / 150KB |
| JS new Function | 60 | 0.84 ms | 5% | 8ms |
| Rhai 直譯 | **18** | **54.6 ms** | **329%** | 38ms |

**N=2000 × 1000 substeps(20 倍重載,每 frame 200 萬次迭代):**

| 模式 | fps | kernel/frame | 幀預算 | 編譯 |
|---|---|---|---|---|
| **生成 WASM 細胞** | **60** | 4.8 ms | 29% | 2000 顆 / 78ms / 613KB |
| JS new Function | 60 | 4.4 ms | 27% | 38ms |
| Rhai(推算) | <1 | ~2200 ms | ~13000% | — |

**畫布結論:**
1. **「每個元件都是有生成行為的 agent」在直譯器上不成立(N=500 即塌到 18fps),在 WASM 細胞上是普通工程(2000 元件重載仍 60fps、預算 29%)**——§16「可行性開關」的活證明。
2. **2000 顆獨一無二的模組,編譯合計 78ms**(~0.04ms/顆)——「AI 對每元件各生成一段碼、當場編譯」的成本可忽略。
3. JS 照例打平——選 WASM 的理由仍是 capability 沙箱(每顆細胞只能 sin/cos/out,連 fetch 都叫不到——codegen 直接拒絕未授權能力),不是速度。

## 跑法

```bash
wasm-pack build --target web --release   # 需 rustup target wasm32-unknown-unknown + wasm-pack
python3 -m http.server 8642              # 任何靜態 server 皆可
open http://localhost:8642/
```

頁面載入即自動跑一輪 smoke(N=1e4),結果 JSON 寫入 `#results-json`(headless 驗證用);選 N 按 Run 重跑。1e7 時 Rhai 這條會凍住頁面數秒。

```bash
cargo test    # native 測試:parser、codegen(wasmparser 驗證)、Rhai vs native 同值
```

## 已知限制(PoC 刻意不做)

- **無 fuel metering**:生成模組無窮迴圈會掛住執行緒。production 要 Worker + `terminate()`,或 codegen 在迴圈 back-edge 插「計數器遞減+trap」(代價 ~10–30%)。
- DSL 僅 f64 / let / assign / while / 四則 / 比較——夠證明管線;字串/物件會把 codegen 帶進 boxing 地獄,正確用法是留在 host,只把數值熱核下沉。
- 嚴格 CSP 環境需 `'wasm-unsafe-eval'`(runtime instantiate bytes 被視同 eval 類)。
- 單一平坦 scope(重複 `let` 同名報錯)。

## 檔案

- `src/parser.rs` — lexer + 遞迴下降 parser + AST→JS 轉譯
- `src/codegen.rs` — AST→WASM(wasm-encoder;block/loop+br_if、f64 比較→i32)
- `src/lib.rs` — wasm-bindgen 匯出(compile_to_wasm / transpile_to_js / RhaiProgram / native_kernel)
- `index.html` — 一頁 demo(自適應計時、四路對比表、auto-smoke)
- `.cargo/config.toml` — getrandom 0.3 wasm_js backend(rhai→ahash 依賴鏈的必要 workaround)
