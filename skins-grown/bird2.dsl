hsl(st(25.0), 0.75, 0.42);
let flap = sin(t * 6.0);
line(px, py, px - s * 4.5, py - flap * s * 3.0);
line(px, py, px + s * 4.5, py - flap * s * 3.0);
disc(px, py, s * 1.2);
hsl(st(25.0), 0.75, 0.6);
disc(px - s * 0.6, py - s * 0.4, s * 0.5);
0.0