// 3D voxel world — third-person chase camera (like you're inside, but you see your body)
// ← → turn, ↑ ↓ forward/back, Space jump.
// true perspective projection + a rotatable camera + far-to-near sampling (painter's algorithm) + distance fog, all inside the seed.
// v2: rewritten after the DSL upgrade — if/else, %, built-in min/max/abs/sqrt/floor;
//     the old "comparison-as-0/1-multiplier" trick is now history.

let F = h * 1.05;
let cx = w * 0.5;
let cy = h * 0.42;

// —— state ——
let px = get(0.0);
let pz = get(1.0);
let py = get(2.0);
let vy = get(3.0);
let yaw = get(6.0);
if get(5.0) < 0.5 {
    px = 6.5;
    pz = 6.5;
    py = 4.0;
    set(5.0, 1.0);
}
let dtr = t - get(4.0);
let dt = 0.016;
if dtr < 0.1 {
    dt = dtr;
}
set(4.0, t);

// —— turn and move ——
yaw = yaw + (key(1.0) - key(0.0)) * 2.0 * dt;
let fx = sin(yaw);
let fz = cos(yaw);
let mv = (key(2.0) - key(3.0)) * 3.5 * dt;
px = px + fx * mv;
pz = pz + fz * mv;

// —— terrain underfoot, jump and gravity ——
let gp = 2.4 + sin(px * 0.55) + cos(pz * 0.65) + sin((px + pz) * 0.35) * 0.6;
if py <= gp + 0.05 {
    if key(4.0) > 0.5 {
        vy = 7.5;
    }
}
vy = vy - 20.0 * dt;
py = py + vy * dt;
if py < gp {
    py = gp;
    vy = 0.0;
}
set(0.0, px);
set(1.0, pz);
set(2.0, py);
set(3.0, vy);
set(6.0, yaw);

// —— camera: above and behind the player ——
let ex = px - fx * 3.2;
let ez = pz - fz * 3.2;
let eh = py + 2.4;
let rx = fz;
let rz = 0.0 - fx;

// —— terrain: sampled far-to-near in camera space ——
let d = 15.0;
while d > 0.7 {
    let l = 0.0 - 8.0;
    while l < 8.1 {
        let wxx = floor(ex + fx * d + rx * l + 0.5);
        let wzz = floor(ez + fz * d + rz * l + 0.5);
        let g = 2.4 + sin(wxx * 0.55) + cos(wzz * 0.65) + sin((wxx + wzz) * 0.35) * 0.6;
        let lo = (g < 1.7);
        let hi = (g >= 3.4);
        let chh = 0.12 * lo + 0.33 * (1.0 - lo) * (1.0 - hi) + 0.55 * hi;
        let cll = (0.58 * lo + 0.42 * (1.0 - lo) * (1.0 - hi) + 0.85 * hi) * (1.0 - d * 0.042);

        // four side faces: draw only the ones facing the camera (an if now, no longer a 0/1 multiplier)
        let k = 0.0;
        while k < 4.0 {
            let nx = cos(k * 1.5708);
            let nz = sin(k * 1.5708);
            if (ex - (wxx + nx * 0.5)) * nx + (ez - (wzz + nz * 0.5)) * nz > 0.0 {
                let qx = 0.0 - nz;
                let qz = nx;
                let ax = wxx + nx * 0.5 + qx * 0.5;
                let az = wzz + nz * 0.5 + qz * 0.5;
                let bx = wxx + nx * 0.5 - qx * 0.5;
                let bz = wzz + nz * 0.5 - qz * 0.5;
                let adx = ax - ex;
                let adz = az - ez;
                let azc = adx * fx + adz * fz;
                let bdx = bx - ex;
                let bdz = bz - ez;
                let bzc = bdx * fx + bdz * fz;
                if min(azc, bzc) > 0.3 {
                    let asx = cx + F * (adx * rx + adz * rz) / azc;
                    let ayt = cy + F * (eh - g) / azc;
                    let ayb = cy + F * eh / azc;
                    let bsx = cx + F * (bdx * rx + bdz * rz) / bzc;
                    let byt = cy + F * (eh - g) / bzc;
                    let byb = cy + F * eh / bzc;
                    col(chh, cll * 0.5);
                    tri(asx, ayt, bsx, byt, bsx, byb);
                    tri(asx, ayt, asx, ayb, bsx, byb);
                }
            }
            k = k + 1.0;
        }

        // top face: drawn only when all four corners pass the near plane
        let d1x = wxx - 0.5 - ex;
        let d1z = wzz - 0.5 - ez;
        let d3x = d1x + 1.0;
        let d3z = d1z + 1.0;
        let z1 = d1x * fx + d1z * fz;
        let z2 = d3x * fx + d1z * fz;
        let z3 = d3x * fx + d3z * fz;
        let z4 = d1x * fx + d3z * fz;
        if min(min(z1, z2), min(z3, z4)) > 0.3 {
            let s1x = cx + F * (d1x * rx + d1z * rz) / z1;
            let s1y = cy + F * (eh - g) / z1;
            let s2x = cx + F * (d3x * rx + d1z * rz) / z2;
            let s2y = cy + F * (eh - g) / z2;
            let s3x = cx + F * (d3x * rx + d3z * rz) / z3;
            let s3y = cy + F * (eh - g) / z3;
            let s4x = cx + F * (d1x * rx + d3z * rz) / z4;
            let s4y = cy + F * (eh - g) / z4;
            col(chh, cll);
            tri(s1x, s1y, s2x, s2y, s3x, s3y);
            tri(s1x, s1y, s4x, s4y, s3x, s3y);
        }

        l = l + 0.8;
    }
    d = d - 0.8;
}

// —— the player's body (3.2 in front of the camera; you can see yourself) ——
col(0.0, 0.08);
disc(cx, cy + F * (eh - gp) / 3.2, F * 0.10);
let bw = F * 0.085;
let byt2 = cy + F * (eh - py - 1.05) / 3.2;
let byb2 = cy + F * (eh - py) / 3.2;
col(0.0, 0.48);
tri(cx - bw, byt2, cx + bw, byt2, cx + bw, byb2);
tri(cx - bw, byt2, cx - bw, byb2, cx + bw, byb2);
col(0.0, 0.36);
tri(cx - bw, byt2, cx + bw, byt2, cx, byt2 - bw * 0.6);
col(0.08, 0.60);
disc(cx, byt2 - bw * 0.85, bw * 0.62);

0.0
