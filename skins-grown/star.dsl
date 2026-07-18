let tw = 0.55 + 0.45 * sin(t * 3.2);
hue(0.15);
let k = 0.0;
while k < 4.0 {
  let a = k * 0.785;
  line(px - cos(a) * s * tw, py - sin(a) * s * tw, px + cos(a) * s * tw, py + sin(a) * s * tw);
  k = k + 1.0;
}
hue(0.14);
disc(px, py, s * 0.18 * tw);
0.0