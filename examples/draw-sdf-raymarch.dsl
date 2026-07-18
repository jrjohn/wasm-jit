// SDF raymarch, handwritten by claude-sonnet-5 in the tiny seed DSL — evidence
// from the L4 falsification experiment (2026-07-18). One ask, one shot, zero
// self-repair: a sphere on a checkered plane, one light, marched ~40 steps per
// sample over a coarse disc grid. The claim "the model can't write vector math
// in a no-vec3, no-function DSL" died here; what survived is the cost (245s of
// generation — 8x a normal draw). Kept as proof of what the fence can hold.

let eye_x = 0.0;
let eye_y = 2.5;
let eye_z = -6.0;
let fw_x = 0.0;
let fw_y = -0.3162;
let fw_z = 0.9487;
let right_x = 1.0;
let right_y = 0.0;
let right_z = 0.0;
let up_x = 0.0;
let up_y = 0.9487;
let up_z = 0.3162;
let light_x = 0.4082;
let light_y = 0.8165;
let light_z = -0.4082;
let sphere_cx = 0.0;
let sphere_cy = 1.0;
let sphere_cz = 0.0;
let sphere_r = 1.0;
let plane_y = 0.0;
let grid_w = 72.0;
let grid_h = 40.0;
let aspect = w / max(h, 1.0);
let cell_w = w / grid_w;
let cell_h = h / grid_h;
let disc_r = max(cell_w, cell_h) * 0.65;
let fov = 0.62;
let eps = 0.05;
let iy = 0.0;
while iy < grid_h {
  let ix = 0.0;
  while ix < grid_w {
    let screen_x = ((ix + 0.5) / grid_w * 2.0 - 1.0) * aspect * fov;
    let screen_y = (1.0 - (iy + 0.5) / grid_h * 2.0) * fov;
    let dir_x = fw_x + right_x * screen_x + up_x * screen_y;
    let dir_y = fw_y + right_y * screen_x + up_y * screen_y;
    let dir_z = fw_z + right_z * screen_x + up_z * screen_y;
    let dir_len = sqrt(dir_x * dir_x + dir_y * dir_y + dir_z * dir_z);
    dir_x = dir_x / dir_len;
    dir_y = dir_y / dir_len;
    dir_z = dir_z / dir_len;
    let ray_t = 0.0;
    let steps = 0.0;
    let hit_flag = 0.0;
    while steps < 40.0 {
      if hit_flag < 0.5 {
        let px = eye_x + dir_x * ray_t;
        let py = eye_y + dir_y * ray_t;
        let pz = eye_z + dir_z * ray_t;
        let sphere_d = sqrt((px - sphere_cx) * (px - sphere_cx) + (py - sphere_cy) * (py - sphere_cy) + (pz - sphere_cz) * (pz - sphere_cz)) - sphere_r;
        let plane_d = py - plane_y;
        let scene_d = min(sphere_d, plane_d);
        if scene_d < 0.001 {
          hit_flag = 1.0;
        } else {
          ray_t = ray_t + scene_d;
        }
      }
      steps = steps + 1.0;
    }
    let color_r = 0.0;
    let color_g = 0.0;
    let color_b = 0.0;
    if hit_flag > 0.5 {
      let hit_x = eye_x + dir_x * ray_t;
      let hit_y = eye_y + dir_y * ray_t;
      let hit_z = eye_z + dir_z * ray_t;
      let px1 = hit_x + eps;
      let sd1 = sqrt((px1 - sphere_cx) * (px1 - sphere_cx) + (hit_y - sphere_cy) * (hit_y - sphere_cy) + (hit_z - sphere_cz) * (hit_z - sphere_cz)) - sphere_r;
      let pd1 = hit_y - plane_y;
      let d1 = min(sd1, pd1);
      let px2 = hit_x - eps;
      let sd2 = sqrt((px2 - sphere_cx) * (px2 - sphere_cx) + (hit_y - sphere_cy) * (hit_y - sphere_cy) + (hit_z - sphere_cz) * (hit_z - sphere_cz)) - sphere_r;
      let d2 = min(sd2, pd1);
      let n_x = d1 - d2;
      let py3 = hit_y + eps;
      let sd3 = sqrt((hit_x - sphere_cx) * (hit_x - sphere_cx) + (py3 - sphere_cy) * (py3 - sphere_cy) + (hit_z - sphere_cz) * (hit_z - sphere_cz)) - sphere_r;
      let pd3 = py3 - plane_y;
      let d3 = min(sd3, pd3);
      let py4 = hit_y - eps;
      let sd4 = sqrt((hit_x - sphere_cx) * (hit_x - sphere_cx) + (py4 - sphere_cy) * (py4 - sphere_cy) + (hit_z - sphere_cz) * (hit_z - sphere_cz)) - sphere_r;
      let pd4 = py4 - plane_y;
      let d4 = min(sd4, pd4);
      let n_y = d3 - d4;
      let pz5 = hit_z + eps;
      let sd5 = sqrt((hit_x - sphere_cx) * (hit_x - sphere_cx) + (hit_y - sphere_cy) * (hit_y - sphere_cy) + (pz5 - sphere_cz) * (pz5 - sphere_cz)) - sphere_r;
      let d5 = min(sd5, pd1);
      let pz6 = hit_z - eps;
      let sd6 = sqrt((hit_x - sphere_cx) * (hit_x - sphere_cx) + (hit_y - sphere_cy) * (hit_y - sphere_cy) + (pz6 - sphere_cz) * (pz6 - sphere_cz)) - sphere_r;
      let d6 = min(sd6, pd1);
      let n_z = d5 - d6;
      let n_len = max(sqrt(n_x * n_x + n_y * n_y + n_z * n_z), 0.0001);
      n_x = n_x / n_len;
      n_y = n_y / n_len;
      n_z = n_z / n_len;
      let diff = max(n_x * light_x + n_y * light_y + n_z * light_z, 0.0);
      let shade = 0.15 + diff * 0.85;
      let sphere_dhit = sqrt((hit_x - sphere_cx) * (hit_x - sphere_cx) + (hit_y - sphere_cy) * (hit_y - sphere_cy) + (hit_z - sphere_cz) * (hit_z - sphere_cz)) - sphere_r;
      let plane_dhit = hit_y - plane_y;
      let is_sphere = sphere_dhit < plane_dhit;
      let chk = floor(hit_x * 0.5) + floor(hit_z * 0.5);
      let chk_mod = chk - 2.0 * floor(chk / 2.0);
      let base_r = 0.0;
      let base_g = 0.0;
      let base_b = 0.0;
      if is_sphere > 0.5 {
        base_r = 0.85;
        base_g = 0.15;
        base_b = 0.15;
      } else {
        if chk_mod < 0.5 {
          base_r = 0.82;
          base_g = 0.82;
          base_b = 0.82;
        } else {
          base_r = 0.18;
          base_g = 0.18;
          base_b = 0.2;
        }
      }
      color_r = base_r * shade;
      color_g = base_g * shade;
      color_b = base_b * shade;
    } else {
      let mix_k = (dir_y + 1.0) * 0.5;
      color_r = 0.75 - 0.4 * mix_k;
      color_g = 0.85 - 0.3 * mix_k;
      color_b = 0.95 - 0.1 * mix_k;
    }
    rgb(color_r, color_g, color_b);
    let screen_px_x = (ix + 0.5) * cell_w;
    let screen_px_y = (iy + 0.5) * cell_h;
    disc(screen_px_x, screen_px_y, disc_r);
    ix = ix + 1.0;
  }
  iy = iy + 1.0;
}
0.0
