use std::f64::consts::TAU;
use std::{env, fs, process};

const WIDTH: usize = 44;
const HEIGHT: usize = 44;
const ROWS: usize = HEIGHT / 2;
const FRAMES: usize = 240;
const OUT_FRAMES: usize = 240;
const SAMPLES_X: usize = 4;
const SAMPLES_Y: usize = 8;
const THICKNESS: f64 = 0.22;
const FILL_GAMMA: f64 = 0.7;
const RIM_LIGHT: f64 = 0.7;
const MID_LIGHT: f64 = 0.88;
const BLUR: usize = 5;
const EDGE_STEP: f64 = 1.2;
const EDGE_SHARE: f64 = 0.2;
const EDGE_COHERENCE: f64 = 0.7;
const EDGE_COVER: f64 = 0.15;
const SCALE: f64 = 0.86;
const BEVEL: f64 = 6.0;
const REACH: i32 = 9;
const CORE: f64 = 12.0;
const MID: f64 = 2.0;
const RIM: f64 = 4.0;
const STEPS: usize = 56;
const COVER: f64 = 0.38;

type V3 = [f64; 3];

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt().max(1e-12);
    [a[0] / l, a[1] / l, a[2] / l]
}

fn mul(m: &[V3; 3], v: V3) -> V3 {
    [dot(m[0], v), dot(m[1], v), dot(m[2], v)]
}

fn transpose(m: &[V3; 3]) -> [V3; 3] {
    [[m[0][0], m[1][0], m[2][0]], [m[0][1], m[1][1], m[2][1]], [m[0][2], m[1][2], m[2][2]]]
}

fn matmul(a: &[V3; 3], b: &[V3; 3]) -> [V3; 3] {
    let t = transpose(b);
    [
        [dot(a[0], t[0]), dot(a[0], t[1]), dot(a[0], t[2])],
        [dot(a[1], t[0]), dot(a[1], t[1]), dot(a[1], t[2])],
        [dot(a[2], t[0]), dot(a[2], t[1]), dot(a[2], t[2])],
    ]
}

fn rot_x(a: f64) -> [V3; 3] {
    [[1.0, 0.0, 0.0], [0.0, a.cos(), -a.sin()], [0.0, a.sin(), a.cos()]]
}

fn rot_y(a: f64) -> [V3; 3] {
    [[a.cos(), 0.0, a.sin()], [0.0, 1.0, 0.0], [-a.sin(), 0.0, a.cos()]]
}

struct Mask {
    n: usize,
    id: Vec<u8>,
    dist: Vec<f32>,
}

impl Mask {
    fn load(path: &str) -> Mask {
        let img = image::open(path).unwrap_or_else(|e| {
            eprintln!("{path}: {e}");
            process::exit(1)
        });
        let img = img.to_rgba8();
        let n = img.width().min(img.height()) as usize;
        let mut id = vec![0u8; n * n];
        for y in 0..n {
            for x in 0..n {
                let p = img.get_pixel(x as u32, y as u32).0;
                if p[3] >= 128 {
                    let sum = p[0] as u32 + p[1] as u32 + p[2] as u32;
                    id[y * n + x] = if sum > 450 { 1 } else { 2 };
                }
            }
        }
        let mut dist = vec![0f32; n * n];
        for y in 0..n as i32 {
            for x in 0..n as i32 {
                let own = id[y as usize * n + x as usize];
                if own == 0 {
                    continue;
                }
                let mut best = (REACH * REACH) as f32;
                for dy in -REACH..=REACH {
                    for dx in -REACH..=REACH {
                        let (px, py) = (x + dx, y + dy);
                        let other = if px < 0 || py < 0 || px >= n as i32 || py >= n as i32 {
                            0
                        } else {
                            id[py as usize * n + px as usize]
                        };
                        if other != own {
                            best = best.min((dx * dx + dy * dy) as f32);
                        }
                    }
                }
                dist[y as usize * n + x as usize] = best.sqrt();
            }
        }
        Mask { n, id, dist }
    }

    fn id_at(&self, u: f64, v: f64) -> u8 {
        let x = ((u + 0.5) * self.n as f64).floor();
        let y = ((v + 0.5) * self.n as f64).floor();
        if x < 0.0 || y < 0.0 || x >= self.n as f64 || y >= self.n as f64 {
            return 0;
        }
        self.id[y as usize * self.n + x as usize]
    }

    fn dist_at(&self, u: f64, v: f64) -> f64 {
        let fx = ((u + 0.5) * self.n as f64 - 0.5).clamp(0.0, self.n as f64 - 1.001);
        let fy = ((v + 0.5) * self.n as f64 - 0.5).clamp(0.0, self.n as f64 - 1.001);
        let (x, y) = (fx as usize, fy as usize);
        let (ax, ay) = (fx - x as f64, fy - y as f64);
        let at = |x: usize, y: usize| self.dist[y * self.n + x] as f64;
        let top = at(x, y) * (1.0 - ax) + at(x + 1, y) * ax;
        let bottom = at(x, y + 1) * (1.0 - ax) + at(x + 1, y + 1) * ax;
        top * (1.0 - ay) + bottom * ay
    }

