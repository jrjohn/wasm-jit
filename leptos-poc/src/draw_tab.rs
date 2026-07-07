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
}

fn with_ctx(f: impl FnOnce(&web_sys::CanvasRenderingContext2d)) {
    CTX.with(|c| {
        if let Some(ctx) = c.borrow().as_ref() {
            f(ctx);
        }
    });
}

const TAU: f64 = 6.283185307;

fn build_draw_cell(src: &str) -> Result<Cell, String> {
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
        .compile(src)
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
pub fn DrawPoc() -> impl IntoView {
    let script = RwSignal::new(String::new());
    let status = RwSignal::new(String::from("載入範例…"));
    let ok = RwSignal::new(true);

    let compile_now = move || match build_draw_cell(&script.get_untracked()) {
        Ok(cell) => {
            let size = cell.size();
            DRAW_CELL.with(|c| *c.borrow_mut() = Some(cell));
            status.set(format!("compiled {size} bytes — 顯化中"));
            ok.set(true);
        }
        Err(e) => {
            DRAW_CELL.with(|c| *c.borrow_mut() = None);
            status.set(format!("compile error: {e}"));
            ok.set(false);
        }
    };

    let load = move |name: String| {
        spawn_local(async move {
            match Request::get(&format!("/api/examples/{name}")).send().await {
                Ok(r) => {
                    script.set(r.text().await.unwrap_or_default());
                    compile_now();
                }
                Err(e) => {
                    status.set(format!("範例載入失敗: {e}"));
                    ok.set(false);
                }
            }
        })
    };

    ensure_loop();
    load("buddha".to_string());

    view! {
        <p class="sub">
            "像素表面:DSL 種子由 /api/examples 載入 → 細胞 → 7 個繪圖 primitive"
            "(sin/cos/hue/disc/ring/arc/line,2D 完備基底)每 frame 顯化。無 widget、無 DOM 權限。"
        </p>
        <canvas id="draw-cv" class="draw-cv" width="1440" height="900"></canvas>
        <div class="tok-row">
            "範例 "
            <select class="draw-example" on:change=move |ev| load(event_target_value(&ev))>
                <option value="buddha" selected>"佛陀的笑臉"</option>
                <option value="guanyin">"觀音菩薩(全身+蓮台)"</option>
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
