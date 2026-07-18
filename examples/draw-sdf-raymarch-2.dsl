// SDF raymarch #2, the harder ask: TWO spheres + soft shadows, same tiny DSL,
// same single shot (249s generation). See draw-sdf-raymarch.dsl for the story.

let a_x = -1.3;
let a_y = 1.0;
let a_z = 0.0;
let a_r = 1.0;
let s_x = 1.4;
let s_y = 1.2;
let s_z = 0.8;
let s_r = 1.2;
let lx = 0.5;
let ly = 0.8;
let lz = -0.4;
let llen = sqrt(lx*lx+ly*ly+lz*lz);
lx = lx/llen;
ly = ly/llen;
lz = lz/llen;
let angle = t*0.15;
let ex = sin(angle)*7.0;
let ey = 3.2;
let ez = cos(angle)*7.0;
let fx = 0.0-ex;
let fy = 1.0-ey;
let fz = 0.0-ez;
let flen = sqrt(fx*fx+fy*fy+fz*fz);
fx = fx/flen;
fy = fy/flen;
fz = fz/flen;
let rx = 0.0-fz;
let ry = 0.0;
let rz = fx;
let rlen = sqrt(rx*rx+rz*rz);
rx = rx/rlen;
rz = rz/rlen;
let upx = 0.0-rz*fy;
let upy = rz*fx-rx*fz;
let upz = rx*fy;
let gw = 72.0;
let gh = 40.0;
let cellw = w/gw;
let cellh = h/gh;
let aspect = w/h;
let fov = 0.65;
let i = 0.0;
let j = 0.0;
let u = 0.0;
let v = 0.0;
let dirx = 0.0;
let diry = 0.0;
let dirz = 0.0;
let dlen = 0.0;
let px = 0.0;
let py = 0.0;
let pz = 0.0;
let stepk = 0.0;
let disttot = 0.0;
let marching = 0.0;
let hittype = 0.0;
let d1 = 0.0;
let d2 = 0.0;
let d3 = 0.0;
let dmin = 0.0;
let nx = 0.0;
let ny = 0.0;
let nz = 0.0;
let diff = 0.0;
let shpx = 0.0;
let shpy = 0.0;
let shpz = 0.0;
let shtravel = 0.0;
let shres = 0.0;
let shmarching = 0.0;
let shk = 0.0;
let shd1 = 0.0;
let shd2 = 0.0;
let shd3 = 0.0;
let shdmin = 0.0;
let bright = 0.0;
let checker = 0.0;
let sx = 0.0;
let sy = 0.0;
let rad = 0.0;
j = 0.0;
while j < gh {
i = 0.0;
while i < gw {
u = ((i+0.5)/gw*2.0-1.0)*fov*aspect;
v = (1.0-(j+0.5)/gh*2.0)*fov;
dirx = fx+u*rx+v*upx;
diry = fy+u*ry+v*upy;
dirz = fz+u*rz+v*upz;
dlen = sqrt(dirx*dirx+diry*diry+dirz*dirz);
dirx = dirx/dlen;
diry = diry/dlen;
dirz = dirz/dlen;
px = ex;
py = ey;
pz = ez;
disttot = 0.0;
marching = 1.0;
hittype = 0.0;
stepk = 0.0;
while stepk < 40.0 {
if marching > 0.5 {
d1 = sqrt((px-a_x)*(px-a_x)+(py-a_y)*(py-a_y)+(pz-a_z)*(pz-a_z))-a_r;
d2 = sqrt((px-s_x)*(px-s_x)+(py-s_y)*(py-s_y)+(pz-s_z)*(pz-s_z))-s_r;
d3 = py;
dmin = min(d1, min(d2, d3));
if dmin < 0.01 {
marching = 0.0;
if d1 <= d2 {
if d1 <= d3 { hittype = 1.0; }
if d1 > d3 { hittype = 3.0; }
}
if d1 > d2 {
if d2 <= d3 { hittype = 2.0; }
if d2 > d3 { hittype = 3.0; }
}
}
if marching > 0.5 {
px = px+dirx*dmin;
py = py+diry*dmin;
pz = pz+dirz*dmin;
disttot = disttot+dmin;
if disttot > 40.0 {
marching = 0.0;
hittype = 0.0;
}
}
}
stepk = stepk+1.0;
}
if hittype > 0.5 {
if hittype < 1.5 {
nx = (px-a_x)/a_r;
ny = (py-a_y)/a_r;
nz = (pz-a_z)/a_r;
}
if hittype > 1.5 {
if hittype < 2.5 {
nx = (px-s_x)/s_r;
ny = (py-s_y)/s_r;
nz = (pz-s_z)/s_r;
}
if hittype > 2.5 {
nx = 0.0;
ny = 1.0;
nz = 0.0;
}
}
diff = nx*lx+ny*ly+nz*lz;
if diff < 0.0 { diff = 0.0; }
shpx = px+nx*0.03;
shpy = py+ny*0.03;
shpz = pz+nz*0.03;
shtravel = 0.05;
shres = 1.0;
shmarching = 1.0;
shk = 0.0;
while shk < 14.0 {
if shmarching > 0.5 {
shd1 = sqrt((shpx+lx*shtravel-a_x)*(shpx+lx*shtravel-a_x)+(shpy+ly*shtravel-a_y)*(shpy+ly*shtravel-a_y)+(shpz+lz*shtravel-a_z)*(shpz+lz*shtravel-a_z))-a_r;
shd2 = sqrt((shpx+lx*shtravel-s_x)*(shpx+lx*shtravel-s_x)+(shpy+ly*shtravel-s_y)*(shpy+ly*shtravel-s_y)+(shpz+lz*shtravel-s_z)*(shpz+lz*shtravel-s_z))-s_r;
shd3 = shpy+ly*shtravel;
shdmin = min(shd1, min(shd2, shd3));
if shdmin < 0.01 {
shres = 0.0;
shmarching = 0.0;
}
if shmarching > 0.5 {
shres = min(shres, 8.0*shdmin/shtravel);
shtravel = shtravel+shdmin;
if shtravel > 12.0 { shmarching = 0.0; }
}
}
shk = shk+1.0;
}
if shres < 0.0 { shres = 0.0; }
bright = 0.18+diff*0.85*shres;
if hittype < 1.5 {
hsl(0.56, 0.78, 0.22+bright*0.42);
}
if hittype > 1.5 {
if hittype < 2.5 {
hsl(0.02, 0.85, 0.22+bright*0.42);
}
if hittype > 2.5 {
checker = floor(px)+floor(pz);
checker = checker-floor(checker/2.0)*2.0;
if checker < 0.5 { hsl(0.0, 0.0, 0.12+bright*0.5); }
if checker > 0.5 { hsl(0.0, 0.0, 0.04+bright*0.4); }
}
}
}
if hittype < 0.5 {
hsl(0.58, 0.5, 0.32+diry*0.3);
}
sx = (i+0.5)*cellw;
sy = (j+0.5)*cellh;
rad = max(cellw, cellh)*0.62;
disc(sx, sy, rad);
i = i+1.0;
}
j = j+1.0;
}
0.0