    fn slope(&self, u: f64, v: f64) -> (f64, f64) {
        let e = 1.5 / self.n as f64;
        (
            (self.dist_at(u + e, v) - self.dist_at(u - e, v)) / 3.0,
            (self.dist_at(u, v + e) - self.dist_at(u, v - e)) / 3.0,
        )
    }
}

fn axis_turn(k: V3, a: f64) -> [V3; 3] {
    let (c, s, t) = (a.cos(), a.sin(), 1.0 - a.cos());
    [
        [t * k[0] * k[0] + c, t * k[0] * k[1] - s * k[2], t * k[0] * k[2] + s * k[1]],
        [t * k[0] * k[1] + s * k[2], t * k[1] * k[1] + c, t * k[1] * k[2] - s * k[0]],
        [t * k[0] * k[2] - s * k[1], t * k[1] * k[2] + s * k[0], t * k[2] * k[2] + c],
    ]
}

fn rot_z(a: f64) -> [V3; 3] {
    [[a.cos(), -a.sin(), 0.0], [a.sin(), a.cos(), 0.0], [0.0, 0.0, 1.0]]
}

fn hash(x: i64, y: i64) -> f64 {
    let mut h = (x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263)) as u64;
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) & 0xffff) as f64 / 65535.0
}

fn value_noise(x: f64, y: f64) -> f64 {
    let (ix, iy) = (x.floor(), y.floor());
    let (fx, fy) = (x - ix, y - iy);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (ix, iy) = (ix as i64, iy as i64);
    let top = hash(ix, iy) * (1.0 - sx) + hash(ix + 1, iy) * sx;
    let bottom = hash(ix, iy + 1) * (1.0 - sx) + hash(ix + 1, iy + 1) * sx;
    top * (1.0 - sy) + bottom * sy
}

fn grain(u: f64, v: f64) -> f64 {
    let hatch = 0.5 + 0.5 * ((u + v) * TAU * 17.0).sin();
    0.5 * value_noise(u * 10.0 + 40.0, v * 10.0 + 40.0) + 0.3 * value_noise(u * 23.0 + 9.0, v * 23.0 + 9.0) + 0.2 * hatch
}

struct Hit {
    id: u8,
    face: bool,
    light: f64,
}

