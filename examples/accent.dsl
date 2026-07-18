// accent cell — computes the PAGE'S accent colour from time. Its entire world
// is sin, cos, hsl. It cannot fetch, cannot read the page, cannot set any style
// but the ONE colour it emits — the host routes that emitted colour to the CSS
// custom property --accent. Richness (any computed hue over time) is unbounded;
// reach (only "here is a colour") is fixed. The host hands the theme in via w
// (>0.5 = dark), the same way it hands the reseed condition into the sky.
let dark = 0.0;
if w > 0.5 { dark = 1.0; }

// hue drifts gently around water-blue; saturation breathes
let hu = 0.547 + 0.05 * sin(t * 0.16);
let sa = 0.60 + 0.06 * sin(t * 0.11);

// lightness sits where the theme needs it, with a slow shimmer
let li = 0.40;
if dark > 0.5 { li = 0.62; }
li = li + 0.03 * sin(t * 0.20);

hsl(hu, sa, li);

0.0
