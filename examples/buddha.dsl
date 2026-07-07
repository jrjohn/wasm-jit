// 佛陀的笑臉 —— 由 7 個 primitive capability 顯化
let cx = w * 0.5;
let cy = h * 0.56;
let r = h * 0.27;

// 光環(呼吸)
hue(0.13);
ring(cx, cy - r * 0.18, r * 1.55 + sin(t) * 6.0);

// 肉髻
hue(0.09);
disc(cx, cy - r * 1.04, r * 0.2);

// 臉
hue(0.1);
disc(cx, cy, r);

// 長耳
line(cx - r * 1.06, cy - r * 0.15, cx - r * 1.06, cy + r * 0.5);
line(cx + r * 1.06, cy - r * 0.15, cx + r * 1.06, cy + r * 0.5);

// 白毫
hue(0.99);
disc(cx, cy - r * 0.28, r * 0.045);

// 閉目(上彎眼瞼)
hue(0.02);
arc(cx - r * 0.36, cy - r * 0.02, r * 0.17, 3.35, 6.05);
arc(cx + r * 0.36, cy - r * 0.02, r * 0.17, 3.35, 6.05);

// 笑(隨呼吸)
arc(cx, cy + r * 0.22, r * 0.46 + sin(t * 2.0) * 3.0, 0.45, 2.69);

0.0