fn trace(mask: &Mask, rot: &[V3; 3], inv: &[V3; 3], x: f64, y: f64) -> Option<Hit> {
    let origin = mul(inv, [x, y, 1.0]);
    let dir = mul(inv, [0.0, 0.0, -1.0]);
    let half = [0.5, 0.5, THICKNESS / 2.0];
    let (mut near, mut far) = (0.0f64, 2.0f64);
    let mut entry = 2usize;
    for axis in 0..3 {
        if dir[axis].abs() < 1e-9 {
            if origin[axis].abs() > half[axis] {
                return None;
            }
            continue;
        }
        let a = (-half[axis] - origin[axis]) / dir[axis];
        let b = (half[axis] - origin[axis]) / dir[axis];
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        if lo > near {
            near = lo;
            entry = axis;
        }
        far = far.min(hi);
    }
    if near >= far {
        return None;
    }
    let at = |t: f64| [origin[0] + dir[0] * t, origin[1] + dir[1] * t, origin[2] + dir[2] * t];
    let step = (far - near) / STEPS as f64;
    let mut found = None;
    for i in 0..=STEPS {
        let t = near + step * i as f64;
        let p = at(t);
        if mask.id_at(p[0], p[1]) != 0 {
            found = Some((i, t));
            break;
        }
    }
    let (index, mut t) = found?;
    let face = index == 0 && entry == 2;
    if index > 0 {
        let (mut lo, mut hi) = (t - step, t);
        for _ in 0..10 {
            let mid = (lo + hi) / 2.0;
            let p = at(mid);
            if mask.id_at(p[0], p[1]) != 0 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        t = hi;
    }
    let p = at(t);
    let id = mask.id_at(p[0], p[1]);
    let (gx, gy) = mask.slope(p[0], p[1]);
    let e = 1.0 / mask.n as f64;
    let texture = grain(p[0], p[1]);
    let (normal, albedo) = if face {
        let side = if p[2] >= 0.0 { 1.0 } else { -1.0 };
        let depth = mask.dist_at(p[0], p[1]);
        let edge = if depth < BEVEL { 1.5 * (1.0 - depth / BEVEL).powf(0.6) } else { 0.0 };
        let bx = (grain(p[0] + e, p[1]) - grain(p[0] - e, p[1])) / (2.0 * e) * 0.018;
        let by = (grain(p[0], p[1] + e) - grain(p[0], p[1] - e)) / (2.0 * e) * 0.018;
        (norm([-gx * edge - bx, -gy * edge - by, side]), 0.70 + 0.60 * texture)
    } else {
        let n = if gx.abs() + gy.abs() < 1e-6 { [-dir[0], -dir[1], 0.0] } else { [-gx, -gy, 0.0] };
        (norm(n), 0.75 + 0.20 * grain(p[0] + p[2] * 3.1, p[1] - p[2] * 2.3))
    };
    let n = mul(rot, normal);
    let light = norm([-0.46, -0.58, 0.67]);
    let halfway = norm([light[0], light[1], light[2] + 1.0]);
    let diffuse = dot(n, light).max(0.0);
    let shine = dot(n, halfway).max(0.0).powf(26.0);
    let rim = (1.0 - n[2].abs()).powf(3.0) * 0.10;
    Some(Hit { id, face, light: 0.16 + 0.78 * diffuse * albedo + 0.62 * shine + rim })
}

fn old_cells(data: &[u8], frame: usize) -> Vec<u8> {
    let at = 4 + frame * WIDTH * HEIGHT;
    let mut cells = vec![0u8; WIDTH * ROWS];
    for row in 0..ROWS {
        for col in 0..WIDTH {
            let v = data[at + row * 2 * WIDTH + col].max(data[at + (row * 2 + 1) * WIDTH + col]);
            cells[row * WIDTH + col] = if v & 15 == 0 { 0 } else if v < 16 { 1 } else { 2 };
        }
    }
    cells
}

fn score(mask: &Mask, rot: &[V3; 3], want: &[u8]) -> f64 {
    let inv = transpose(rot);
    let (mut same, mut total) = (0.0f64, 0.0f64);
    for row in 0..ROWS {
        for col in 0..WIDTH {
            let x = ((col as f64 + 0.5) / WIDTH as f64 - 0.5) / SCALE;
            let y = ((row as f64 + 0.5) / ROWS as f64 - 0.5) / SCALE;
            let got = trace(mask, rot, &inv, x, y).map(|h| h.id).unwrap_or(0);
            let old = want[row * WIDTH + col];
            if got != 0 || old != 0 {
                total += 1.0;
                if got == old {
                    same += 1.0;
                } else if got != 0 && old != 0 {
                    same += 0.35;
                }
            }
        }
    }
    if total == 0.0 { 0.0 } else { same / total }
}

fn refine(mask: &Mask, start: [V3; 3], want: &[u8], steps: &[(f64, i32)]) -> ([V3; 3], f64) {
    let mut best = start;
    let mut top = score(mask, &best, want);
    for &(step, span) in steps {
        let base = best;
        for a in -span..=span {
            for b in -span..=span {
                for c in -span..=span {
                    if a == 0 && b == 0 && c == 0 {
                        continue;
                    }
                    let delta = matmul(&matmul(&rot_x((a as f64 * step).to_radians()), &rot_y((b as f64 * step).to_radians())), &rot_z((c as f64 * step).to_radians()));
                    let cand = matmul(&delta, &base);
                    let s = score(mask, &cand, want);
                    if s > top {
                        top = s;
                        best = cand;
                    }
                }
            }
        }
    }
    (best, top)
}

fn locate(mask: &Mask, want: &[u8]) -> [V3; 3] {
    let mut best = rot_x(0.0);
    let mut top = -1.0;
    for a in 0..24 {
        for b in 0..24 {
            for c in 0..8 {
                let cand = matmul(&matmul(&rot_y((b as f64 * 15.0).to_radians()), &rot_x((a as f64 * 15.0).to_radians())), &rot_z((c as f64 * 15.0).to_radians()));
                let s = score(mask, &cand, want);
                if s > top {
                    top = s;
                    best = cand;
                }
            }
        }
    }
    refine(mask, best, want, &[(5.0, 2), (2.0, 2), (1.0, 2)]).0
}

fn fit(mask: &Mask, old: &[u8]) {
    let mut rot = locate(mask, &old_cells(old, 0));
    for frame in 0..FRAMES {
        let want = old_cells(old, frame);
        let (mut next, mut s) = refine(mask, rot, &want, &[(3.0, 3), (1.0, 2), (0.5, 1)]);
        if s < 0.84 {
            let (again, t) = refine(mask, locate(mask, &want), &want, &[(1.0, 2), (0.5, 1)]);
            if t > s + 0.02 {
                next = again;
                s = t;
            }
        }
        rot = next;
        println!("{frame} {s:.3} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5}", rot[0][0], rot[0][1], rot[0][2], rot[1][0], rot[1][1], rot[1][2], rot[2][0], rot[2][1], rot[2][2]);
    }
}

fn tidy(m: [V3; 3]) -> [V3; 3] {
    let col = |i: usize| [m[0][i], m[1][i], m[2][i]];
    let x = norm(col(0));
    let y0 = col(1);
    let d = dot(x, y0);
    let y = norm([y0[0] - d * x[0], y0[1] - d * x[1], y0[2] - d * x[2]]);
    let z = [x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]];
    [[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]]
}

