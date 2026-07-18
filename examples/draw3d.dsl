// §22 the seed writes the SCENE — 3D composed the way 2D always was, one
// dimension up. This cell places primitives in world coordinates each frame;
// the host owns the camera, projection, depth and light (a seed can never
// write a matrix, so the model never has to). y is up.

// a slow orbit around the grove
cam(cos(t * 0.25) * 17.0, 8.5, sin(t * 0.25) * 17.0, 0.0, 1.5, 0.0);

// ground: one wide flat box
rgb(0.16, 0.2, 0.16);
box(0.0, 0.0 - 0.6, 0.0, 34.0, 1.2, 34.0);

// a ring of twelve breathing spheres, each its own hue
let k = 0.0;
while k < 12.0 {
  let a = k * 0.5236;
  hsl(k / 12.0, 0.65, 0.55);
  sphere(cos(a) * 8.0, 2.0 + sin(t * 1.3 + k) * 0.7, sin(a) * 8.0, 0.8 + 0.2 * sin(t + k));
  k = k + 1.0;
}

// a tree: box trunk + three sphere crowns
rgb(0.36, 0.25, 0.16);
box(0.0, 2.0, 0.0, 0.9, 4.0, 0.9);
hsl(0.33, 0.5, 0.4);
sphere(0.0, 5.2, 0.0, 2.4);
sphere(1.4, 4.4, 0.6, 1.5);
sphere(0.0 - 1.3, 4.5, 0.0 - 0.5, 1.6);
0.0
