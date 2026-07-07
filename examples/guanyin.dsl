// Guanyin (full body) — manifested by 7 primitives
let u = h * 0.01;
let cx = w * 0.5;
let hy = h * 0.22;
let hr = 6.0 * u;
let sy = hy + hr + 3.0 * u;
let ly = h * 0.80;
let sway = sin(t * 1.5) * 1.5 * u;

// body halo (breathing)
hue(0.13);
ring(cx, h * 0.52, 30.0 * u + sin(t) * 1.2 * u);
// head halo
ring(cx, hy, 9.5 * u);

// white robe (celestial garment)
hue(0.55);
line(cx - 1.2 * u, hy + hr, cx - 9.0 * u, sy + 3.0 * u);
line(cx + 1.2 * u, hy + hr, cx + 9.0 * u, sy + 3.0 * u);
line(cx - 9.0 * u, sy + 3.0 * u, cx - 16.0 * u, ly - 2.0 * u);
line(cx + 9.0 * u, sy + 3.0 * u, cx + 16.0 * u, ly - 2.0 * u);
line(cx - 16.0 * u, ly - 2.0 * u, cx + 16.0 * u, ly - 2.0 * u);
// robe folds
arc(cx, ly - 11.0 * u, 8.0 * u, 0.7, 2.44);
arc(cx, ly - 9.0 * u, 9.0 * u, 0.7, 2.44);

// right arm holding a willow branch (viewer's left)
line(cx - 8.5 * u, sy + 4.0 * u, cx - 12.5 * u, sy - 1.0 * u);
// left arm cradling the pure vase (viewer's right)
line(cx + 8.5 * u, sy + 4.0 * u, cx + 11.5 * u, sy + 12.0 * u);

// hands
hue(0.1);
disc(cx - 12.5 * u, sy - 1.5 * u, 1.1 * u);
disc(cx + 11.5 * u, sy + 12.5 * u, 1.1 * u);

// pure vase
hue(0.48);
ring(cx + 11.5 * u, sy + 15.0 * u, 1.0 * u);
disc(cx + 11.5 * u, sy + 18.0 * u, 2.2 * u);

// willow (hanging branch swaying in the wind)
hue(0.32);
line(cx - 12.5 * u, sy - 2.0 * u, cx - 13.5 * u + sway, sy - 11.0 * u);
let k = 0.0;
while k < 3.0 {
    line(cx - 13.2 * u + sway * 0.6, sy - (10.0 - k) * u,
         cx - (15.5 + k * 1.8) * u + sway, sy - (4.0 - k * 2.5) * u);
    k = k + 1.0;
}

// necklace ornament
hue(0.13);
arc(cx, sy + 3.5 * u, 4.5 * u, 0.6, 2.54);

// head
hue(0.1);
disc(cx, hy, hr);
line(cx - hr - 0.8 * u, hy - 0.5 * u, cx - hr - 0.8 * u, hy + 3.0 * u);
line(cx + hr + 0.8 * u, hy - 0.5 * u, cx + hr + 0.8 * u, hy + 3.0 * u);
// hair bun
hue(0.05);
disc(cx, hy - hr - 1.2 * u, 1.9 * u);
// crown (central Amitabha)
hue(0.13);
disc(cx, hy - hr + 0.2 * u, 1.3 * u);
disc(cx - 3.2 * u, hy - hr + 1.2 * u, 0.9 * u);
disc(cx + 3.2 * u, hy - hr + 1.2 * u, 0.9 * u);

// brows/eyes (closed) and a slight smile
hue(0.02);
arc(cx - 2.6 * u, hy - 0.3 * u, 1.5 * u, 3.35, 6.05);
arc(cx + 2.6 * u, hy - 0.3 * u, 1.5 * u, 3.35, 6.05);
arc(cx, hy + 2.0 * u, 2.4 * u + sin(t * 2.0) * 0.2 * u, 0.5, 2.64);
// urna (forehead dot)
disc(cx, hy - 2.2 * u, 0.4 * u);

// lotus throne — two rows of upward petals + waist + pedestal
let px = 0.0;
let ph = 0.0;
let tx = 0.0;

// lower petals (offset, shorter)
hue(0.85);
k = 0.0;
while k < 6.0 {
    px = cx + (k - 2.5) * 4.6 * u;
    ph = 4.2 * u - (k - 2.5) * (k - 2.5) * 0.25 * u;
    tx = cx + (k - 2.5) * 6.0 * u;
    line(px - 2.1 * u, ly + 3.2 * u, tx, ly + 3.2 * u - ph);
    line(px + 2.1 * u, ly + 3.2 * u, tx, ly + 3.2 * u - ph);
    k = k + 1.0;
}

// upper petals (pointed, tallest at center)
hue(0.92);
k = 0.0;
while k < 7.0 {
    px = cx + (k - 3.0) * 4.6 * u;
    ph = 6.5 * u - (k - 3.0) * (k - 3.0) * 0.35 * u;
    tx = cx + (k - 3.0) * 5.8 * u;
    line(px - 2.2 * u, ly + 1.0 * u, tx, ly + 1.0 * u - ph);
    line(px + 2.2 * u, ly + 1.0 * u, tx, ly + 1.0 * u - ph);
    arc(px, ly + 1.0 * u, 2.2 * u, 0.0, 3.1416);
    k = k + 1.0;
}

// waist
hue(0.13);
line(cx - 6.5 * u, ly + 4.8 * u, cx - 4.5 * u, ly + 8.0 * u);
line(cx + 6.5 * u, ly + 4.8 * u, cx + 4.5 * u, ly + 8.0 * u);
// pedestal (two layers)
line(cx - 11.0 * u, ly + 8.0 * u, cx + 11.0 * u, ly + 8.0 * u);
line(cx - 11.0 * u, ly + 8.0 * u, cx - 12.5 * u, ly + 10.5 * u);
line(cx + 11.0 * u, ly + 8.0 * u, cx + 12.5 * u, ly + 10.5 * u);
line(cx - 12.5 * u, ly + 10.5 * u, cx + 12.5 * u, ly + 10.5 * u);

0.0