fn motion(path: &str) -> Vec<[V3; 3]> {
    let text = fs::read_to_string(path).unwrap_or_default();
    let half: Vec<[V3; 3]> = text
        .lines()
        .filter_map(|l| {
            let v: Vec<f64> = l.split_whitespace().skip(2).filter_map(|n| n.parse().ok()).collect();
            (v.len() == 9).then(|| [[v[0], v[1], v[2]], [v[3], v[4], v[5]], [v[6], v[7], v[8]]])
        })
        .take(FRAMES)
        .collect();
    if half.len() < FRAMES {
        return Vec::new();
    }
    let mirror = |m: &[V3; 3]| [[m[0][0], m[0][1], -m[0][2]], [m[1][0], m[1][1], -m[1][2]], [-m[2][0], -m[2][1], m[2][2]]];
    let close = |a: &[V3; 3], b: &[V3; 3]| dot(a[0], b[0]) + dot(a[1], b[1]) + dot(a[2], b[2]);
    let start = (0..FRAMES).min_by(|&a, &b| half[a][2][2].abs().total_cmp(&half[b][2][2].abs())).unwrap_or(0);
    let mut path: Vec<[V3; 3]> = vec![half[start]];
    for i in 1..FRAMES {
        let now = half[(start + i) % FRAMES];
        let prev = if i >= 3 { matmul(&matmul(&path[i - 1], &transpose(&path[i - 3])), &path[i - 1]) } else { path[i - 1] };
        let mut best = now;
        let mut top = f64::MIN;
        for base in [now, mirror(&now)] {
            for k in 0..3 {
                let cand = matmul(&base, &rot_z(k as f64 * TAU / 3.0));
                let c = close(&prev, &cand);
                if c > top {
                    top = c;
                    best = cand;
                }
            }
        }
        path.push(best);
    }
    let mut whole = vec![[[0.0; 3]; 3]; FRAMES];
    for (i, m) in path.into_iter().enumerate() {
        whole[(start + i) % FRAMES] = m;
    }
    (0..FRAMES)
        .map(|f| {
            let mut sum = [[0.0; 3]; 3];
            for k in 0..11 {
                let m = &whole[(f + FRAMES + k - 5) % FRAMES];
                let c = &whole[f];
                let near = (dot(m[0], c[0]) + dot(m[1], c[1]) + dot(m[2], c[2]) - 1.0) / 2.0;
                if near < 0.85 {
                    continue;
                }
                let weight = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0][k];
                for r in 0..3 {
                    for c in 0..3 {
                        sum[r][c] += m[r][c] * weight;
                    }
                }
            }
            tidy(sum)
        })
        .collect()
}

fn texture(old: &[u8]) -> Vec<u8> {
    let mut out = old[..4].to_vec();
    let (w, h) = (WIDTH as i32, HEIGHT as i32);
    for frame in 0..FRAMES {
        let at = 4 + frame * WIDTH * HEIGHT;
        let id = |x: i32, y: i32| -> u8 {
            if x < 0 || y < 0 || x >= w || y >= h {
                return 0;
            }
            let v = old[at + (y * w + x) as usize];
            if v & 15 == 0 { 0 } else if v < 16 { 1 } else { 2 }
        };
        let mut height = vec![0f64; WIDTH * HEIGHT];
        for y in 0..h {
            for x in 0..w {
                let own = id(x, y);
                if own == 0 {
                    continue;
                }
                let mut ring = 3i32;
                for dy in -2i32..=2 {
                    for dx in -2i32..=2 {
                        if id(x + dx, y + dy) != own {
                            ring = ring.min(dx.abs().max(dy.abs()));
                        }
                    }
                }
                height[(y * w + x) as usize] = ring as f64;
            }
        }
        let mut shades = vec![0u8; WIDTH * HEIGHT];
        for y in 0..h {
            for x in 0..w {
                let own = id(x, y);
                if own == 0 {
                    continue;
                }
                let mut peak = 0.0f64;
                for dy in -2i32..=2 {
                    for dx in -2i32..=2 {
                        if id(x + dx, y + dy) == own {
                            peak = peak.max(height[((y + dy) * w + x + dx) as usize]);
                        }
                    }
                }
                let ring = height[(y * w + x) as usize];
                let drop = if ring >= peak { 0.0 } else if ring <= 1.0 { RIM } else { MID };
                let lit = (((old[at + (y * w + x) as usize] & 15) as f64 - 4.0) / 9.0).clamp(0.0, 1.0);
                let level = CORE + (15.0 - CORE) * lit - drop;
                shades[(y * w + x) as usize] = (level).round().clamp(1.0, 15.0) as u8;
            }
        }
        for y in (0..h).step_by(2) {
            for x in 0..w {
                let (a, b) = ((y * w + x) as usize, ((y + 1) * w + x) as usize);
                if id(x, y) != 0 && id(x, y) == id(x, y + 1) {
                    let mean = ((shades[a] as u32 + shades[b] as u32 + 1) / 2) as u8;
                    shades[a] = mean;
                    shades[b] = mean;
                }
            }
        }
        for (i, &shade) in shades.iter().enumerate() {
            let value = old[at + i];
            out.push(if shade == 0 { value } else { (value & 16) | shade });
        }
    }
    out
}



