//! draw_tab.rs — 第 5 個 tab:自由繪(像素表面)。
//! 佛陀/觀音的 DSL 種子(由 /api/examples/{name} 於 runtime 載入)→
//! wasm-jit 編成細胞 → 7 個繪圖 primitive capability(sin/cos/hue/disc/ring/arc/line)
//! → 每 frame 原生速度顯化到 <canvas>。細胞無 DOM、無網路。

use crate::cell::Cell;
use gloo_net::http::Request;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::cell::Cell as BoolCell;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

thread_local! {
    static DRAW_CELL: RefCell<Option<Cell>> = const { RefCell::new(None) };
    static CTX: RefCell<Option<web_sys::CanvasRenderingContext2d>> = const { RefCell::new(None) };
    static LOOP_STARTED: BoolCell<bool> = const { BoolCell::new(false) };
    /// 鍵盤狀態:0=左 1=右 2=前 3=後 4=跳(方向鍵/WASD/Space)
    static KEYS: RefCell<[f64; 8]> = const { RefCell::new([0.0; 8]) };
    /// 細胞的 32 槽 f64 記憶(get/set capability)——跨 frame 狀態(玩家位置/速度)住這裡
    static STATE: RefCell<[f64; 32]> = const { RefCell::new([0.0; 32]) };
    static KEYS_HOOKED: BoolCell<bool> = const { BoolCell::new(false) };
}

fn key_index(code: &str) -> Option<usize> {
    match code {
        "ArrowLeft" | "KeyA" => Some(0),
        "ArrowRight" | "KeyD" => Some(1),
        "ArrowUp" | "KeyW" => Some(2),
        "ArrowDown" | "KeyS" => Some(3),
        "Space" => Some(4),
        _ => None,
    }
}

fn ensure_keys() {
    if KEYS_HOOKED.with(|s| s.replace(true)) {
        return;
    }
    let Some(w) = web_sys::window() else { return };
    let down = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(|e: web_sys::KeyboardEvent| {
        if let Some(i) = key_index(&e.code()) {
            e.prevent_default();
            KEYS.with(|k| k.borrow_mut()[i] = 1.0);
        }
    });
    let up = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(|e: web_sys::KeyboardEvent| {
        if let Some(i) = key_index(&e.code()) {
            KEYS.with(|k| k.borrow_mut()[i] = 0.0);
        }
    });
    let _ = w.add_event_listener_with_callback("keydown", down.as_ref().unchecked_ref());
    let _ = w.add_event_listener_with_callback("keyup", up.as_ref().unchecked_ref());
    std::mem::forget(down);
    std::mem::forget(up);
}

fn with_ctx(f: impl FnOnce(&web_sys::CanvasRenderingContext2d)) {
    CTX.with(|c| {
        if let Some(ctx) = c.borrow().as_ref() {
            f(ctx);
        }
    });
}

const TAU: f64 = 6.283185307;

