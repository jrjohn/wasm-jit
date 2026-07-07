# wasm-jit-poc — Runtime WASM codegen(借 V8 JIT)vs Rhai tree-walk 直譯

最小 PoC,一頁 demo。驗證主張:**腳本在 runtime 編成 WASM bytes → `WebAssembly.instantiate()` → V8 幫你 JIT → 近原生執行**,對比 Rhai(tree-walking 直譯器)。

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
