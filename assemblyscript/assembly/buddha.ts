// 佛陀的笑臉 —— AssemblyScript(Tier 2 種子)。
// 與自家 DSL 版對照:這裡有 const、typed 參數、function 抽取、for、真 if。
// 產物是標準 .wasm,import 節 = {env.sin,cos,hue,disc,ring,arc,line}(draw ABI),
// 走 wasm-jit 同一道 import 審計進沙箱——語言換了,圍欄沒換。

// —— host capabilities（@external 指定 import 的 module::name = env.*,對齊 draw ABI 的授權清單）——
@external("env", "sin")  declare function sin(x: f64): f64;
@external("env", "cos")  declare function cos(x: f64): f64;
@external("env", "hue")  declare function hue(v: f64): void;
@external("env", "disc") declare function disc(x: f64, y: f64, r: f64): void;
@external("env", "ring") declare function ring(x: f64, y: f64, r: f64): void;
@external("env", "arc")  declare function arc(x: f64, y: f64, r: f64, a0: f64, a1: f64): void;
@external("env", "line") declare function line(x1: f64, y1: f64, x2: f64, y2: f64): void;

// DSL 做不到的:把「一隻閉眼」抽成函數,兩眼各呼叫一次。
function closedEye(cx: f64, cy: f64, r: f64): void {
  arc(cx, cy, r, 3.35, 6.05);
}

// 匯出的 kernel。ABI 與自家 DSL 版一致:run(t, w, h) -> f64。
export function run(t: f64, w: f64, h: f64): f64 {
  const cx: f64 = w * 0.5;
  const cy: f64 = h * 0.56;
  const r: f64 = h * 0.27;

  // 光環(呼吸)
  hue(0.13);
  ring(cx, cy - r * 0.18, r * 1.55 + sin(t) * 6.0);

  // 肉髻
  hue(0.09);
  disc(cx, cy - r * 1.04, r * 0.2);

  // 臉
  hue(0.1);
  disc(cx, cy, r);

  // 長耳（for 迴圈 + 真 if,示範控制流；DSL 版是逐行 line）
  hue(0.1);
  for (let i: i32 = 0; i < 2; i++) {
    const side: f64 = i == 0 ? -1.0 : 1.0;
    line(cx + side * r * 1.06, cy - r * 0.15, cx + side * r * 1.06, cy + r * 0.5);
  }

  // 白毫
  hue(0.99);
  disc(cx, cy - r * 0.28, r * 0.045);

  // 閉目 + 笑（呼吸）
  hue(0.02);
  closedEye(cx - r * 0.36, cy - r * 0.02, r * 0.17);
  closedEye(cx + r * 0.36, cy - r * 0.02, r * 0.17);
  arc(cx, cy + r * 0.22, r * 0.46 + sin(t * 2.0) * 3.0, 0.45, 2.69);

  return 0.0;
}
