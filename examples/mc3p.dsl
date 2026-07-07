// 3D 體素世界 —— 第三人稱跟隨鏡頭(像在裡面玩,看得到身體)
// ← → 轉向、↑ ↓ 前進/後退、Space 跳。
// 真透視投影 + 可旋轉鏡頭 + 由遠到近取樣(畫家演算法)+ 距離霧,全在種子裡。
// 側面用 k 迴圈生成法向量(cos/sin),面可見性 = 法向量朝鏡頭(比較式,無 if)。

let F = h * 1.05;
let cx = w * 0.5;
let cy = h * 0.42;

// —— 狀態 ——
let ini = get(5.0);
let px = get(0.0) * ini + 6.5 * (1.0 - ini);
let pz = get(1.0) * ini + 6.5 * (1.0 - ini);
let py = get(2.0) * ini + 4.0 * (1.0 - ini);
let vy = get(3.0);
let yaw = get(6.0);
let lt = get(4.0);
let dtr = t - lt;
let dt = dtr * (dtr < 0.1) + 0.016 * (dtr >= 0.1);
set(4.0, t);
set(5.0, 1.0);

// —— 轉向與移動 ——
yaw = yaw + (key(1.0) - key(0.0)) * 2.0 * dt;
let fx = sin(yaw);
let fz = cos(yaw);
let mv = (key(2.0) - key(3.0)) * 3.5 * dt;
px = px + fx * mv;
pz = pz + fz * mv;

// —— 腳下地形、跳躍與重力 ——
let gp = 2.4 + sin(px * 0.55) + cos(pz * 0.65) + sin((px + pz) * 0.35) * 0.6;
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
set(6.0, yaw);

// —— 鏡頭:玩家身後上方 ——
let ex = px - fx * 3.2;
let ez = pz - fz * 3.2;
let eh = py + 2.4;
let rx = fz;
let rz = 0.0 - fx;

// —— 地形:鏡頭空間由遠到近取樣 ——
let d = 15.0;
while d > 0.7 {
    let l = 0.0 - 8.0;
    while l < 8.1 {
        let wxx = flr(ex + fx * d + rx * l + 0.5);
        let wzz = flr(ez + fz * d + rz * l + 0.5);
        let g = 2.4 + sin(wxx * 0.55) + cos(wzz * 0.65) + sin((wxx + wzz) * 0.35) * 0.6;
        let lo = (g < 1.7);
        let hi = (g >= 3.4);
        let mid = (1.0 - lo) * (1.0 - hi);
        let chh = 0.12 * lo + 0.33 * mid + 0.55 * hi;
        let fog = 1.0 - d * 0.042;
        let cll = (0.58 * lo + 0.42 * mid + 0.85 * hi) * fog;

        // 四個側面(法向量由 k 生成;只畫朝鏡頭的面)
        let k = 0.0;
        while k < 4.0 {
            let nx = cos(k * 1.5708);
            let nz = sin(k * 1.5708);
            let qx = 0.0 - nz;
            let qz = nx;
            let vv = (((ex - (wxx + nx * 0.5)) * nx + (ez - (wzz + nz * 0.5)) * nz) > 0.0);
            let ax = wxx + nx * 0.5 + qx * 0.5;
            let az = wzz + nz * 0.5 + qz * 0.5;
            let bx = wxx + nx * 0.5 - qx * 0.5;
            let bz = wzz + nz * 0.5 - qz * 0.5;
            let adx = ax - ex;
            let adz = az - ez;
            let azc = adx * fx + adz * fz;
            let av = (azc > 0.3) * vv;
            let azz = azc * (azc > 0.3) + 0.3 * (azc <= 0.3);
            let asx = (cx + F * (adx * rx + adz * rz) / azz) * av;
            let ayt = (cy + F * (eh - g) / azz) * av;
            let ayb = (cy + F * eh / azz) * av;
            let bdx = bx - ex;
            let bdz = bz - ez;
            let bzc = bdx * fx + bdz * fz;
            let bv = (bzc > 0.3) * vv;
            let bzz = bzc * (bzc > 0.3) + 0.3 * (bzc <= 0.3);
            let bsx = (cx + F * (bdx * rx + bdz * rz) / bzz) * bv;
            let byt = (cy + F * (eh - g) / bzz) * bv;
            let byb = (cy + F * eh / bzz) * bv;
            col(chh, cll * 0.5);
            tri(asx, ayt, bsx, byt, bsx, byb);
            tri(asx, ayt, asx, ayb, bsx, byb);
            k = k + 1.0;
        }

        // 頂面(四角投影)
        let p1x = wxx - 0.5;
        let p1z = wzz - 0.5;
        let d1x = p1x - ex;
        let d1z = p1z - ez;
        let z1 = d1x * fx + d1z * fz;
        let v1 = (z1 > 0.3);
        let zz1 = z1 * v1 + 0.3 * (1.0 - v1);
        let s1x = (cx + F * (d1x * rx + d1z * rz) / zz1) * v1;
        let s1y = (cy + F * (eh - g) / zz1) * v1;
        let d2x = p1x + 1.0 - ex;
        let d2z = d1z;
        let z2 = d2x * fx + d2z * fz;
        let v2 = (z2 > 0.3);
        let zz2 = z2 * v2 + 0.3 * (1.0 - v2);
        let s2x = (cx + F * (d2x * rx + d2z * rz) / zz2) * v2;
        let s2y = (cy + F * (eh - g) / zz2) * v2;
        let d3x = d2x;
        let d3z = d1z + 1.0;
        let z3 = d3x * fx + d3z * fz;
        let v3 = (z3 > 0.3);
        let zz3 = z3 * v3 + 0.3 * (1.0 - v3);
        let s3x = (cx + F * (d3x * rx + d3z * rz) / zz3) * v3;
        let s3y = (cy + F * (eh - g) / zz3) * v3;
        let d4x = d1x;
        let d4z = d3z;
        let z4 = d4x * fx + d4z * fz;
        let v4 = (z4 > 0.3);
        let zz4 = z4 * v4 + 0.3 * (1.0 - v4);
        let s4x = (cx + F * (d4x * rx + d4z * rz) / zz4) * v4;
        let s4y = (cy + F * (eh - g) / zz4) * v4;
        col(chh, cll);
        tri(s1x, s1y, s2x, s2y, s3x, s3y);
        tri(s1x, s1y, s4x, s4y, s3x, s3y);

        l = l + 0.8;
    }
    d = d - 0.8;
}

// —— 玩家身體(鏡頭正前方 3.2,看得到自己)——
// 影子(投在腳下地形上,跳躍時分離)
col(0.0, 0.08);
disc(cx, cy + F * (eh - gp) / 3.2, F * 0.10);
// 身體(紅色方塊人)
let bw = F * 0.085;
let byt2 = cy + F * (eh - py - 1.05) / 3.2;
let byb2 = cy + F * (eh - py) / 3.2;
col(0.0, 0.48);
tri(cx - bw, byt2, cx + bw, byt2, cx + bw, byb2);
tri(cx - bw, byt2, cx - bw, byb2, cx + bw, byb2);
col(0.0, 0.36);
tri(cx - bw, byt2, cx + bw, byt2, cx, byt2 - bw * 0.6);
// 頭
col(0.08, 0.60);
disc(cx, byt2 - bw * 0.85, bw * 0.62);

0.0
