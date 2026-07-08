//! draw_tab.rs — the 5th tab: free-drawing (a pixel surface).
//! Buddha/Guanyin DSL seeds (loaded at runtime from /api/examples/{name}) →
//! compiled by wasm-jit into a cell → 7 drawing-primitive capabilities (sin/cos/hue/disc/ring/arc/line)
//! → manifested to a <canvas> at native speed every frame. The cell has no DOM, no network.

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
    /// Keyboard state: 0=left 1=right 2=forward 3=back 4=jump (arrow keys/WASD/Space)
    static KEYS: RefCell<[f64; 8]> = const { RefCell::new([0.0; 8]) };
    /// The cell's 32-slot f64 memory (get/set capability) — cross-frame state (player position/velocity) lives here
    static STATE: RefCell<[f64; 32]> = const { RefCell::new([0.0; 32]) };
    static KEYS_HOOKED: BoolCell<bool> = const { BoolCell::new(false) };

    // ---- input recording / deterministic replay (docs §18, layer #6) ----
    /// Recorded input stream: one (t, keys[0..5]) sample per frame.
    static TAPE: RefCell<Vec<(f64, [f64; 5])>> = const { RefCell::new(Vec::new()) };
    static RECORDING: BoolCell<bool> = const { BoolCell::new(false) };
    static REPLAYING: BoolCell<bool> = const { BoolCell::new(false) };
    static REPLAY_IDX: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// STATE snapshot taken when recording stops — the replay's expected end state.
    static END_STATE: RefCell<Option<[f64; 32]>> = const { RefCell::new(None) };
    /// Signal handle for reporting record/replay results back into the view.
    static REC_SIG: std::cell::Cell<Option<RwSignal<String>>> = const { std::cell::Cell::new(None) };
}

fn rec_report(msg: String) {
    if let Some(sig) = REC_SIG.with(|s| s.get()) {
        sig.set(msg);
    }
}

/// Zero the cell's cross-frame state and the key latches: a fresh world.
/// Determinism makes this the whole story — same seed + same inputs from a
/// zeroed world ⇒ a bit-identical trajectory.
fn reset_world() {
    STATE.with(|s| s.borrow_mut().fill(0.0));
    KEYS.with(|k| k.borrow_mut().fill(0.0));
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

/// Fuel budget per frame call. mc3p runs ~2k loop iterations/frame — 5M is
/// generous headroom for hand-tuned seeds while still trapping a runaway
/// loop within milliseconds instead of hanging the tab.
const DRAW_FUEL: u32 = 5_000_000;

/// The full grant set of drawing + interaction capabilities (shared by the Tier 1 DSL and the Tier 2 AS artifact).
/// Fuel metering applies to Tier 1 (our codegen instruments the loops); a
/// Tier 2 external artifact carries no back-edge counters — its containment
/// story is a Worker + terminate (docs §18).
fn draw_builder() -> crate::cell::CellBuilder {
    Cell::builder(&["t", "w", "h"])
        .fuel(DRAW_FUEL)
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
        // For 3D: col(hue, lightness) for fine-grained coloring, tri fills a triangle (a quad = two tris)
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
        // For interaction: key(i)=key state, get/set=32-slot cross-frame memory, flr=floor
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

/// Tier 1: home DSL source → codegen → cell.
fn build_draw_cell(src: &str) -> Result<Cell, String> {
    draw_builder().compile(src)
}

/// Tier 2: external toolchain (AssemblyScript) artifact → import audit → cell. Same grant set.
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

    // Deterministic replay: override t and the key latches from the tape.
    let mut t = ts / 1000.0;
    if REPLAYING.get() {
        let step = TAPE.with(|tp| tp.borrow().get(REPLAY_IDX.get()).copied());
        match step {
            Some((rt, keys)) => {
                t = rt;
                KEYS.with(|k| {
                    let mut k = k.borrow_mut();
                    k[..5].copy_from_slice(&keys);
                });
                REPLAY_IDX.set(REPLAY_IDX.get() + 1);
            }
            None => {
                REPLAYING.set(false);
                // The moat, cashed in: same seed + same inputs ⇒ the replay's
                // end state must be BIT-identical to the recorded one.
                let now = STATE.with(|s| *s.borrow());
                let expected = END_STATE.with(|e| *e.borrow());
                let msg = match expected {
                    Some(exp) => {
                        let identical = exp
                            .iter()
                            .zip(now.iter())
                            .all(|(a, b)| a.to_bits() == b.to_bits());
                        let frames = TAPE.with(|tp| tp.borrow().len());
                        format!("replay done ({frames} frames) — bit-identical: {identical}")
                    }
                    None => "replay done (no recorded end state to compare)".into(),
                };
                rec_report(msg);
                return;
            }
        }
    } else if RECORDING.get() {
        let keys = KEYS.with(|k| {
            let k = k.borrow();
            [k[0], k[1], k[2], k[3], k[4]]
        });
        TAPE.with(|tp| tp.borrow_mut().push((t, keys)));
    }

    DRAW_CELL.with(|c| {
        if let Some(cell) = c.borrow().as_ref() {
            let _ = cell.call(&[t, w, h]);
        }
    });
}

/// Start one global rAF loop; if the canvas isn't present (switched to another tab) skip that frame, and resume drawing automatically on return.
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
    std::mem::forget(holder); // a global persistent loop, deliberately never reclaimed
}