const GLYPH_HOLD: usize = 5;
const HOLD_ON: f64 = 1.2;
const HOLD_OFF: f64 = 0.7;
const HOLD_EDGE: f64 = 0.6;
const HOLD_COHERENCE: f64 = 0.15;
const HOLD_ANGLE: f64 = 15.0;

fn steady(data: &[u8]) -> (Vec<u8>, usize) {
    let mut out = data.to_vec();
    let frame_len = WIDTH * HEIGHT;
    let on = |f: usize, row: usize, col: usize| data[4 + f * frame_len + row * 2 * WIDTH + col] & 15 != 0;
    let mut changed = 0;
    for row in 0..ROWS {
        for col in 0..WIDTH {
            for f in 0..FRAMES {
                let before = on((f + FRAMES - 1) % FRAMES, row, col);
                for span in 1..=3 {
                    let after = on((f + span) % FRAMES, row, col);
                    let inside: Vec<bool> = (0..span).map(|k| on((f + k) % FRAMES, row, col)).collect();
                    if before == after && inside.iter().all(|&s| s != before) {
                        let src = 4 + ((f + FRAMES - 1) % FRAMES) * frame_len + row * 2 * WIDTH + col;
                        for k in 0..span {
                            let dst = 4 + ((f + k) % FRAMES) * frame_len + row * 2 * WIDTH + col;
                            out[dst] = data[src];
                            out[dst + WIDTH] = data[src + WIDTH];
                            changed += 1;
                        }
                        break;
                    }
                }
            }
        }
    }
    let lone = out.clone();
    let lit0 = |f: usize, r: i32, c: i32| r >= 0 && c >= 0 && (r as usize) < ROWS && (c as usize) < WIDTH && lone[4 + f * frame_len + r as usize * 2 * WIDTH + c as usize] & 15 != 0;
    for f in 0..FRAMES {
        for row in 0..ROWS as i32 {
            for col in 0..WIDTH as i32 {
                if !lit0(f, row, col) {
                    continue;
                }
                let near = (-1..=1).flat_map(|dr| (-1..=1).map(move |dc| (dr, dc))).filter(|&(dr, dc)| (dr, dc) != (0, 0) && lit0(f, row + dr, col + dc)).count();
                if near == 0 {
                    let at = 4 + f * frame_len + row as usize * 2 * WIDTH + col as usize;
                    out[at] = 0;
                    out[at + WIDTH] = b' ';
                    changed += 1;
                }
            }
        }
    }
    let base = out.clone();
    let glyph = |f: usize, row: usize, col: usize| base[4 + f * frame_len + (row * 2 + 1) * WIDTH + col];
    let lit = |f: usize, row: usize, col: usize| base[4 + f * frame_len + row * 2 * WIDTH + col] & 15 != 0;
    for row in 0..ROWS {
        for col in 0..WIDTH {
            for f in 0..FRAMES {
                if !lit(f, row, col) {
                    continue;
                }
                let mut counts: Vec<(u8, usize)> = Vec::new();
                for k in 0..=2 * GLYPH_HOLD {
                    let g = (f + FRAMES + k - GLYPH_HOLD) % FRAMES;
                    if !lit(g, row, col) {
                        continue;
                    }
                    let ch = glyph(g, row, col);
                    match counts.iter_mut().find(|(c, _)| *c == ch) {
                        Some(slot) => slot.1 += 1,
                        None => counts.push((ch, 1)),
                    }
                }
                let own = glyph(f, row, col);
                let own_count = counts.iter().find(|(c, _)| *c == own).map_or(0, |(_, n)| *n);
                if let Some(&(best, n)) = counts.iter().max_by_key(|(_, n)| *n) {
                    if n > own_count {
                        out[4 + f * frame_len + (row * 2 + 1) * WIDTH + col] = best;
                        changed += 1;
                    }
                }
            }
        }
    }
    (out, changed)
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() == 3 && args[0] == "steady" {
        let old = fs::read(&args[1]).unwrap_or_default();
        if old.len() < 4 + FRAMES * WIDTH * HEIGHT {
            eprintln!("{}: not a frames file", args[1]);
            process::exit(1);
        }
        let (out, changed) = steady(&old);
        println!("steadied {changed} cells");
        if let Err(e) = fs::write(&args[2], &out) {
            eprintln!("{}: {e}", args[2]);
            process::exit(1);
        }
        return;
    }
    if args.len() == 3 && args[0] == "ascii" {
        let old = fs::read(&args[1]).unwrap_or_default();
        if old.len() < 4 + FRAMES * WIDTH * HEIGHT {
            eprintln!("{}: not a frames file", args[1]);
            process::exit(1);
        }
        if let Err(e) = fs::write(&args[2], flat_ascii(&old)) {
            eprintln!("{}: {e}", args[2]);
            process::exit(1);
        }
        return;
    }
    if args.len() == 3 && args[0] == "texture" {
        let old = fs::read(&args[1]).unwrap_or_default();
        if old.len() < 4 + FRAMES * WIDTH * HEIGHT {
            eprintln!("{}: not a frames file", args[1]);
            process::exit(1);
        }
        let out = texture(&old);
        if let Err(e) = fs::write(&args[2], &out) {
            eprintln!("{}: {e}", args[2]);
            process::exit(1);
        }
        return;
    }
    if args.len() == 3 && args[0] == "fit" {
        let old = fs::read(&args[2]).unwrap_or_default();
        if old.len() < 4 + FRAMES * WIDTH * HEIGHT {
            eprintln!("{}: not a frames file", args[2]);
            process::exit(1);
        }
        return fit(&Mask::load(&args[1]), &old);
    }
    if args.len() < 2 {
        eprintln!("Use: nix-spin-gen MASK.png OUT.bin [MOTION TURNS START]");
        process::exit(2)
    }
    let (mask_path, out_path) = (&args[0], &args[1]);
    let number = |i: usize, d: f64| args.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(d);
    let (around, start) = (number(3, 1.0), number(4, 0.0).to_radians());
    let measured = args.get(2).map(|p| motion(p)).unwrap_or_default();
    let mask = Mask::load(mask_path);
    let mut out = vec![(OUT_FRAMES & 255) as u8, WIDTH as u8, HEIGHT as u8, (OUT_FRAMES >> 8) as u8];
    let mut prev: Option<(Vec<u8>, Vec<u8>)> = None;
    for pass_frame in 0..2 * OUT_FRAMES {
        let frame = pass_frame % OUT_FRAMES;
        let turn = frame as f64 / OUT_FRAMES as f64;
        let rot = if measured.len() == FRAMES {
            let at = turn * FRAMES as f64;
            let (i, part) = (at.floor() as usize % FRAMES, at - at.floor());
            let (a, b) = (&measured[i], &measured[(i + 1) % FRAMES]);
            let part = if dot(a[0], b[0]) + dot(a[1], b[1]) + dot(a[2], b[2]) < 2.2 { 0.0 } else { part };
            let mut mix = [[0.0; 3]; 3];
            for r in 0..3 {
                for c in 0..3 {
                    mix[r][c] = a[r][c] * (1.0 - part) + b[r][c] * part;
                }
            }
            tidy(mix)
        } else {
            matmul(&matmul(&rot_y(TAU * 2.0 * turn), &rot_x(TAU * 3.0 * turn)), &rot_z(TAU * around * turn + start))
        };
        let inv = transpose(&rot);
        let (gw, gh) = (WIDTH * SAMPLES_X, ROWS * SAMPLES_Y);
        let mut kind = vec![0u8; gw * gh];
        let mut glow = vec![0f64; gw * gh];
        for gy in 0..gh {
            for gx in 0..gw {
                let x = ((gx as f64 + 0.5) / gw as f64 - 0.5) / SCALE;
                let y = ((gy as f64 + 0.5) / gh as f64 - 0.5) / SCALE;
                if let Some(hit) = trace(&mask, &rot, &inv, x, y) {
                    kind[gy * gw + gx] = if hit.face { hit.id } else { 3 };
                    glow[gy * gw + gx] = hit.light;
                }
            }
        }
        let field: Vec<f64> = kind.iter().map(|&k| [0.0, 1.0, 0.62, 0.36][k as usize]).collect();
        let (cells, glyphs) = ascii_cells(&kind, &glow, &field, prev.as_ref());
        if pass_frame >= OUT_FRAMES {
            for row in 0..ROWS {
                out.extend_from_slice(&cells[row * WIDTH..(row + 1) * WIDTH]);
                out.extend_from_slice(&glyphs[row * WIDTH..(row + 1) * WIDTH]);
            }
        }
        prev = Some((cells, glyphs));
    }
    if let Err(e) = fs::write(out_path, &out) {
        eprintln!("{out_path}: {e}");
        process::exit(1);
    }
}