/// 全部繪圖 + 互動 capability 的授權組(Tier 1 DSL 與 Tier 2 AS 產物共用)。
fn draw_builder() -> crate::cell::CellBuilder {
    Cell::builder(&["t", "w", "h"])
        .cap1("sin", f64::sin)
        .cap1("cos", f64::cos)
        .cap1_void("hue", |v| {
            with_ctx(|ctx| {
                let c = format!("hsl({},62%,62%)", (v.rem_euclid(1.0)) * 360.0);
                ctx.set_stroke_style_str(&c);
                ctx.set_fill_style_str(&c);
            })
        })
        .cap3_void("disc", |x, y, r| {
            with_ctx(|ctx| {
                ctx.begin_path();
                let _ = ctx.arc(x, y, r.max(0.0), 0.0, TAU);
                ctx.fill();
            })
        })
        .cap3_void("ring", |x, y, r| {
            with_ctx(|ctx| {
                ctx.begin_path();
                let _ = ctx.arc(x, y, r.max(0.0), 0.0, TAU);
                ctx.stroke();
            })
        })
        .cap5_void("arc", |x, y, r, a0, a1| {
            with_ctx(|ctx| {
                ctx.begin_path();
                let _ = ctx.arc(x, y, r.max(0.0), a0, a1);
                ctx.stroke();
            })
        })
        .cap4_void("line", |x1, y1, x2, y2| {
            with_ctx(|ctx| {
                ctx.begin_path();
                ctx.move_to(x1, y1);
                ctx.line_to(x2, y2);
                ctx.stroke();
            })
        })
        // 3D 用:col(hue, lightness) 精細配色、tri 填色三角形(quad = 兩個 tri)
        .cap2_void("col", |hv, l| {
            with_ctx(|ctx| {
                let c = format!(
                    "hsl({},55%,{}%)",
                    hv.rem_euclid(1.0) * 360.0,
                    (l.clamp(0.0, 1.0)) * 100.0
                );
                ctx.set_stroke_style_str(&c);
                ctx.set_fill_style_str(&c);
            })
        })
        .cap6_void("tri", |x1, y1, x2, y2, x3, y3| {
            with_ctx(|ctx| {
                ctx.begin_path();
                ctx.move_to(x1, y1);
                ctx.line_to(x2, y2);
                ctx.line_to(x3, y3);
                ctx.close_path();
                ctx.fill();
            })
        })
        // 互動用:key(i)=按鍵狀態、get/set=32 槽跨幀記憶、flr=floor
        .cap1("key", |i| {
            KEYS.with(|k| *k.borrow().get(i as usize).unwrap_or(&0.0))
        })
        .cap1("flr", f64::floor)
        .cap1("get", |i| {
            STATE.with(|s| *s.borrow().get(i as usize).unwrap_or(&0.0))
        })
        .cap2_void("set", |i, v| {
            STATE.with(|s| {
                if let Some(slot) = s.borrow_mut().get_mut(i as usize) {
                    *slot = v;
                }
            })
        })
}

/// Tier 1:自家 DSL 源碼 → codegen → cell。
fn build_draw_cell(src: &str) -> Result<Cell, String> {
    draw_builder().compile(src)
}

/// Tier 2:外部工具鏈(AssemblyScript)產物 → import 審計 → cell。同一組授權。
fn build_draw_cell_from_bytes(bytes: &[u8]) -> Result<Cell, String> {
    draw_builder().from_wasm_bytes(bytes)
}

fn frame(ts: f64) {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return };
    let Some(el) = doc.get_element_by_id("draw-cv") else { return };
    let Ok(cv) = el.dyn_into::<web_sys::HtmlCanvasElement>() else { return };
    let ctx = cv
        .get_context("2d")
        .ok()
        .flatten()
        .and_then(|o| o.dyn_into::<web_sys::CanvasRenderingContext2d>().ok());
    let Some(ctx) = ctx else { return };
    let (w, h) = (cv.width() as f64, cv.height() as f64);
    ctx.clear_rect(0.0, 0.0, w, h);
    ctx.set_line_width(7.0);
    ctx.set_line_cap("round");
    CTX.with(|c| *c.borrow_mut() = Some(ctx));
    DRAW_CELL.with(|c| {
        if let Some(cell) = c.borrow().as_ref() {
            let _ = cell.call(&[ts / 1000.0, w, h]);
        }
    });
}

/// 啟一次全域 rAF 迴圈;canvas 不在(切到別的 tab)就跳過該幀,回來自動續畫。
fn ensure_loop() {
    if LOOP_STARTED.with(|s| s.replace(true)) {
        return;
    }
    fn schedule(cb: &Closure<dyn FnMut(f64)>) {
        if let Some(w) = web_sys::window() {
            let _ = w.request_animation_frame(cb.as_ref().unchecked_ref());
        }
    }
    let holder: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let holder2 = holder.clone();
    *holder.borrow_mut() = Some(Closure::new(move |ts: f64| {
        frame(ts);
        if let Some(cb) = holder2.borrow().as_ref() {
            schedule(cb);
        }
    }));
    schedule(holder.borrow().as_ref().unwrap());
    std::mem::forget(holder); // 全域常駐迴圈,刻意不回收
}

