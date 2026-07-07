# wasm-jit — 腳本即種子:runtime 生成 WASM 細胞(借瀏覽器 JIT),跑「不必被信任的碼」

**核心主張**:腳本在 runtime 編成 WASM bytes → `WebAssembly.instantiate()` → 瀏覽器引擎幫你 JIT → **近原生速度 + capability 沙箱 + 同步呼叫**,三者同時成立的唯一路徑。實測與 AOT 天花板持平、對 JS 速度打平——但買到 JS 給不了的性質:**顯化物不需被信任**(import 表就是它能碰的全部世界)。

> **緣起與致謝**:這個構想始於「想要一個*可沙箱*的 runtime 腳本語言」。我們最初拿 [Rhai](https://github.com/rhaiscript/rhai) 當原型,發現 tree-walking 直譯器用原生速度換來它的沙箱——而 wasm-jit 想兩者兼得。**感謝 Rhai 給的火花**;它啟發了整個方向,而本專案已不含 Rhai(比較的敘事也拿掉,回到自己的主張:沙箱裡的原生速度)。

理論脈絡:`docs/multidimensional-composition-architecture.md`(§16 執行層、§17 AI 時代的前端形態)。

## PoC 一覽

**獨立頁面:**
1. **`index.html` — benchmark**:同一段腳本三路執行(生成 WASM / JS / AOT Rust),證生成 WASM = AOT 天花板、與 JS 打平。
2. **`canvas.html` — 畫布**:2000 個元件各掛一顆獨立生成的 WASM kernel 細胞,每 frame 全跑 60fps。
3. **`draw.html` — 自由繪**:7 個繪圖 primitive,佛陀笑臉 / 觀音全身+蓮台由 DSL 種子顯化(`examples/*.dsl`)。

**`leptos-poc/` — 純 Rust CSR(Leptos 0.8)七分頁**(零手寫 JS,搭配 `api-server`):
| 分頁 | 證明什麼 |
|---|---|
| DynamicCell | 行為動態:腳本即時編輯 → 細胞驅動 signal → DOM 反應式更新 |
| 表單 | 9 種 widget schema 驅動(server 現讀磁碟 JSON,改檔重載即變);驗證/計算欄 = DSL 細胞;部門/人員走真 Axum API |
| Tokens | 樣式即 capability:SCSS 生成 `--tk-*` rails,style spec 只准引用 token,raw CSS 被拒 |
| Layout | 版面即 schema:整個 app shell(header/選單/profile/table)由遞迴 JSON 樹顯化,table 資料源也是資料 |
| 自由繪 | 像素表面:佛陀/觀音由 DSL 種子現讀;**+「佛陀 — AssemblyScript」= 真 asc 編的 637-byte 種子,走同一道 import 審計** |
| **3D 體素** | **可玩的 Minecraft 風世界**:←→ 轉向、↑↓ 前進、Space 跳;真透視 + 鏡頭跟隨 + 無限地形 + 距離霧,**渲染與物理全在 ~2.4KB 種子裡**;互動經 `key`/`get`/`set`(狀態本身也是授予的 capability) |
| 種子語言光譜 | **圍欄語言無關**:Tier 1(DSL codegen)vs Tier 2(外部 WASM,走 `Cell::from_wasm_bytes` 的 import 審計)同 ABI 值一致;越權外部種子(額外 import `env::fetch`)被拒 |

**三種表面、三種完備詞彙**:像素(7 primitives)、表單(9 widgets)、版面(9 layout cells)——生成永遠不創造詞彙,只組合詞彙。

**種子語言光譜**(§16):圍欄在 import 表不在文法,所以同一道沙箱容得下多種種子語言——
| Tier | 語言 | 圍欄入口 | 實證 |
|---|---|---|---|
| 1 | 自家 DSL(f64 純量:let/while/**if-else**/四則+%/比較 + 內建 min/max/abs/sqrt/floor) | codegen 拒未授權函式 | 全部 DSL 種子 |
| 2 | **AssemblyScript**(TS 語法)/ Rust→wasm / 手寫 WAT | `Cell::from_wasm_bytes` 前審計 import 節 ⊆ 授權 | `assembly/buddha.ts` 經 asc 編、走 draw 分頁畫出 |

## wasm-jit 的威力(vs JS,誠實版)

速度:純 f64 kernel 與 JS 打平(V8 主場);好寫度:JS 仍略勝(DSL 已補 if/%/內建數學;仍無函數/陣列/字串)。威力在五個 JS 結構上給不了的性質:

| 性質 | JS `new Function` | wasm-jit 細胞 |
|---|---|---|
| 世界的邊界 | ambient authority(整頁 fetch/document/cookies) | **import 表 = 全部世界**(3D 遊戲共 12 個 capability;`fetch()` 編譯期被拒) |
| 記憶 | 任意閉包/全域 | 連狀態都是授予的(get/set 32 槽) |
| 確定性 | 靠紀律 | 靠構造:同輸入位元級同輸出 → 可重放/審計 |
| 逃逸 | prototype 污染 + sandbox escape 史 | 記憶體隔離是 VM 規格 |
| 冷碼/幀時 | 要暖 JIT、GC/deopt 尾巴 | instantiate 即近原生、細胞內零分配 |

JS 拿隔離的三條路各缺一角:iframe(非同步)、可沙箱直譯器(慢一到兩個數量級)、SES(不成熟)。**「快+隔離+同步」只有 runtime 生成 WASM 全拿。** 加一層:文法即圍欄——DSL 只能表達 f64 數學 + 授權呼叫,「這段碼會碰什麼」是編譯期可枚舉的清單。**JS 寫的是你信任的碼;wasm-jit 跑的是你不必信任的碼**——AI 生成 / 使用者貼上 / schema 攜帶的顯化物,能被允許活起來,正因為活在細胞裡。

```
src(textarea,小型 f64 種子語言)
  → parser(Rust,遞迴下降)→ AST
      ├→ wasm-encoder → 百 byte 級 .wasm 模組 → WebAssembly.instantiate()(引擎 JIT)
      ├→ 同 AST 轉譯 JS → new Function(引擎 JS-JIT 參照)
      └→ 同 kernel 手寫 Rust,AOT 編進主模組(天花板參照)
```

同一段原始碼、三條執行路徑、值位元級一致(相同 f64 運算順序)。

## 實測(2026-07-07,Apple Silicon Mac,Chrome headless=new)

預設 kernel:`sum = sum + i*i - sum/(i+1)` 迴圈 N 次。exec 為自適應內圈 + 多輪中位。

| N | **生成 WASM(引擎 JIT)** | JS `new Function` | AOT Rust(天花板) | WASM vs AOT |
|---|---|---|---|---|
| 1e4 | **0.033 ms** | 0.055 ms | 0.033 ms | **1.0×** |
| 1e6 | **3.27 ms** | 3.33 ms | 3.27 ms | **1.0×** |
| 1e7 | **32.6 ms** | 32.3 ms | 31.2 ms | **1.04×** |

compile 成本(一次性):codegen 0.4–2.2 ms + instantiate ~0.6 ms;生成模組 **~117 bytes**。

**結論:**
1. **生成 WASM = AOT 天花板(≈1.0×)**——引擎對這種數值 kernel 給出滿速 JIT,「借引擎的 JIT」零額外開銷。
2. **與手寫 JS 打平**(純 f64 kernel 是 V8 JS-JIT 的主場)——所以這條路的價值**不是速度**,而是:① capability 沙箱(生成模組只能碰 import 給它的東西 + 自己的記憶體,`fetch()` 編譯期被拒);② 確定可重放(位元級同值);③ 無 GC、幀時可預測。
3. 編譯成本 ~1–3 ms,一次攤提;熱路徑(重複呼叫)才值得編,run-once 直接用其他辦法即可 → tiering:冷碼別編、熱碼編成 WASM。

## 畫布 PoC 實測(canvas.html,2026-07-07,同機 headless Chrome)

每個元件一段**獨一份的腳本原文**(4 種模板 × 依 index 烘焙常數,模擬 AI 對每元件各生成一段行為),各自編成一顆 WASM 細胞(**capability imports 僅 `sin`/`cos`/`out`——import 表即能力清單**);kernel 重量 = while substep 數。兩模式吃同一批腳本。

| 配置 | **生成 WASM 細胞** | JS new Function |
|---|---|---|
| N=500 × 200 substeps(每 frame 10 萬迭代) | **60fps · 1.17 ms · 幀預算 7%** | 60fps · 0.84 ms · 5% |
| N=2000 × 1000(20 倍重載,每 frame 200 萬迭代) | **60fps · 4.8 ms · 29%**(2000 顆 / 編譯 78ms / 613KB) | 60fps · 4.4 ms · 27% |

**畫布結論:**
1. **「每個元件都是有生成行為的 agent」在 WASM 細胞上是普通工程**(2000 元件重載仍 60fps、預算 29%)——§16「可行性開關」的活證明;2000 顆獨一無二的模組編譯合計 78ms(~0.04ms/顆),「AI 對每元件各生成一段碼、當場編譯」成本可忽略。
2. JS 速度照例打平——選 WASM 的理由仍是 capability 沙箱(每顆細胞只能 sin/cos/out,連 fetch 都叫不到,codegen 直接拒絕未授權能力),不是速度。

## 跑法

```bash
# 需 rustup target add wasm32-unknown-unknown + wasm-pack + trunk

# A) 獨立頁面(benchmark / canvas / draw)
wasm-pack build --target web --release
python3 -m http.server 8642              # open http://localhost:8642/{index,canvas,draw}.html

# B) leptos-poc 七分頁 + Rust API(表單/Layout/自由繪/3D/光譜 需要它)
cd assemblyscript && npm install && npm run build && cd ..   # Tier 2:asc 編佛陀種子(選配)
cd leptos-poc && trunk build --release && cd ..
cargo run --release -p api-server -- leptos-poc/dist
# → http://127.0.0.1:8645(同源 serve dist + /api/*;schema/範例改磁碟檔即生效,零重編)

cargo test                          # native 測試
cargo run --example audit_as --no-default-features   # dogfood:用自家 audit 驗真 asc 產物
```

**可動手玩的檔案(改完重載即變,不碰 Rust)**:`api-server/form-schema.json`(表單)、`api-server/layout-schema.json`(版面)、`examples/*.dsl`(佛陀/觀音/等角體素/第三人稱體素)、`assemblyscript/assembly/buddha.ts`(Tier 2 AS 種子,`npm run build` 後生效)。

## 已知限制(PoC 刻意不做)

- **無 fuel metering**:生成模組無窮迴圈會掛住執行緒。production 要 Worker + `terminate()`,或 codegen 在迴圈 back-edge 插「計數器遞減+trap」(代價 ~10–30%)。
- DSL 為 f64 純量語言:let / assign / while / **if-else** / 四則+**%** / 比較 / 內建 **min-max-abs-sqrt-floor**;仍無函數/陣列/字串——字串與資料結構留在 host,只把數值熱核下沉(要容器 = 授予 memory capability,是安全決策不是文法問題)。
- 嚴格 CSP 環境需 `'wasm-unsafe-eval'`(runtime instantiate bytes 被視同 eval 類)。
- 單一平坦 scope(重複 `let` 同名報錯)。

## 檔案

- `src/parser.rs` — lexer + 遞迴下降 parser(let/while/if-else/%/呼叫/比較 + 內建 min-max-abs-sqrt-floor)+ AST→JS 轉譯
- `src/codegen.rs` — AST→WASM(wasm-encoder;`compile_with(params, imports)`——import 表即 capability 清單)
- `src/audit.rs` — 種子語言光譜的地基:`imports_of()` / `audit(bytes, grants)` 掃模組 import 節,任一 import 不在授權清單(或想 import memory/table/global)即拒——「fetch() codegen 拒絕」的模組級孿生,語言無關
- `assemblyscript/` — Tier 2 種子(`assembly/buddha.ts` → asc → 637B .wasm);`npm run build` 產出,api-server 供應
- `src/lib.rs` — wasm-bindgen 匯出(`compile_to_wasm`/`compile_kernel_wasm`/`compile_draw_wasm`/`transpile_to_js`/`native_kernel`);feature `js-api`——下游 `default-features = false` 拿純編譯器 + audit,零 wasm-bindgen 匯出
- `leptos-poc/` — 七分頁 app;`src/cell.rs` = 唯一碰 js-sys 的模組(CellBuilder:grant 清單同時生成 codegen import 表與 JS env,不可能漂移;closure 生命週期入型別)
- `api-server/` — Axum:靜態 dist + `/api/{departments,members,form-schema,layout-schema,examples,as,as-src}`(schema/種子每請求現讀磁碟)
- `examples/*.dsl` — buddha / guanyin / minecraft(等角)/ mc3p(第三人稱可玩)
- `docs/multidimensional-composition-architecture.md` — 理論全文(§0–§17)