fn ascii_cells(kind: &[u8], glow: &[f64], field: &[f64], prev: Option<&(Vec<u8>, Vec<u8>)>) -> (Vec<u8>, Vec<u8>) {
    let (gw, gh) = (WIDTH * SAMPLES_X, ROWS * SAMPLES_Y);
    let field = |x: i64, y: i64| -> f64 {
        if x < 0 || y < 0 || x >= gw as i64 || y >= gh as i64 {
            return 0.0;
        }
        field[y as usize * gw + x as usize]
    };
        let mut cells = vec![0u8; WIDTH * ROWS];
        let mut glyphs = vec![0u8; WIDTH * ROWS];
        for row in 0..ROWS {
            for col in 0..WIDTH {
                let (mut hits, mut sum, mut first, mut edges) = (0usize, 0.0f64, 0usize, 0usize);
                let (mut jxx, mut jyy, mut jxy) = (0.0f64, 0.0f64, 0.0f64);
                for sy in 0..SAMPLES_Y {
                    for sx in 0..SAMPLES_X {
                        let (x, y) = (col * SAMPLES_X + sx, row * SAMPLES_Y + sy);
                        let k = kind[y * gw + x];
                        if k != 0 {
                            hits += 1;
                            sum += glow[y * gw + x];
                            first += (k == 1) as usize;
                        }
                        let (x, y) = (x as i64, y as i64);
                        let f = |dx: i64, dy: i64| field(x + dx, y + dy);
                        let sx_ = (f(1, -1) + 2.0 * f(1, 0) + f(1, 1)) - (f(-1, -1) + 2.0 * f(-1, 0) + f(-1, 1));
                        let sy_ = (f(-1, 1) + 2.0 * f(0, 1) + f(1, 1)) - (f(-1, -1) + 2.0 * f(0, -1) + f(1, -1));
                        if sx_.abs() + sy_.abs() > EDGE_STEP {
                            edges += 1;
                            jxx += sx_ * sx_;
                            jyy += sy_ * sy_;
                            jxy += sx_ * sy_;
                        }
                    }
                }
                let total = (SAMPLES_X * SAMPLES_Y) as f64;
                let spread = jxx + jyy;
                let coherence = if spread > 0.0 { ((jxx - jyy).powi(2) + 4.0 * jxy * jxy).sqrt() / spread } else { 0.0 };
                let at = row * WIDTH + col;
                let (was_lit, was_glyph) = prev.map_or((false, 0u8), |(c, g)| (c[at] != 0, g[at]));
                let (share, coh) = if was_glyph != 0 { (EDGE_SHARE * HOLD_EDGE, EDGE_COHERENCE - HOLD_COHERENCE) } else { (EDGE_SHARE, EDGE_COHERENCE) };
                let lined = edges as f64 >= share * total && coherence >= coh && hits as f64 >= EDGE_COVER * total;
                let cover = if prev.is_none() { COVER } else if was_lit { COVER * HOLD_OFF } else { COVER * HOLD_ON };
                if !lined && (hits as f64) < cover * total {
                    continue;
                }
                let level = ((sum / hits.max(1) as f64 - 0.14) / 1.02).clamp(0.0, 1.0).powf(FILL_GAMMA);
                let shade = 1 + (level * 14.0).round() as u8;
                cells[at] = if first * 2 >= hits { shade } else { 16 + shade };
                if lined {
                    let line = (0.5 * (2.0 * jxy).atan2(jxx - jyy)).to_degrees() + 90.0;
                    let line = line.rem_euclid(180.0);
                    let center = match was_glyph { b'_' => Some(0.0), b'\\' => Some(45.0), b'|' => Some(90.0), b'/' => Some(135.0), _ => None };
                    let near = center.is_some_and(|c: f64| { let d = (line - c).rem_euclid(180.0); d.min(180.0 - d) <= 22.5 + HOLD_ANGLE });
                    glyphs[at] = if near { was_glyph } else { match line {
                        a if !(22.5..157.5).contains(&a) => b'_',
                        a if a < 67.5 => b'\\',
                        a if a < 112.5 => b'|',
                        _ => b'/',
                    }; };
                }
            }
        }
    (cells, glyphs)
}