#[component]
pub fn DrawPoc(#[prop(default = "buddha")] example: &'static str) -> impl IntoView {
    let script = RwSignal::new(String::new());
    let status = RwSignal::new(String::from("載入範例…"));
    let ok = RwSignal::new(true);
    let sel = RwSignal::new(example.to_string());
    // Tier 2:AS 產物的 bytes（None = Tier 1 DSL 模式,編 textarea 源碼）
    let as_bytes: RwSignal<Option<Vec<u8>>, LocalStorage> = RwSignal::new_local(None);

    let install = move |cell: Result<Cell, String>, tier: &str| match cell {
        Ok(cell) => {
            let size = cell.size();
            STATE.with(|s| s.borrow_mut().fill(0.0)); // 新種子 = 新世界,清記憶
            DRAW_CELL.with(|c| *c.borrow_mut() = Some(cell));
            status.set(format!("{tier} → {size} bytes — 顯化中"));
            ok.set(true);
        }
        Err(e) => {
            DRAW_CELL.with(|c| *c.borrow_mut() = None);
            status.set(format!("error: {e}"));
            ok.set(false);
        }
    };

    let compile_now = move || match as_bytes.get_untracked() {
        Some(b) => install(build_draw_cell_from_bytes(&b), "AS 產物 → import 審計通過"),
        None => install(build_draw_cell(&script.get_untracked()), "DSL → codegen"),
    };

    let load = move |name: String| {
        spawn_local(async move {
            if let Some(as_name) = name.strip_prefix("as:") {
                // Tier 2:抓 AS 源碼(顯示語法)+ 真 asc 產物(執行)
                let src = Request::get(&format!("/api/as-src/{as_name}"))
                    .send().await.ok();
                if let Some(r) = src {
                    script.set(r.text().await.unwrap_or_default());
                }
                match Request::get(&format!("/api/as/{as_name}")).send().await {
                    Ok(r) => match r.binary().await {
                        Ok(bytes) => {
                            as_bytes.set(Some(bytes));
                            compile_now();
                        }
                        Err(e) => { status.set(format!("AS wasm 讀取失敗: {e}")); ok.set(false); }
                    },
                    Err(e) => { status.set(format!("AS 載入失敗: {e}")); ok.set(false); }
                }
            } else {
                as_bytes.set(None);
                match Request::get(&format!("/api/examples/{name}")).send().await {
                    Ok(r) => { script.set(r.text().await.unwrap_or_default()); compile_now(); }
                    Err(e) => { status.set(format!("範例載入失敗: {e}")); ok.set(false); }
                }
            }
        })
    };

    ensure_loop();
    ensure_keys();
    load(example.to_string());

    view! {
        <p class="sub">
            "像素表面:DSL 種子由 /api/examples 載入 → 細胞 → 繪圖 primitives + "
            "key/get/set 互動 capabilities,每 frame 顯化。無 widget、無 DOM 權限。"
            "3D 範例:← → ↑ ↓ / WASD 移動、Space 跳(物理與投影全在種子裡算)。"
        </p>
        <canvas id="draw-cv" class="draw-cv" width="1440" height="900"></canvas>
        <div class="tok-row">
            "範例 "
            <select class="draw-example"
                prop:value=move || sel.get()
                on:change=move |ev| {
                    let v = event_target_value(&ev);
                    sel.set(v.clone());
                    load(v);
                }>
                <option value="buddha">"佛陀的笑臉"</option>
                <option value="guanyin">"觀音菩薩(全身+蓮台)"</option>
                <option value="minecraft">"3D 體素地形(等角視角)"</option>
                <option value="mc3p">"3D 體素世界(第三人稱跟隨,可走可跳)"</option>
                <option value="as:buddha">"佛陀 —— AssemblyScript(真 asc 編,Tier 2)"</option>
            </select>
            <button class="apply draw-run" on:click=move |_| compile_now()>"Compile & Run"</button>
            <button class="tok-violate draw-violate" on:click=move |_| {
                script.set("fetch(t);\n0.0".to_string());
                compile_now();
            }>"試圖越權 fetch()"</button>
            <span class="draw-status" class:ok=move || ok.get() class:bad=move || !ok.get()>
                {move || status.get()}
            </span>
        </div>
        <textarea class="draw-src" rows="14" spellcheck="false"
            prop:value=move || script.get()
            on:input=move |ev| script.set(event_target_value(&ev))></textarea>
    }
}
