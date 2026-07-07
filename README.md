# wasm-jit — 腳本即種子:runtime 生成 WASM 細胞(借瀏覽器 JIT),跑「不必被信任的碼」

**核心主張**:腳本在 runtime 編成 WASM bytes → `WebAssembly.instantiate()` → 瀏覽器引擎幫你 JIT → **近原生速度 + capability 沙箱 + 同步呼叫**,三者同時成立的唯一路徑。實測比可沙箱直譯器(Rhai)快 **133–147×**、與 AOT 天花板持平;對 JS 速度打平,但買到 JS 給不了的性質:**顯化物不需被信任**。

理論脈絡:`docs/multidimensional-composition-architecture.md`(§16 執行層、§17 AI 時代的前端形態)。

## PoC 一覽

**獨立頁面:**
1. **`index.html` — benchmark**:同一段腳本四路執行(Rhai / 生成 WASM / JS / AOT Rust),量化差距。
2. **`canvas.html` — 畫布**:2000 個元件各掛一顆獨立生成的 WASM kernel 細胞,每 frame 全跑 60fps。
3. **`draw.html` — 自由繪**:7 個繪圖 primitive,佛陀笑臉 / 觀音全身+蓮台由 DSL 種子顯化(`examples/*.dsl`)。

**`leptos-poc/` — 純 Rust CSR(Leptos 0.8)六分頁**(零手寫 JS,搭配 `api-server`):
| 分頁 | 證明什麼 |
|---|---|
| DynamicCell | 行為動態:腳本即時編輯 → 細胞驅動 signal → DOM 反應式更新 |
| 表單 | 9 種 widget schema 驅動(server 現讀磁碟 JSON,改檔重載即變);驗證/計算欄 = DSL 細胞;部門/人員走真 Axum API |
| Tokens | 樣式即 capability:SCSS 生成 `--tk-*` rails,style spec 只准引用 token,raw CSS 被拒 |
| Layout | 版面即 schema:整個 app shell(header/選單/profile/table)由遞迴 JSON 樹顯化,table 資料源也是資料 |
| 自由繪 | 像素表面收進 app:佛陀/觀音種子由 `/api/examples` 現讀 |
| **3D 體素** | **可玩的 Minecraft 風世界**:←→ 轉向、↑↓ 前進、Space 跳;真透視 + 鏡頭跟隨 + 無限地形 + 距離霧,**渲染與物理全在 2826-byte 種子裡**;互動經 `key`/`get`/`set`(狀態本身也是授予的 capability) |

**三種表面、三種完備詞彙**:像素(7 primitives)、表單(9 widgets)、版面(9 layout cells)——生成永遠不創造詞彙,只組合詞彙。

## wasm-jit 的威力(vs JS,誠實版)

速度:純 f64 kernel 與 JS 打平(V8 主場);好寫度:JS 贏(DSL 無 if/函數/陣列)。威力在五個 JS 結構上給不了的性質:

| 性質 | JS `new Function` | wasm-jit 細胞 |
|---|---|---|
| 世界的邊界 | ambient authority(整頁 fetch/document/cookies) | **import 表 = 全部世界**(3D 遊戲共 12 個 capability;`fetch()` 編譯期被拒) |
| 記憶 | 任意閉包/全域 | 連狀態都是授予的(get/set 32 槽) |
| 確定性 | 靠紀律 | 靠構造:同輸入位元級同輸出 → 可重放/審計 |
| 逃逸 | prototype 污染 + sandbox escape 史 | 記憶體隔離是 VM 規格 |
| 冷碼/幀時 | 要暖 JIT、GC/deopt 尾巴 | instantiate 即近原生、細胞內零分配 |

JS 拿隔離的三條路各缺一角:iframe(非同步)、直譯器(145× 慢)、SES(不成熟)。**「快+隔離+同步」只有 runtime 生成 WASM 全拿。** 加一層:文法即圍欄——DSL 只能表達 f64 數學 + 授權呼叫,「這段碼會碰什麼」是編譯期可枚舉的清單。**JS 寫的是你信任的碼;wasm-jit 跑的是你不必信任的碼**——AI 生成 / 使用者貼上 / schema 攜帶的顯化物,能被允許活起來,正因為活在細胞裡。

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
# 需 rustup target add wasm32-unknown-unknown + wasm-pack + trunk

# A) 獨立頁面(benchmark / canvas / draw)
wasm-pack build --target web --release
python3 -m http.server 8642              # open http://localhost:8642/{index,canvas,draw}.html

# B) leptos-poc 六分頁 + Rust API(表單/Layout/自由繪/3D 需要它)
cd leptos-poc && trunk build --release && cd ..
cargo run --release -p api-server -- leptos-poc/dist
# → http://127.0.0.1:8645(同源 serve dist + /api/*;schema/範例改磁碟檔即生效,零重編)

cargo test    # native 測試(default 含 Rhai lanes;--no-default-features 為純編譯器)
```

**可動手玩的檔案(改完重載即變,不碰 Rust)**:`api-server/form-schema.json`(表單)、`api-server/layout-schema.json`(版面)、`examples/*.dsl`(佛陀/觀音/等角體素/第三人稱體素)。

## 已知限制(PoC 刻意不做)

- **無 fuel metering**:生成模組無窮迴圈會掛住執行緒。production 要 Worker + `terminate()`,或 codegen 在迴圈 back-edge 插「計數器遞減+trap」(代價 ~10–30%)。
- DSL 僅 f64 / let / assign / while / 四則 / 比較——夠證明管線;字串/物件會把 codegen 帶進 boxing 地獄,正確用法是留在 host,只把數值熱核下沉。
- 嚴格 CSP 環境需 `'wasm-unsafe-eval'`(runtime instantiate bytes 被視同 eval 類)。
- 單一平坦 scope(重複 `let` 同名報錯)。

## 檔案

- `src/parser.rs` — lexer + 遞迴下降 parser(let/while/呼叫/比較,無 if)+ AST→JS 轉譯
- `src/codegen.rs` — AST→WASM(wasm-encoder;`compile_with(params, imports)`——import 表即 capability 清單)
- `src/lib.rs` — wasm-bindgen 匯出;feature gate:`js-api`(JS 匯出)/`rhai-bench`(對照組)——下游 `default-features = false` 拿純編譯器(leptos-poc bundle 2.85MB→631KB 的教訓)
- `leptos-poc/` — 六分頁 app;`src/cell.rs` = 唯一碰 js-sys 的模組(CellBuilder:grant 清單同時生成 codegen import 表與 JS env,不可能漂移;closure 生命週期入型別)
- `api-server/` — Axum:靜態 dist + `/api/{departments,members,form-schema,layout-schema,examples}`(schema 每請求現讀磁碟)
- `examples/*.dsl` — buddha / guanyin / minecraft(等角)/ mc3p(第三人稱可玩)
- `docs/multidimensional-composition-architecture.md` — 理論全文(§0–§17)
- `.cargo/config.toml` — getrandom 0.3 wasm_js backend(rhai→ahash 依賴鏈的必要 workaround)