fn flat_ascii(old: &[u8]) -> Vec<u8> {
    let mut out = old[..4].to_vec();
    let (gw, gh) = (WIDTH * SAMPLES_X, ROWS * SAMPLES_Y);
    let (w, h) = (WIDTH as i32, HEIGHT as i32);
    let py_per = SAMPLES_Y / 2;
    for frame in 0..FRAMES {
        let at = 4 + frame * WIDTH * HEIGHT;
        let id = |x: i32, y: i32| -> u8 {
            if x < 0 || y < 0 || x >= w || y >= h {
                return 0;
            }
            let v = old[at + (y * w + x) as usize];
            if v & 15 == 0 { 0 } else if v < 16 { 1 } else { 2 }
        };
        let mut light = vec![0f64; WIDTH * HEIGHT];
        for y in 0..h {
            for x in 0..w {
                let own = id(x, y);
                if own == 0 {
                    continue;
                }
                let ring = |r: i32| (-r..=r).any(|dy| (-r..=r).any(|dx| id(x + dx, y + dy) != own));
                let band = if ring(1) { RIM_LIGHT } else if ring(2) { MID_LIGHT } else { 1.0 };
                let base = (old[at + (y * w + x) as usize] & 15) as f64 / 15.0;
                let lit = (0.55 + 0.45 * base) * band;
                light[(y * w + x) as usize] = 0.14 + 1.02 * lit;
            }
        }
        let mut kind = vec![0u8; gw * gh];
        let mut glow = vec![0f64; gw * gh];
        let mut raw = vec![0f64; gw * gh];
        for gy in 0..gh {
            for gx in 0..gw {
                let (x, y) = ((gx / SAMPLES_X) as i32, (gy / py_per) as i32);
                let k = id(x, y);
                kind[gy * gw + gx] = k;
                glow[gy * gw + gx] = light[(y * w + x) as usize];
                raw[gy * gw + gx] = [0.0, 1.0, 0.62][k as usize];
            }
        }
        let r = BLUR as i64;
        let mut field = vec![0f64; gw * gh];
        for gy in 0..gh as i64 {
            for gx in 0..gw as i64 {
                let (mut sum, mut n) = (0.0, 0.0);
                for dy in -r..=r {
                    for dx in -r..=r {
                        let (x, y) = (gx + dx, gy + dy);
                        if x >= 0 && y >= 0 && x < gw as i64 && y < gh as i64 {
                            sum += raw[y as usize * gw + x as usize];
                        }
                        n += 1.0;
                    }
                }
                field[gy as usize * gw + gx as usize] = sum / n;
            }
        }
        let (cells, glyphs) = ascii_cells(&kind, &glow, &field, None);
        for row in 0..ROWS {
            out.extend_from_slice(&cells[row * WIDTH..(row + 1) * WIDTH]);
            out.extend_from_slice(&glyphs[row * WIDTH..(row + 1) * WIDTH]);
        }
    }
    out
}

