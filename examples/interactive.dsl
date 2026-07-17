// §21 the interaction loop — a drawing you can TOUCH.
// A comet chases your cursor and trails motes it REMEMBERS in the host data
// root (get/set). The host owns the pointer and the memory; this cell only
// sees mx()/my()/down() and 32 f64 slots. Reach fixed, richness unbounded:
// it still cannot fetch, read the page, or capture the mouse anywhere else.
//
// State layout in the data root:
//   slot 0,1   = the eased head position (x,y)
//   slot 2..13 = a 6-point ring buffer of past head positions (x,y pairs)
//   slot 20    = the ring-buffer write head
//
// Try it, then hot-patch the LOOK in place with a `~` edit — the trail (the
// accumulated state) survives the swap, because the data root is the host's.

let trail = 6.0;

// where the head wants to go: your cursor, or a lazy drift when you're away
let tx = mx();
let ty = my();
if tx < 0.0 {
  tx = w * 0.5 + sin(t * 0.6) * w * 0.30;
  ty = h * 0.5 + cos(t * 0.8) * h * 0.30;
}

// ease the remembered head toward the target (inertia = a felt weight)
let hx = get(0.0);
let hy = get(1.0);
hx = hx + (tx - hx) * 0.14;
hy = hy + (ty - hy) * 0.14;
set(0.0, hx);
set(1.0, hy);

// push the head into the ring buffer and advance the write index
let idx = get(20.0);
set(2.0 + idx * 2.0, hx);
set(3.0 + idx * 2.0, hy);
idx = idx + 1.0;
if idx >= trail { idx = 0.0; }
set(20.0, idx);

// draw the remembered trail: older motes fainter and smaller
let k = 0.0;
while k < trail {
  let px = get(2.0 + k * 2.0);
  let py = get(3.0 + k * 2.0);
  let age = k / trail;
  hsl(0.58 + age * 0.08, 0.70, 0.40 + age * 0.20);
  disc(px, py, 2.0 + age * 6.0);
  k = k + 1.0;
}

// the head: brighter, a soft glow, and a squeeze while you press
let r = 10.0;
if down() > 0.5 { r = 16.0; }
hsl(0.56, 0.85, 0.70);
glow(hx, hy, 44.0);
disc(hx, hy, r);
0.0
