// 3D 體素世界(可走可跳,Minecraft 風)
// ← → ↑ ↓ / WASD 移動、Space 跳。
// 3D 只是像素表面上的數學:等角投影 + 對角線畫家演算法 + sin/cos 無限地形。
// 物理(重力/跳躍)與鏡頭跟隨全在種子裡;跨幀狀態住在被授予的 get/set 32 槽記憶。
// DSL 沒有 if —— 條件全用比較式(0/1)乘法合成。

let b = h * 0.036;
let bx = b * 0.87;
let bz = b * 0.5;
let by = b * 0.95;
let cx = w * 0.5;
let cy = h * 0.36;

// —— 讀狀態(首幀初始化到 6.5, 6.5)——
let ini = get(5.0);
let px = get(0.0) * ini + 6.5 * (1.0 - ini);
let pz = get(1.0) * ini + 6.5 * (1.0 - ini);
let py = get(2.0) * ini + 4.0 * (1.0 - ini);
let vy = get(3.0);
let lt = get(4.0);
let dtr = t - lt;
let dt = dtr * (dtr < 0.1) + 0.016 * (dtr >= 0.1);
set(4.0, t);
set(5.0, 1.0);

// —— 移動(鍵盤 capability)——
px = px + (key(1.0) - key(0.0)) * 3.5 * dt;
pz = pz + (key(3.0) - key(2.0)) * 3.5 * dt;

// —— 腳下地形高度(高度場公式;下方地形迴圈用同一式)——
let gp = 2.4 + sin(px * 0.55) + cos(pz * 0.65) + sin((px + pz) * 0.35) * 0.6;

// —— 跳躍與重力 ——
let on = (py <= gp + 0.05);
vy = vy + (7.5 - vy) * on * key(4.0);
vy = vy - 20.0 * dt;
py = py + vy * dt;
let und = (py < gp);
py = py * (1.0 - und) + gp * und;
vy = vy * (1.0 - und);
set(0.0, px);
set(1.0, pz);
set(2.0, py);
set(3.0, vy);

// —— 太陽 ——
hue(0.13);
disc(cx + w * 0.3, h * 0.10, b * 1.2);

// —— 無限體素地形:以玩家為中心 15×15,對角線由遠到近(畫家演算法)——
let x0 = flr(px) - 7.0;
let z0 = flr(pz) - 7.0;
let s = 0.0;
while s < 29.0 {
    let i0 = (s - 14.0) * (s > 14.0);
    let i1 = s * (s < 15.0) + 14.0 * (s >= 15.0);
    let i = i0;
    while i <= i1 {
        let xg = x0 + i;
        let zg = z0 + (s - i);
        let g = 2.4 + sin(xg * 0.55) + cos(zg * 0.65) + sin((xg + zg) * 0.35) * 0.6;
        let sx = cx + ((xg - px) - (zg - pz)) * bx;
        let sy = cy + ((xg - px) + (zg - pz)) * bz;
        let ty = sy - g * by;
        let d = g * by;
        // 顏色帶:低=沙、中=草、高=雪
        let lo = (g < 1.7);
        let hi = (g >= 3.4);
        let mid = (1.0 - lo) * (1.0 - hi);
        let chh = 0.12 * lo + 0.33 * mid + 0.55 * hi;
        let cll = 0.58 * lo + 0.42 * mid + 0.85 * hi;
        // 頂面(菱形 = 兩個三角)
        col(chh, cll);
        tri(sx, ty - bz, sx + bx, ty, sx, ty + bz);
        tri(sx, ty - bz, sx - bx, ty, sx, ty + bz);
        // 右側面(暗)
        col(chh, cll * 0.55);
        tri(sx + bx, ty, sx, ty + bz, sx, ty + bz + d);
        tri(sx + bx, ty, sx + bx, ty + d, sx, ty + bz + d);
        // 左側面(更暗)
        col(chh, cll * 0.38);
        tri(sx - bx, ty, sx, ty + bz, sx, ty + bz + d);
        tri(sx - bx, ty, sx - bx, ty + d, sx, ty + bz + d);
        i = i + 1.0;
    }
    s = s + 1.0;
}

// —— 玩家(鏡頭中心;紅色小方塊 + 頭 + 影子)——
col(0.0, 0.10);
disc(cx, cy - gp * by + bz * 0.4, b * 0.5);
let pgy = cy - py * by;
let pb = b * 0.55;
let pbx = pb * 0.87;
let pbz = pb * 0.5;
let pby = b * 0.9;
let pty = pgy - pby;
col(0.0, 0.55);
tri(cx, pty - pbz, cx + pbx, pty, cx, pty + pbz);
tri(cx, pty - pbz, cx - pbx, pty, cx, pty + pbz);
col(0.0, 0.40);
tri(cx + pbx, pty, cx, pty + pbz, cx, pty + pbz + pby);
tri(cx + pbx, pty, cx + pbx, pty + pby, cx, pty + pbz + pby);
col(0.0, 0.30);
tri(cx - pbx, pty, cx, pty + pbz, cx, pty + pbz + pby);
tri(cx - pbx, pty, cx - pbx, pty + pby, cx, pty + pbz + pby);
col(0.08, 0.62);
disc(cx, pty - pbz - b * 0.28, b * 0.3);

0.0