#[cfg(test)]
mod steady_tests {
    use super::*;

    fn blank() -> Vec<u8> {
        let mut data = vec![0u8; 4 + FRAMES * WIDTH * HEIGHT];
        data[0] = (FRAMES % 256) as u8;
        data[1] = WIDTH as u8;
        data[2] = HEIGHT as u8;
        data[3] = (FRAMES / 256) as u8;
        data
    }

    fn cell(frame: usize, row: usize, col: usize) -> usize {
        4 + frame * WIDTH * HEIGHT + row * 2 * WIDTH + col
    }

    #[test]
    fn short_blips_are_removed() {
        let mut data = blank();
        data[cell(10, 3, 5)] = 7;
        data[cell(20, 4, 6)] = 7;
        data[cell(21, 4, 6)] = 7;
        let (out, changed) = steady(&data);
        assert_eq!(out[cell(10, 3, 5)], 0);
        assert_eq!(out[cell(20, 4, 6)], 0);
        assert_eq!(out[cell(21, 4, 6)], 0);
        assert_eq!(changed, 3);
    }

    #[test]
    fn short_gaps_are_filled_and_long_shapes_stay() {
        let mut data = blank();
        for f in 0..FRAMES {
            data[cell(f, 2, 3)] = 9;
        }
        for f in 0..FRAMES {
            if f != 50 {
                data[cell(f, 2, 2)] = 9;
                data[cell(f, 2, 2) + WIDTH] = b'/';
            }
        }
        for f in 100..110 {
            data[cell(f, 8, 8)] = 5;
            data[cell(f, 8, 9)] = 5;
        }
        let (out, _) = steady(&data);
        assert_eq!(out[cell(50, 2, 2)], 9);
        assert_eq!(out[cell(50, 2, 2) + WIDTH], b'/');
        assert_eq!(out[cell(105, 8, 8)], 5);
        assert_eq!(out[cell(99, 8, 8)], 0);
    }

    #[test]
    fn lone_cells_are_removed() {
        let mut data = blank();
        data[cell(7, 10, 10)] = 6;
        data[cell(7, 10, 10) + WIDTH] = b'\\';
        data[cell(6, 10, 10)] = 6;
        data[cell(8, 10, 10)] = 6;
        let (out, _) = steady(&data);
        assert_eq!(out[cell(7, 10, 10)], 0);
    }

    #[test]
    fn short_glyph_flips_are_held() {
        let mut data = blank();
        for f in 0..FRAMES {
            data[cell(f, 5, 5)] = 9;
            data[cell(f, 5, 6)] = 9;
            data[cell(f, 5, 5) + WIDTH] = if f == 30 || f == 31 { b'_' } else if f >= 120 { b'\\' } else { b'/' };
        }
        let (out, _) = steady(&data);
        assert_eq!(out[cell(30, 5, 5) + WIDTH], b'/');
        assert_eq!(out[cell(31, 5, 5) + WIDTH], b'/');
        assert_eq!(out[cell(150, 5, 5) + WIDTH], b'\\');
    }
}
