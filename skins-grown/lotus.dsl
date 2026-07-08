hue(0.92);
let k = 0.0;
while k < 6.0 {
  let a = k * 1.047;
  disc(px + cos(a) * s * 0.5, py + sin(a) * s * 0.5, s * 0.28);
  k = k + 1.0;
}
hue(0.17);
disc(px, py, s * 0.3);
0.0