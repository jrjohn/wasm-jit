// §22 full-suite demo — a windmill on a checkered plain, at golden hour.
// Structure from the transform stack (hierarchy without matrices), matter from
// shine/lum/pat, shadows and the orbit camera from host law (drag to look).

// checkered ground
pat(1.0);
rgb(0.42, 0.5, 0.42);
box(0.0, 0.0 - 0.5, 0.0, 44.0, 1.0, 44.0);
pat(0.0);

// the tower: a cone standing at the origin
rgb(0.72, 0.64, 0.52);
cone(2.4, 9.5);

// the rotor: one moving frame, four blades hanging off it
push();
move(0.0, 8.2, 1.4);
rotz(t * 1.6);
let k = 0.0;
while k < 4.0 {
  push();
  rotz(k * 1.5708);
  rgb(0.92, 0.92, 0.96);
  shine(0.5);
  box(0.0, 2.8, 0.0, 0.75, 5.6, 0.14);
  shine(0.15);
  pop();
  k = k + 1.0;
}
// the hub
shine(0.85);
rgb(0.75, 0.78, 0.85);
sphere(0.0, 0.0, 0.0, 0.55);
shine(0.15);
pop();

// a lantern by the path — self-luminous, it ignores the sun
lum(0.95);
hsl(0.11, 0.95, 0.62);
sphere(6.5, 1.1, 5.0, 0.55);
lum(0.0);
rgb(0.3, 0.26, 0.2);
cyl(0.12, 1.1);

// a metal sphere to catch the highlight
shine(0.95);
rgb(0.6, 0.65, 0.75);
sphere(0.0 - 6.0, 1.2, 4.0, 1.2);
0.0