#[component]
pub fn DrawPoc(#[prop(default = "buddha")] example: &'static str) -> impl IntoView {
    let script = RwSignal::new(String::new());
    let status = RwSignal::new(String::from("loading example…"));
    let ok = RwSignal::new(true);
    let sel = RwSignal::new(example.to_string());
    // Tier 2: the AS artifact's bytes (None = Tier 1 DSL mode, compiling the textarea source)
    let as_bytes: RwSignal<Option<Vec<u8>>, LocalStorage> = RwSignal::new_local(None);

    let install = move |cell: Result<Cell, String>, tier: &str| match cell {
        Ok(cell) => {
            let size = cell.size();
            reset_world(); // a new seed = a new world, clear the memory
            DRAW_CELL.with(|c| *c.borrow_mut() = Some(cell));
            status.set(format!("{tier} → {size} bytes — manifesting"));
            ok.set(true);
        }
        Err(e) => {
            DRAW_CELL.with(|c| *c.borrow_mut() = None);
            status.set(format!("error: {e}"));
            ok.set(false);
        }
    };

    let compile_now = move || match as_bytes.get_untracked() {
        Some(b) => install(build_draw_cell_from_bytes(&b), "AS output → import audit passed"),
        None => install(build_draw_cell(&script.get_untracked()), "DSL → codegen"),
    };

    let load = move |name: String| {
        spawn_local(async move {
            if let Some(as_name) = name.strip_prefix("as:") {
                // Tier 2: fetch the AS source (to show the syntax) + the real asc artifact (to execute)
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
                        Err(e) => { status.set(format!("AS wasm read failed: {e}")); ok.set(false); }
                    },
                    Err(e) => { status.set(format!("AS load failed: {e}")); ok.set(false); }
                }
            } else {
                as_bytes.set(None);
                match Request::get(&format!("/api/examples/{name}")).send().await {
                    Ok(r) => { script.set(r.text().await.unwrap_or_default()); compile_now(); }
                    Err(e) => { status.set(format!("example load failed: {e}")); ok.set(false); }
                }
            }
        })
    };

    ensure_loop();
    ensure_keys();
    load(example.to_string());

    // ---- record / replay / durable state (docs §18, layer #6) ----
    let rec_status = RwSignal::new(String::new());
    REC_SIG.with(|s| s.set(Some(rec_status)));

    let start_rec = move |_| {
        REPLAYING.set(false);
        reset_world();
        TAPE.with(|t| t.borrow_mut().clear());
        END_STATE.with(|e| *e.borrow_mut() = None);
        RECORDING.set(true);
        rec_status.set("recording from a zeroed world… play, then Stop".into());
    };
    let stop_rec = move |_| {
        if !RECORDING.get() {
            return;
        }
        RECORDING.set(false);
        END_STATE.with(|e| *e.borrow_mut() = Some(STATE.with(|s| *s.borrow())));
        let n = TAPE.with(|t| t.borrow().len());
        rec_status.set(format!("recorded {n} frames + end-state snapshot — Replay to verify"));
    };
    let replay = move |_| {
        let n = TAPE.with(|t| t.borrow().len());
        if n == 0 {
            rec_status.set("nothing recorded yet".into());
            return;
        }
        RECORDING.set(false);
        reset_world();
        REPLAY_IDX.set(0);
        REPLAYING.set(true);
        rec_status.set(format!("replaying {n} frames from the tape…"));
    };

    let storage = || web_sys::window().and_then(|w| w.local_storage().ok().flatten());
    let save_state = move |_| {
        let key = format!("wasmjit-state-{}", sel.get_untracked());
        let state: Vec<f64> = STATE.with(|s| s.borrow().to_vec());
        let json = serde_json::to_string(&state).unwrap_or_default();
        match storage() {
            Some(st) if st.set_item(&key, &json).is_ok() => {
                rec_status.set(format!("world state persisted to localStorage[{key}]"));
            }
            _ => rec_status.set("localStorage unavailable".into()),
        }
    };
    let restore_state = move |_| {
        let key = format!("wasmjit-state-{}", sel.get_untracked());
        let loaded = storage().and_then(|st| st.get_item(&key).ok().flatten());
        match loaded.and_then(|j| serde_json::from_str::<Vec<f64>>(&j).ok()) {
            Some(v) if v.len() == 32 => {
                STATE.with(|s| s.borrow_mut().copy_from_slice(&v));
                rec_status.set(format!("world state restored from localStorage[{key}]"));
            }
            _ => rec_status.set(format!("no saved state under {key}")),
        }
    };

    view! {
        <p class="sub">
            "Pixel surface: a DSL seed loaded from /api/examples → cell → drawing primitives + "
            "key/get/set interaction capabilities, manifested every frame. No widgets, no DOM authority. "
            "3D example: ← → ↑ ↓ / WASD to move, Space to jump (physics and projection all computed inside the seed)."
        </p>
        <canvas id="draw-cv" class="draw-cv" width="1440" height="900"></canvas>
        <div class="tok-row">
            "example "
            <select class="draw-example"
                prop:value=move || sel.get()
                on:change=move |ev| {
                    let v = event_target_value(&ev);
                    sel.set(v.clone());
                    load(v);
                }>
                <option value="buddha">"Smiling Buddha"</option>
                <option value="guanyin">"Guanyin (full body + lotus throne)"</option>
                <option value="minecraft">"3D voxel terrain (isometric)"</option>
                <option value="mc3p">"3D voxel world (third-person chase, walk + jump)"</option>
                <option value="as:buddha">"Buddha — AssemblyScript (real asc build, Tier 2)"</option>
            </select>
            <button class="apply draw-run" on:click=move |_| compile_now()>"Compile & Run"</button>
            <button class="tok-violate draw-violate" on:click=move |_| {
                script.set("fetch(t);\n0.0".to_string());
                compile_now();
            }>"try to escalate: fetch()"</button>
            <span class="draw-status" class:ok=move || ok.get() class:bad=move || !ok.get()>
                {move || status.get()}
            </span>
        </div>
        <div class="rec-bar">
            "determinism · "
            <button class="rec-start" on:click=start_rec>"⏺ Record"</button>
            <button class="rec-stop" on:click=stop_rec>"⏹ Stop"</button>
            <button class="rec-replay" on:click=replay>"▶ Replay (bit-identity check)"</button>
            <button class="rec-save" on:click=save_state>"Save world state"</button>
            <button class="rec-restore" on:click=restore_state>"Restore world state"</button>
            <span class="rec-status">{move || rec_status.get()}</span>
        </div>
        <textarea class="draw-src" rows="14" spellcheck="false"
            prop:value=move || script.get()
            on:input=move |ev| script.set(event_target_value(&ev))></textarea>
    }
}
