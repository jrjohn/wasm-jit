//! skins — the SKIN half of an inhabitant package (docs §19).
//!
//! An entity's granularity dissects into soul / skin / bounds. This crate is
//! the skin: it runs with full canvas authority, so it lives in the SLOW loop
//! (a .rs crate, reviewed, gated) — you cannot generate a skin, only inhabit
//! one. Interface: `<type>(ctx, px, py, s, t)` where s = canvas px per grid
//! unit and t = seconds (micro-animation). Detail adapts to s: far away stays
//! a clean silhouette, up close the brush strokes appear.

use wasm_bindgen::prelude::*;
use web_sys::CanvasRenderingContext2d;

const TAU: f64 = 6.283_185_307;
const INK: &str = "#1c2027"; // the silhouette ink
const HAT: &str = "#c9b87a"; // straw
const HULL: &str = "#5d452b"; // weathered wood
const LINE: &str = "#9aa4b2"; // fishing line / rigging

fn path_close_fill(ctx: &CanvasRenderingContext2d, color: &str) {
    ctx.close_path();
    ctx.set_fill_style_str(color);
    ctx.fill();
}

/// A wupeng river boat: upswept bow, low hull, one brush-stroke waterline.
#[wasm_bindgen]
pub fn boat(ctx: &CanvasRenderingContext2d, px: f64, py: f64, s: f64, t: f64) {
    let sway = (t * 1.3).sin() * s * 0.04; // riding its own wake
    let y = py + sway;
    ctx.begin_path();
    ctx.move_to(px - s * 1.7, y - s * 0.10); // stern tip
    ctx.quadratic_curve_to(px - s * 0.6, y + s * 0.42, px, y + s * 0.45); // belly
    ctx.quadratic_curve_to(px + s * 0.8, y + s * 0.40, px + s * 1.55, y - s * 0.05);
    ctx.quadratic_curve_to(px + s * 1.9, y - s * 0.28, px + s * 1.95, y - s * 0.42); // upswept bow
    ctx.quadratic_curve_to(px + s * 1.2, y - s * 0.12, px, y - s * 0.10);
    ctx.quadratic_curve_to(px - s * 1.0, y - s * 0.10, px - s * 1.7, y - s * 0.10);
    path_close_fill(ctx, HULL);
    if s > 6.0 {
        // plank strokes, only visible up close
        ctx.set_stroke_style_str("#453220");
        ctx.set_line_width((s * 0.05).max(0.6));
        ctx.begin_path();
        ctx.move_to(px - s * 1.3, y + s * 0.08);
        ctx.quadratic_curve_to(px, y + s * 0.22, px + s * 1.3, y + s * 0.05);
        ctx.stroke();
    }
    // waterline reflection: one pale stroke
    ctx.set_stroke_style_str("rgba(154,164,178,.35)");
    ctx.set_line_width((s * 0.07).max(0.7));
    ctx.begin_path();
    ctx.move_to(px - s * 1.5, y + s * 0.58);
    ctx.quadratic_curve_to(px, y + s * 0.66, px + s * 1.5, y + s * 0.55);
    ctx.stroke();
}

/// The straw-cloaked fisherman, seated, rod out over the water. Stillness with
/// a breath: the shoulders rise ~2% and the rod tip trembles; the float bobs.
#[wasm_bindgen]
pub fn fisherman(ctx: &CanvasRenderingContext2d, px: f64, py: f64, s: f64, t: f64) {
    let breath = (t * 0.9).sin() * s * 0.02;
    let y = py + breath;

    // seated body: bent back, knees drawn up — one closed silhouette
    ctx.begin_path();
    ctx.move_to(px - s * 0.34, y + s * 0.42); // seat, behind
    ctx.quadratic_curve_to(px - s * 0.52, y - s * 0.05, px - s * 0.22, y - s * 0.34); // curved back
    ctx.quadratic_curve_to(px - s * 0.05, y - s * 0.46, px + s * 0.10, y - s * 0.40); // shoulder→neck
    ctx.quadratic_curve_to(px + s * 0.34, y - s * 0.18, px + s * 0.40, y + s * 0.02); // chest→arm reach
    ctx.quadratic_curve_to(px + s * 0.46, y + s * 0.18, px + s * 0.30, y + s * 0.26); // forearm on knee
    ctx.quadratic_curve_to(px + s * 0.42, y + s * 0.34, px + s * 0.30, y + s * 0.44); // knee drop
    path_close_fill(ctx, INK);

    // head under the hat
    ctx.begin_path();
    let _ = ctx.arc(px - s * 0.02, y - s * 0.50, s * 0.16, 0.0, TAU);
    path_close_fill(ctx, INK);

    // straw cloak (蓑衣): layered fringe over the back
    if s > 5.0 {
        ctx.set_stroke_style_str("#6b5d3f");
        ctx.set_line_width((s * 0.05).max(0.6));
        for k in 0..5 {
            let f = k as f64 / 4.0;
            ctx.begin_path();
            ctx.move_to(px - s * (0.18 + 0.16 * f), y - s * (0.30 - 0.34 * f));
            ctx.quadratic_curve_to(
                px - s * (0.40 + 0.10 * f),
                y + s * (0.05 + 0.25 * f),
                px - s * (0.30 + 0.12 * f),
                y + s * (0.30 + 0.14 * f),
            );
            ctx.stroke();
        }
    }

    // the wide brim (斗笠), a shallow arc with a peak
    ctx.begin_path();
    ctx.move_to(px - s * 0.42, y - s * 0.52);
    ctx.quadratic_curve_to(px - s * 0.02, y - s * 0.98, px + s * 0.40, y - s * 0.50);
    ctx.quadratic_curve_to(px - s * 0.02, y - s * 0.62, px - s * 0.42, y - s * 0.52);
    path_close_fill(ctx, HAT);

    // rod, line, float — the tip trembles, the float bobs on the water
    let tremble = (t * 2.3).sin() * s * 0.03;
    let (tip_x, tip_y) = (px + s * 1.55, y - s * 0.42 + tremble);
    ctx.set_stroke_style_str("#3a3226");
    ctx.set_line_width((s * 0.06).max(0.7));
    ctx.begin_path();
    ctx.move_to(px + s * 0.34, y + s * 0.10); // from the hand
    ctx.quadratic_curve_to(px + s * 1.0, y - s * 0.28, tip_x, tip_y);
    ctx.stroke();
    let bob = (t * 1.7).sin() * s * 0.045;
    let (float_x, float_y) = (tip_x + s * 0.22, y + s * 0.62 + bob);
    ctx.set_stroke_style_str(LINE);
    ctx.set_line_width((s * 0.03).max(0.5));
    ctx.begin_path();
    ctx.move_to(tip_x, tip_y);
    ctx.line_to(float_x, float_y);
    ctx.stroke();
    ctx.begin_path();
    let _ = ctx.arc(float_x, float_y, (s * 0.05).max(0.8), 0.0, TAU);
    path_close_fill(ctx, "#b8433a");
}

/// A standing figure: head, sloped shoulders, weight on one leg.
#[wasm_bindgen]
pub fn person(ctx: &CanvasRenderingContext2d, px: f64, py: f64, s: f64, t: f64) {
    let breath = (t * 0.8).sin() * s * 0.015;
    let y = py + breath;
    ctx.begin_path();
    let _ = ctx.arc(px, y - s * 0.62, s * 0.15, 0.0, TAU);
    path_close_fill(ctx, INK);
    ctx.begin_path();
    ctx.move_to(px - s * 0.22, y - s * 0.44); // left shoulder
    ctx.quadratic_curve_to(px, y - s * 0.52, px + s * 0.22, y - s * 0.44);
    ctx.quadratic_curve_to(px + s * 0.18, y - s * 0.05, px + s * 0.12, y + s * 0.10); // torso taper
    ctx.line_to(px + s * 0.10, y + s * 0.55); // standing leg
    ctx.line_to(px + s * 0.02, y + s * 0.55);
    ctx.line_to(px + s * 0.01, y + s * 0.16);
    ctx.line_to(px - s * 0.06, y + s * 0.54); // relaxed leg, slight angle
    ctx.line_to(px - s * 0.14, y + s * 0.53);
    ctx.quadratic_curve_to(px - s * 0.18, y - s * 0.05, px - s * 0.22, y - s * 0.44);
    path_close_fill(ctx, INK);
}

/// A small car: cabin curve, body, two wheels, a window.
#[wasm_bindgen]
pub fn car(ctx: &CanvasRenderingContext2d, px: f64, py: f64, s: f64, _t: f64) {
    ctx.begin_path();
    ctx.move_to(px - s * 1.0, y_of(py, s, 0.30));
    ctx.line_to(px - s * 1.0, y_of(py, s, 0.02));
    ctx.quadratic_curve_to(px - s * 0.9, y_of(py, s, -0.18), px - s * 0.55, y_of(py, s, -0.20));
    ctx.quadratic_curve_to(px - s * 0.35, y_of(py, s, -0.52), px + s * 0.10, y_of(py, s, -0.52)); // cabin
    ctx.quadratic_curve_to(px + s * 0.55, y_of(py, s, -0.50), px + s * 0.70, y_of(py, s, -0.20));
    ctx.quadratic_curve_to(px + s * 1.0, y_of(py, s, -0.14), px + s * 1.0, y_of(py, s, 0.05));
    ctx.line_to(px + s * 1.0, y_of(py, s, 0.30));
    path_close_fill(ctx, "#8f3a3a");
    if s > 5.0 {
        ctx.set_fill_style_str("#aebacc");
        ctx.begin_path();
        ctx.move_to(px - s * 0.42, y_of(py, s, -0.22));
        ctx.quadratic_curve_to(px - s * 0.28, y_of(py, s, -0.46), px + s * 0.05, y_of(py, s, -0.46));
        ctx.line_to(px + s * 0.05, y_of(py, s, -0.22));
        path_close_fill(ctx, "#aebacc");
    }
    ctx.set_fill_style_str("#14171d");
    for wx in [px - s * 0.55, px + s * 0.55] {
        ctx.begin_path();
        let _ = ctx.arc(wx, py + s * 0.32, s * 0.20, 0.0, TAU);
        path_close_fill(ctx, "#14171d");
    }
}

fn y_of(py: f64, s: f64, k: f64) -> f64 {
    py + s * k
}
