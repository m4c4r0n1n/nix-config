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
        let bx = (grain(p[0] + e, p[1]) - grain(p[0] - e, p[1])) / (2.0 * e) * BUMP;
        let by = (grain(p[0], p[1] + e) - grain(p[0], p[1] - e)) / (2.0 * e) * BUMP;
        (norm([-gx * edge - bx, -gy * edge - by, side]), ALBEDO_BASE + ALBEDO_GRAIN * texture)
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



const GLYPH_HOLD: usize = 8;
const OUTLINES: bool = true;
const BUMP: f64 = 0.018;
const ALBEDO_BASE: f64 = 0.70;
const ALBEDO_GRAIN: f64 = 0.60;
const SIDE_ALBEDO: f64 = 0.5;
const EASE: f64 = 0.0;
const ROUND: f64 = 8.0;

fn ease(a: f64) -> f64 {
    a - EASE * (2.0 * a).sin()
}
const HOLD_ON: f64 = 1.0;
const HOLD_OFF: f64 = 1.0;
const HOLD_EDGE: f64 = 1.0;
const HOLD_COHERENCE: f64 = 0.0;
const HOLD_ANGLE: f64 = -22.5;

fn steady(data: &[u8]) -> (Vec<u8>, usize) {
    let frames = data[0] as usize + data[3] as usize * 256;
    let mut out = data.to_vec();
    let frame_len = WIDTH * HEIGHT;
    let on = |f: usize, row: usize, col: usize| data[4 + f * frame_len + row * 2 * WIDTH + col] & 15 != 0;
    let mut changed = 0;
    for row in 0..ROWS {
        for col in 0..WIDTH {
            for f in 0..frames {
                let before = on((f + frames - 1) % frames, row, col);
                for span in 1..=3 {
                    let after = on((f + span) % frames, row, col);
                    let inside: Vec<bool> = (0..span).map(|k| on((f + k) % frames, row, col)).collect();
                    if before == after && inside.iter().all(|&s| s != before) {
                        let src = 4 + ((f + frames - 1) % frames) * frame_len + row * 2 * WIDTH + col;
                        for k in 0..span {
                            let dst = 4 + ((f + k) % frames) * frame_len + row * 2 * WIDTH + col;
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
    for f in 0..frames {
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
            for f in 0..frames {
                if !lit(f, row, col) {
                    continue;
                }
                let mut counts: Vec<(u8, usize)> = Vec::new();
                for k in 0..=2 * GLYPH_HOLD {
                    let g = (f + frames + k - GLYPH_HOLD) % frames;
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
    let strokes = out.clone();
    let mark = |f: usize, r: i32, c: i32| -> u8 {
        if r < 0 || c < 0 || r as usize >= ROWS || c as usize >= WIDTH {
            return 0;
        }
        strokes[4 + f * frame_len + (r as usize * 2 + 1) * WIDTH + c as usize]
    };
    for f in 0..frames {
        for row in 0..ROWS as i32 {
            for col in 0..WIDTH as i32 {
                let g = mark(f, row, col);
                let steps: [(i32, i32); 2] = match g {
                    b'_' => [(0, -1), (0, 1)],
                    b'|' => [(-1, 0), (1, 0)],
                    b'/' => [(-1, 1), (1, -1)],
                    b'\\' => [(-1, -1), (1, 1)],
                    _ => continue,
                };
                if steps.iter().all(|&(dr, dc)| mark(f, row + dr, col + dc) != g) {
                    out[4 + f * frame_len + (row as usize * 2 + 1) * WIDTH + col as usize] = 0;
                    changed += 1;
                }
            }
        }
    }
    (out, changed)
}

const SHAPE_CHARS: &[u8] = b".,:;'`_/\\|()[]<>^*+#%$&";
const SHAPE_SX: usize = 12;
const SHAPE_SY: usize = 26;
const CELL_ASPECT: f64 = 128.0 / 59.0;
const CIRCLE_R: f64 = 0.27;
const DIR_EXP: f64 = 3.0;
const GLOBAL_EXP: f64 = 2.0;
const INNER: [(f64, f64); 6] = [(0.28, 0.23), (0.72, 0.17), (0.28, 0.53), (0.72, 0.47), (0.28, 0.83), (0.72, 0.77)];
const OUTER: [(f64, f64); 10] = [
    (0.28, -0.13),
    (0.72, -0.13),
    (-0.22, 0.23),
    (1.22, 0.17),
    (-0.22, 0.53),
    (1.22, 0.47),
    (-0.22, 0.83),
    (1.22, 0.77),
    (0.28, 1.13),
    (0.72, 1.13),
];
const AFFECTING: [&[usize]; 6] = [&[0, 1, 2, 4], &[0, 1, 3, 5], &[2, 4, 6], &[3, 5, 7], &[4, 6, 8, 9], &[5, 7, 8, 9]];

struct GlyphSet {
    chars: Vec<u8>,
    vecs: Vec<[f64; 6]>,
}

fn in_circle(u: f64, v: f64, c: (f64, f64)) -> bool {
    let (dx, dy) = (u - c.0, (v - c.1) * CELL_ASPECT);
    dx * dx + dy * dy <= CIRCLE_R * CIRCLE_R
}

fn glyph_vector(pixels: &[u8], w: usize, h: usize) -> [f64; 6] {
    let mut out = [0.0; 6];
    for (i, c) in INNER.iter().enumerate() {
        let (mut sum, mut n) = (0.0, 0.0);
        for y in 0..h {
            for x in 0..w {
                let (u, v) = ((x as f64 + 0.5) / w as f64, (y as f64 + 0.5) / h as f64);
                if in_circle(u, v, *c) {
                    sum += pixels[y * w + x] as f64 / 255.0;
                    n += 1.0;
                }
            }
        }
        out[i] = if n > 0.0 { sum / n } else { 0.0 };
    }
    out
}

fn load_glyphs(dir: &str) -> GlyphSet {
    let mut chars = vec![b' '];
    let mut vecs = vec![[0.0; 6]];
    let mut entries: Vec<_> = fs::read_dir(dir).map(|r| r.flatten().collect()).unwrap_or_default();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(code) = name.strip_suffix(".png").and_then(|s| s.parse::<u8>().ok()) else { continue };
        if !SHAPE_CHARS.contains(&code) {
            continue;
        }
        let Ok(img) = image::open(e.path()) else { continue };
        let g = img.to_luma8();
        chars.push(code);
        vecs.push(glyph_vector(g.as_raw(), g.width() as usize, g.height() as usize));
    }
    let mut max = [1e-9f64; 6];
    for v in &vecs {
        for i in 0..6 {
            max[i] = max[i].max(v[i]);
        }
    }
    for v in vecs.iter_mut() {
        for i in 0..6 {
            v[i] /= max[i];
        }
    }
    GlyphSet { chars, vecs }
}

fn nearest(set: &GlyphSet, v: &[f64; 6]) -> u8 {
    let mut best = (f64::MAX, b' ');
    for (k, g) in set.vecs.iter().enumerate() {
        let d: f64 = (0..6).map(|i| (g[i] - v[i]).powi(2)).sum();
        if d < best.0 {
            best = (d, set.chars[k]);
        }
    }
    best.1
}

fn shape_frame(mask: &Mask, rot: &[V3; 3], set: &GlyphSet) -> (Vec<u8>, Vec<u8>) {
    let inv = transpose(rot);
    let (gw, gh) = (WIDTH * SHAPE_SX, ROWS * SHAPE_SY);
    let mut light = vec![0f64; gw * gh];
    let mut ids = vec![0u8; gw * gh];
    for gy in 0..gh {
        for gx in 0..gw {
            let x = ((gx as f64 + 0.5) / gw as f64 - 0.5) / SCALE;
            let y = ((gy as f64 + 0.5) / gh as f64 - 0.5) / SCALE;
            if let Some(hit) = trace(mask, rot, &inv, x, y) {
                light[gy * gw + gx] = hit.light.clamp(0.0, 1.0);
                ids[gy * gw + gx] = hit.id;
            }
        }
    }
    let sample = |col: i64, row: i64, c: (f64, f64)| -> f64 {
        let (mut sum, mut n) = (0.0, 0.0);
        let x0 = ((col as f64 + c.0 - CIRCLE_R) * SHAPE_SX as f64).floor() as i64;
        let x1 = ((col as f64 + c.0 + CIRCLE_R) * SHAPE_SX as f64).ceil() as i64;
        let ry = CIRCLE_R / CELL_ASPECT;
        let y0 = ((row as f64 + c.1 - ry) * SHAPE_SY as f64).floor() as i64;
        let y1 = ((row as f64 + c.1 + ry) * SHAPE_SY as f64).ceil() as i64;
        for py in y0..y1 {
            for px in x0..x1 {
                let u = (px as f64 + 0.5) / SHAPE_SX as f64 - col as f64;
                let v = (py as f64 + 0.5) / SHAPE_SY as f64 - row as f64;
                if !in_circle(u, v, c) {
                    continue;
                }
                n += 1.0;
                if px >= 0 && py >= 0 && (px as usize) < gw && (py as usize) < gh {
                    sum += light[py as usize * gw + px as usize];
                }
            }
        }
        if n > 0.0 { sum / n } else { 0.0 }
    };
    let mut cells = vec![0u8; WIDTH * ROWS];
    let mut glyphs = vec![0u8; WIDTH * ROWS];
    for row in 0..ROWS {
        for col in 0..WIDTH {
            let (c, r) = (col as i64, row as i64);
            let mut v = [0.0f64; 6];
            for i in 0..6 {
                v[i] = sample(c, r, INNER[i]);
            }
            let ext: Vec<f64> = OUTER.iter().map(|&o| sample(c, r, o)).collect();
            let mut d = v;
            for i in 0..6 {
                let m = AFFECTING[i].iter().fold(v[i], |m, &k| m.max(ext[k]));
                if m > 0.0 {
                    d[i] = (v[i] / m).powf(DIR_EXP) * m;
                }
            }
            let gmax = d.iter().cloned().fold(0.0f64, f64::max);
            if gmax > 0.0 {
                for x in d.iter_mut() {
                    *x = (*x / gmax).powf(GLOBAL_EXP) * gmax;
                }
            }
            let ch = nearest(set, &d);
            if ch == b' ' {
                continue;
            }
            let (mut sum, mut hits, mut first) = (0.0, 0usize, 0usize);
            for sy in 0..SHAPE_SY {
                for sx in 0..SHAPE_SX {
                    let at = (row * SHAPE_SY + sy) * gw + col * SHAPE_SX + sx;
                    if ids[at] != 0 {
                        hits += 1;
                        sum += light[at];
                        first += (ids[at] == 1) as usize;
                    }
                }
            }
            let level = if hits > 0 { sum / hits as f64 } else { v.iter().sum::<f64>() / 6.0 };
            let shade = 1 + (level.clamp(0.0, 1.0) * 14.0).round() as u8;
            cells[row * WIDTH + col] = if first * 2 >= hits.max(1) { shade } else { 16 + shade };
            glyphs[row * WIDTH + col] = ch;
        }
    }
    (cells, glyphs)
}

fn shape_render(mask_path: &str, glyph_dir: &str, out_path: &str) {
    let mask = Mask::load(mask_path);
    let set = load_glyphs(glyph_dir);
    let mut out = vec![(OUT_FRAMES & 255) as u8, WIDTH as u8, HEIGHT as u8, (OUT_FRAMES >> 8) as u8];
    for frame in 0..OUT_FRAMES {
        let turn = frame as f64 / OUT_FRAMES as f64;
        let rot = matmul(&matmul(&rot_y(TAU * 2.0 * turn), &rot_x(TAU * 3.0 * turn)), &rot_z(TAU * turn));
        let (cells, glyphs) = shape_frame(&mask, &rot, &set);
        for row in 0..ROWS {
            out.extend_from_slice(&cells[row * WIDTH..(row + 1) * WIDTH]);
            out.extend_from_slice(&glyphs[row * WIDTH..(row + 1) * WIDTH]);
        }
    }
    if let Err(e) = fs::write(out_path, &out) {
        eprintln!("{out_path}: {e}");
        process::exit(1);
    }
}

const F3_SX: f64 = 0.07;
const F3_SY: f64 = 0.14;
const F3_LAYERS: usize = 6;
const F3_SUBDIV: usize = 2;
const F3_K2: f64 = 5.5;
const F3_FILL: f64 = 0.95;
const F3_LIGHT: V3 = [0.4082, 0.8165, -0.4082];

struct F3Point {
    p: V3,
    n: V3,
    face: bool,
}

fn f3_weight(ch: char) -> f64 {
    match ch {
        'M' => 1.0,
        'N' => 0.88,
        'm' => 0.76,
        'd' => 0.66,
        'h' | 'b' => 0.56,
        'y' => 0.46,
        'o' | 'n' => 0.38,
        's' => 0.30,
        '+' | '=' => 0.22,
        ':' => 0.18,
        '-' => 0.14,
        '`' => 0.08,
        '.' => 0.10,
        '/' => 0.12,
        '\'' => 0.06,
        ' ' => 0.0,
        '█' => 1.0,
        '▓' | '▛' | '▜' | '▙' | '▟' => 0.75,
        '▒' | '▀' | '▄' | '▌' | '▐' => 0.50,
        '░' | '▖' | '▗' | '▘' | '▝' => 0.25,
        c if c.is_ascii_uppercase() => 0.80,
        c if c.is_ascii_lowercase() => 0.50,
        c if c.is_ascii_digit() => 0.40,
        c if c.is_ascii() => 0.15,
        c if ('\u{2500}'..='\u{257F}').contains(&c) => 0.20,
        _ => 0.30,
    }
}

fn f3_points(text: &str) -> Vec<F3Point> {
    let mut lines: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
    while lines.last().is_some_and(|l| l.iter().all(|c| *c == ' ')) {
        lines.pop();
    }
    let rows = lines.len();
    let cols = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let h = |r: i64, c: i64| -> f64 {
        if r < 0 || c < 0 || r as usize >= rows || c as usize >= cols {
            return 0.0;
        }
        lines[r as usize].get(c as usize).map_or(0.0, |&ch| f3_weight(ch))
    };
    let (mut sum, mut sum2, mut n) = (0.0, 0.0, 0.0);
    for r in 0..rows as i64 {
        for c in 0..cols as i64 {
            let v = h(r, c);
            if v > 0.0 {
                sum += v;
                sum2 += v * v;
                n += 1.0;
            }
        }
    }
    let mut depth = 1.0;
    if n > 0.0 {
        let mean = sum / n;
        let sd = (sum2 / n - mean * mean).max(0.0).sqrt();
        if sd < 0.25 {
            depth *= 1.0 + 2.0 * (0.25 - sd) / 0.25;
        }
    }
    let zmax = 0.18 * depth;
    let (cx, cy) = ((cols as f64 - 1.0) * 0.5, (rows as f64 - 1.0) * 0.5);
    let grad = |r: i64, c: i64| -> V3 {
        let dx = if c > 0 && (c as usize) < cols - 1 { (h(r, c + 1) - h(r, c - 1)) * 0.5 } else if c == 0 { h(r, c + 1) - h(r, c) } else { h(r, c) - h(r, c - 1) };
        let dy = if r > 0 && (r as usize) < rows - 1 { (h(r + 1, c) - h(r - 1, c)) * 0.5 } else if r == 0 { h(r + 1, c) - h(r, c) } else { h(r, c) - h(r - 1, c) };
        norm([-dx / F3_SX, dy / F3_SY, 1.0])
    };
    let mut out = Vec::new();
    for row in 0..rows as i64 {
        for col in 0..cols as i64 {
            let base = h(row, col);
            if base <= 0.0 {
                continue;
            }
            let g = grad(row, col);
            let is_edge = (-1..=1).any(|dr| (-1..=1).any(|dc| (dr, dc) != (0, 0) && h(row + dr, col + dc) <= 0.0));
            let (mut ex, mut ey) = (0.0f64, 0.0f64);
            for dr in -1..=1i64 {
                for dc in -1..=1i64 {
                    if (dr, dc) != (0, 0) && h(row + dr, col + dc) < base {
                        ex += dc as f64;
                        ey -= dr as f64;
                    }
                }
            }
            let el = (ex * ex + ey * ey).sqrt();
            if el > 1e-6 {
                ex /= el;
                ey /= el;
            }
            for sr in 0..F3_SUBDIV {
                for sc in 0..F3_SUBDIV {
                    let (fr, fc) = (sr as f64 / F3_SUBDIV as f64, sc as f64 / F3_SUBDIV as f64);
                    let mut ih = base;
                    if sr > 0 || sc > 0 {
                        let nr = (row + (sr > 0) as i64).min(rows as i64 - 1);
                        let nc = (col + (sc > 0) as i64).min(cols as i64 - 1);
                        ih = h(row, col) * (1.0 - fr) * (1.0 - fc) + h(nr, col) * fr * (1.0 - fc) + h(row, nc) * (1.0 - fr) * fc + h(nr, nc) * fr * fc;
                        if ih <= 0.0 {
                            continue;
                        }
                    }
                    let ox = (col as f64 + fc - cx) * F3_SX;
                    let oy = (cy - (row as f64 + fr)) * F3_SY;
                    let zr = ih * zmax;
                    let layers = if is_edge || ih < 0.15 { 2 } else { F3_LAYERS };
                    for k in 0..layers {
                        let t = k as f64 / (layers - 1) as f64 - 0.5;
                        let face = k == 0 || k == layers - 1;
                        let n = if k == 0 {
                            [g[0], g[1], -g[2]]
                        } else if k == layers - 1 {
                            g
                        } else {
                            let tn = k as f64 / (layers - 1) as f64 * 2.0 - 1.0;
                            let side = (1.0 - tn * tn).sqrt();
                            [ex * side, ey * side, tn]
                        };
                        out.push(F3Point { p: [ox, oy, t * 2.0 * zr], n, face });
                    }
                }
            }
        }
    }
    out
}

const F3_FRAMES: usize = 942;
const F3_RAMP: &[u8] = b".,-~:;=!*#$@";
const F3_COLOR: u8 = 13;

fn fetch3d_render(logo_path: &str, out_path: &str) {
    let text = fs::read_to_string(logo_path).unwrap_or_default();
    let points = f3_points(&text);
    if points.is_empty() {
        eprintln!("{logo_path}: no logo");
        process::exit(1);
    }
    let (half_w, y_center) = (WIDTH as f64 * 0.5, ROWS as f64 * 0.5);
    let logo_height = (ROWS as f64).min((WIDTH * 3 / 5) as f64);
    let k1 = 37.0 * logo_height / 36.0;
    let half = norm([F3_LIGHT[0], F3_LIGHT[1], F3_LIGHT[2] - 1.0]);
    let smax = (F3_RAMP.len() - 1) as f64;
    let mut out = vec![(F3_FRAMES & 255) as u8, WIDTH as u8, HEIGHT as u8, (F3_FRAMES >> 8) as u8];
    for frame in 0..F3_FRAMES {
        let turn = frame as f64 / F3_FRAMES as f64;
        let (a, b) = (TAU * 2.0 * turn, TAU * 3.0 * turn);
        let (ca, sa, cb, sb) = (a.cos(), a.sin(), b.cos(), b.sin());
        let spin = |v: V3| -> V3 {
            let y1 = v[1] * ca - v[2] * sa;
            let z1 = v[1] * sa + v[2] * ca;
            [v[0] * cb + z1 * sb, y1, -v[0] * sb + z1 * cb]
        };
        let mut zbuf = vec![0f64; WIDTH * ROWS];
        let mut lum = vec![0f64; WIDTH * ROWS];
        let mut face = vec![false; WIDTH * ROWS];
        for pt in &points {
            let q = spin(pt.p);
            let zc = q[2] + F3_K2;
            if zc < 0.1 {
                continue;
            }
            let ooz = 1.0 / zc;
            let xs = (half_w + 2.0 * k1 * q[0] * ooz) as i64;
            let ys = (y_center - k1 * q[1] * ooz) as i64;
            if xs < 0 || ys < 0 || xs >= WIDTH as i64 || ys >= ROWS as i64 {
                continue;
            }
            let at = ys as usize * WIDTH + xs as usize;
            if ooz > zbuf[at] {
                let n = spin(pt.n);
                let diff = dot(n, F3_LIGHT).max(0.0);
                let spec = dot(n, half).max(0.0).powi(8);
                zbuf[at] = ooz;
                lum[at] = (0.08 + 0.62 * diff + 0.30 * spec).min(1.0);
                face[at] = pt.face;
            }
        }
        for row in 0..ROWS {
            let mut glyphs = vec![0u8; WIDTH];
            for col in 0..WIDTH {
                let at = row * WIDTH + col;
                if zbuf[at] > 0.0 {
                    let ci = (lum[at] * smax + 0.5).clamp(0.0, smax) as usize;
                    glyphs[col] = F3_RAMP[ci];
                    out.push(if face[at] { F3_COLOR } else { 16 + F3_COLOR });
                } else {
                    out.push(0);
                }
            }
            out.extend_from_slice(&glyphs);
        }
    }
    if let Err(e) = fs::write(out_path, &out) {
        eprintln!("{out_path}: {e}");
        process::exit(1);
    }
}

const RP_FRAMES: usize = 300;
const RP_FORM: usize = 16;
const RP_IMPACT: usize = 58;
const RP_DROPS: [(usize, f64, f64, f64); 2] = [(58, 0.0, 0.0, 1.0), (165, 9.0, -2.0, 0.65)];
const RP_SPEED: f64 = 20.0;
const RP_WAVE: f64 = 0.95;
const RP_WIDTH: f64 = 5.5;
const RP_DECAY: f64 = 1.5;
const RP_AMP: f64 = 2.6;
const RP_ASPECT: f64 = 2.17;
const RP_LIGHT: V3 = [-0.48, -0.62, 0.62];

fn rp_height(r: f64, t: f64) -> f64 {
    if t <= 0.0 {
        return 0.0;
    }
    let packet = |delay: f64, amp: f64| -> f64 {
        let tt = t - delay;
        if tt <= 0.0 {
            return 0.0;
        }
        let d = r - RP_SPEED * tt;
        amp * (-(d / RP_WIDTH).powi(2)).exp() * (RP_WAVE * d).cos() * (-tt / RP_DECAY).exp() / (r * 0.18 + 1.0).sqrt()
    };
    packet(0.0, RP_AMP) + packet(0.42, RP_AMP * 0.45)
}

fn rp_slope(r: f64, t: f64) -> f64 {
    let e = 0.05;
    (rp_height(r + e, t) - rp_height(r - e, t)) / (2.0 * e)
}

fn rp_hash(i: u64) -> f64 {
    let mut h = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 29;
    (h & 0xffff) as f64 / 65535.0
}

const RAIN_FRAMES: usize = 240;
const RAIN_FPS: f64 = 48.0;
const RAIN_DROPS: usize = 72;
const RAIN_SPEED: f64 = 24.0;
const RAIN_LIFE: f64 = 0.55;
const RAIN_WIDTH: f64 = 2.0;
const RAIN_WAVE: f64 = 1.8;
const RAIN_AMP: f64 = 1.5;
const RAIN_HIT: f64 = 0.05;
const RAIN_BEVEL: f64 = 4.0;
const RAIN_LIFT: f64 = 1.6;
const RAIN_SHADOW: (usize, usize) = (2, 1);
const RAIN_SCALE: f64 = 0.95;

fn rain_outline(mask: &Mask, to_mask: &dyn Fn(f64, f64) -> (f64, f64)) -> (Vec<u8>, Vec<u8>) {
    let (sx, sy) = (4usize, 8usize);
    let (gw, gh) = (WIDTH * sx, ROWS * sy);
    let mut ids = vec![0u8; gw * gh];
    for gy in 0..gh {
        for gx in 0..gw {
            let (u, v) = to_mask((gx as f64 + 0.5) / sx as f64, (gy as f64 + 0.5) / sy as f64);
            ids[gy * gw + gx] = mask.id_at(u, v);
        }
    }
    let on = |x: i64, y: i64| -> f64 {
        if x < 0 || y < 0 || x >= gw as i64 || y >= gh as i64 {
            0.0
        } else {
            (ids[y as usize * gw + x as usize] != 0) as u8 as f64
        }
    };
    let mut glyphs = vec![0u8; WIDTH * ROWS];
    let mut cell_id = vec![0u8; WIDTH * ROWS];
    for row in 0..ROWS {
        for col in 0..WIDTH {
            let (mut hits, mut first) = (0usize, 0usize);
            let (mut jxx, mut jyy, mut jxy) = (0.0f64, 0.0f64, 0.0f64);
            for j in 0..sy {
                for i in 0..sx {
                    let (x, y) = ((col * sx + i) as i64, (row * sy + j) as i64);
                    let id = ids[y as usize * gw + x as usize];
                    if id != 0 {
                        hits += 1;
                        first += (id == 1) as usize;
                    }
                    let gxv = on(x + 1, y) - on(x - 1, y);
                    let gyv = (on(x, y + 1) - on(x, y - 1)) * 0.5;
                    jxx += gxv * gxv;
                    jyy += gyv * gyv;
                    jxy += gxv * gyv;
                }
            }
            let share = hits as f64 / (sx * sy) as f64;
            if !(0.04..0.5).contains(&share) || jxx + jyy == 0.0 {
                continue;
            }
            let line = ((0.5 * (2.0 * jxy).atan2(jxx - jyy)).to_degrees() + 90.0).rem_euclid(180.0);
            glyphs[row * WIDTH + col] = match line {
                a if !(22.5..157.5).contains(&a) => b'_',
                a if a < 67.5 => b'\\',
                a if a < 112.5 => b'|',
                _ => b'/',
            };
            cell_id[row * WIDTH + col] = if first * 2 >= hits.max(1) { 1 } else { 2 };
        }
    }
    (glyphs, cell_id)
}

fn ripple_render(mask_path: &str, out_path: &str) {
    let mask = Mask::load(mask_path);
    let to_mask = |x: f64, y: f64| -> (f64, f64) { ((x / WIDTH as f64 - 0.5) / RAIN_SCALE, (y / ROWS as f64 - 0.5) / RAIN_SCALE) };
    let (outline, outline_id) = rain_outline(&mask, &to_mask);
    let mut drops: Vec<(f64, f64, f64)> = Vec::new();
    let mut seed = 1u64;
    while drops.len() < RAIN_DROPS {
        let x = 1.0 + rp_hash(seed) * (WIDTH as f64 - 2.0);
        let y = 0.5 + rp_hash(seed + 7777) * (ROWS as f64 - 1.0);
        seed += 1;
        let (u, v) = to_mask(x, y);
        if mask.id_at(u, v) == 0 {
            continue;
        }
        let start = (drops.len() as f64 + 0.8 * rp_hash(seed + 31337)) / RAIN_DROPS as f64 * RAIN_FRAMES as f64;
        drops.push((x, y, start));
    }
    let light = norm(RP_LIGHT);
    let half = norm([light[0], light[1], light[2] + 1.0]);
    let rest = light[2];
    let (sx, sy) = (4usize, 8usize);
    let mut out = vec![(RAIN_FRAMES & 255) as u8, WIDTH as u8, HEIGHT as u8, (RAIN_FRAMES >> 8) as u8];
    for frame in 0..RAIN_FRAMES {
        let live: Vec<(f64, f64, f64)> = drops
            .iter()
            .filter_map(|&(x, y, start)| {
                let tt = (frame as f64 - start).rem_euclid(RAIN_FRAMES as f64) / RAIN_FPS;
                (tt < RAIN_LIFE).then_some((x, y, tt))
            })
            .collect();
        let mut values = vec![0u8; WIDTH * ROWS];
        let mut glyphs = vec![0u8; WIDTH * ROWS];
        for row in 0..ROWS {
            for col in 0..WIDTH {
                let (mut hits, mut first, mut rim, mut gx, mut gy) = (0usize, 0usize, 0.0f64, 0.0f64, 0.0f64);
                for j in 0..sy {
                    for i in 0..sx {
                        let x = col as f64 + (i as f64 + 0.5) / sx as f64;
                        let y = row as f64 + (j as f64 + 0.5) / sy as f64;
                        let (u, v) = to_mask(x, y);
                        let id = mask.id_at(u, v);
                        if id == 0 {
                            continue;
                        }
                        let (mut hx, mut hy) = (0.0, 0.0);
                        for &(dxp, dyp, tt) in &live {
                            let dx = x - dxp;
                            let dy = (y - dyp) * RP_ASPECT;
                            let r = (dx * dx + dy * dy).sqrt().max(0.001);
                            let d = r - RAIN_SPEED * tt;
                            if d.abs() > 3.0 * RAIN_WIDTH {
                                continue;
                            }
                            let amp = RAIN_AMP * (1.0 - tt / RAIN_LIFE).powi(2);
                            let g = (-(d / RAIN_WIDTH).powi(2)).exp();
                            let (c, s) = ((RAIN_WAVE * d).cos(), (RAIN_WAVE * d).sin());
                            let slope = amp * g * (-RAIN_WAVE * s - 2.0 * d / (RAIN_WIDTH * RAIN_WIDTH) * c);
                            hx += slope * dx / r;
                            hy += slope * dy / r;
                        }
                        hits += 1;
                        first += (id == 1) as usize;
                        let depth = mask.dist_at(u, v);
                        rim += depth;
                        let (mx, my) = mask.slope(u, v);
                        let bevel = if depth < RAIN_BEVEL { RAIN_LIFT * (1.0 - depth / RAIN_BEVEL).powf(0.7) } else { 0.0 };
                        gx += hx + mx * bevel;
                        gy += hy + my * bevel * RP_ASPECT;
                    }
                }
                let at = row * WIDTH + col;
                let share = hits as f64 / (sx * sy) as f64;
                let color = |shade: u8, id: u8| if id == 2 { 16 + shade } else { shade };
                if outline[at] != 0 {
                    let lit = if hits > 0 { 11 } else { 9 };
                    values[at] = color(lit, outline_id[at]);
                    glyphs[at] = outline[at];
                    continue;
                }
                if share < 0.5 {
                    continue;
                }
                let k = hits as f64;
                let n = norm([-gx / k, -gy / k, 1.0]);
                let diff = dot(n, light);
                let spec = dot(n, half).max(0.0).powi(36);
                let edge = (1.0 - (rim / k / 2.5).min(1.0)) * 0.18;
                let level = (0.5 + 1.6 * (diff - rest) + 0.8 * spec - edge).clamp(0.0, 1.0);
                let shade = (1.0 + (level * 14.0).round()).clamp(1.0, 15.0) as u8;
                values[at] = color(shade, if first * 2 >= hits { 1 } else { 2 });
            }
        }
        for &(dxp, dyp, tt) in &live {
            if tt < RAIN_HIT {
                let at = (dyp as usize).min(ROWS - 1) * WIDTH + (dxp as usize).min(WIDTH - 1);
                if outline[at] == 0 {
                    values[at] = (values[at] & 16) | 15;
                    glyphs[at] = if tt < RAIN_HIT * 0.5 { b'*' } else { b'o' };
                }
            }
        }
        let mut shadow = vec![false; WIDTH * ROWS];
        for row in RAIN_SHADOW.1..ROWS {
            for col in RAIN_SHADOW.0..WIDTH {
                let at = row * WIDTH + col;
                shadow[at] = values[at] == 0 && values[(row - RAIN_SHADOW.1) * WIDTH + col - RAIN_SHADOW.0] != 0;
            }
        }
        for row in 0..ROWS {
            for col in 0..WIDTH {
                let at = row * WIDTH + col;
                out.push(if shadow[at] { 16 + 2 } else { values[at] });
            }
            out.extend_from_slice(&glyphs[row * WIDTH..(row + 1) * WIDTH]);
        }
    }
    if let Err(e) = fs::write(out_path, &out) {
        eprintln!("{out_path}: {e}");
        process::exit(1);
    }
}

fn rain_still(frames_path: &str, out_path: &str) {
    let src = fs::read(frames_path).unwrap_or_default();
    if src.len() < 4 + WIDTH * HEIGHT {
        eprintln!("{frames_path}: not a frames file");
        process::exit(1);
    }
    let base = &src[4..4 + WIDTH * HEIGHT];
    let value = |row: usize, col: usize| base[row * 2 * WIDTH + col];
    let glyph = |row: usize, col: usize| base[(row * 2 + 1) * WIDTH + col];
    let fill = |row: usize, col: usize| glyph(row, col) <= b' ' && value(row, col) & 15 != 0;
    let mut drops: Vec<(f64, f64, f64)> = Vec::new();
    let mut seed = 1u64;
    while drops.len() < RAIN_DROPS {
        let col = (rp_hash(seed) * WIDTH as f64) as usize;
        let row = (rp_hash(seed + 7777) * ROWS as f64) as usize;
        seed += 1;
        if col >= WIDTH || row >= ROWS || !fill(row, col) {
            continue;
        }
        let start = (drops.len() as f64 + 0.8 * rp_hash(seed + 31337)) / RAIN_DROPS as f64 * RAIN_FRAMES as f64;
        drops.push((col as f64 + 0.5, row as f64 + 0.5, start));
    }
    let light = norm(RP_LIGHT);
    let half = norm([light[0], light[1], light[2] + 1.0]);
    let rest = light[2];
    let mut out = vec![(RAIN_FRAMES & 255) as u8, WIDTH as u8, HEIGHT as u8, (RAIN_FRAMES >> 8) as u8];
    for frame in 0..RAIN_FRAMES {
        let live: Vec<(f64, f64, f64)> = drops
            .iter()
            .filter_map(|&(x, y, start)| {
                let tt = (frame as f64 - start).rem_euclid(RAIN_FRAMES as f64) / RAIN_FPS;
                (tt < RAIN_LIFE).then_some((x, y, tt))
            })
            .collect();
        let mut frame_bytes = base.to_vec();
        for row in 0..ROWS {
            for col in 0..WIDTH {
                if !fill(row, col) {
                    continue;
                }
                let (x, y) = (col as f64 + 0.5, row as f64 + 0.5);
                let (mut gx, mut gy) = (0.0, 0.0);
                for &(dxp, dyp, tt) in &live {
                    let dx = x - dxp;
                    let dy = (y - dyp) * RP_ASPECT;
                    let r = (dx * dx + dy * dy).sqrt().max(0.001);
                    let d = r - RAIN_SPEED * tt;
                    if d.abs() > 3.0 * RAIN_WIDTH {
                        continue;
                    }
                    let amp = RAIN_AMP * (1.0 - tt / RAIN_LIFE).powi(2);
                    let g = (-(d / RAIN_WIDTH).powi(2)).exp();
                    let (c, s) = ((RAIN_WAVE * d).cos(), (RAIN_WAVE * d).sin());
                    let slope = amp * g * (-RAIN_WAVE * s - 2.0 * d / (RAIN_WIDTH * RAIN_WIDTH) * c);
                    gx += slope * dx / r;
                    gy += slope * dy / r;
                }
                let n = norm([-gx, -gy, 1.0]);
                let delta = 1.6 * (dot(n, light) - rest) + 0.8 * dot(n, half).max(0.0).powi(36) * ((gx * gx + gy * gy) > 1e-4) as u8 as f64;
                let v = value(row, col);
                let shade = ((v & 15) as f64 + delta * 14.0).round().clamp(1.0, 15.0) as u8;
                for half_row in 0..2 {
                    let at = (row * 2 + half_row) * WIDTH + col;
                    if half_row == 0 || base[at] & 15 != 0 {
                        frame_bytes[at] = (base[at] & 16) | shade;
                    }
                }
            }
        }
        for &(dxp, dyp, tt) in &live {
            if tt < RAIN_HIT {
                let (row, col) = (dyp as usize, dxp as usize);
                frame_bytes[row * 2 * WIDTH + col] = (value(row, col) & 16) | 15;
                frame_bytes[(row * 2 + 1) * WIDTH + col] = if tt < RAIN_HIT * 0.5 { b'*' } else { b'o' };
            }
        }
        out.extend_from_slice(&frame_bytes);
    }
    if let Err(e) = fs::write(out_path, &out) {
        eprintln!("{out_path}: {e}");
        process::exit(1);
    }
}

const RT_FRAMES: usize = 240;
const RT_FPS: f64 = 48.0;
const RT_DROPS: usize = 40;
const RT_LIFE: f64 = 0.9;
const RT_SPEED: f64 = 16.0;
const RT_FALL: usize = 4;

fn rain_text(logo_path: &str, out_path: &str) {
    let text = fs::read_to_string(logo_path).unwrap_or_default();
    let lines: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
    let rows = lines.len();
    let width = lines.iter().map(|l| l.len()).max().unwrap_or(0) + 1;
    let logo = |r: usize, c: usize| -> char { lines.get(r).and_then(|l| l.get(c)).copied().unwrap_or(' ') };
    let byte = |ch: char| -> u8 { if ch == '¯' { 0xAF } else if ch.is_ascii() { ch as u8 } else { b'-' } };
    let mut drops: Vec<(usize, usize, f64)> = Vec::new();
    let mut seed = 3u64;
    while drops.len() < RT_DROPS {
        let c = 1 + (rp_hash(seed) * (width - 2) as f64) as usize;
        let r = 1 + (rp_hash(seed + 999) * (rows - 2) as f64) as usize;
        seed += 1;
        if logo(r, c) == ' ' {
            continue;
        }
        let start = (drops.len() as f64 + 0.8 * rp_hash(seed + 4242)) / RT_DROPS as f64 * RT_FRAMES as f64;
        drops.push((r, c, start));
    }
    let mut out = vec![(RT_FRAMES & 255) as u8, width as u8, (rows * 2) as u8, (RT_FRAMES >> 8) as u8];
    for frame in 0..RT_FRAMES {
        let mut value = vec![0u8; width * rows];
        let mut glyph = vec![0u8; width * rows];
        for r in 0..rows {
            for c in 0..width {
                let ch = logo(r, c);
                if ch != ' ' {
                    value[r * width + c] = if ch == '#' { 16 + 10 } else { 12 };
                    glyph[r * width + c] = byte(ch);
                }
            }
        }
        for r in 0..rows {
            for c in 0..width {
                if logo(r, c) == ' ' {
                    continue;
                }
                let mut h = 0.0;
                for &(dr, dc, start) in &drops {
                    let tt = (frame as f64 - start).rem_euclid(RT_FRAMES as f64) / RT_FPS;
                    if tt >= RT_LIFE {
                        continue;
                    }
                    let dx = c as f64 - dc as f64;
                    let dy = (r as f64 - dr as f64) * RP_ASPECT;
                    let d = (dx * dx + dy * dy).sqrt() - RT_SPEED * tt;
                    let g = (-(d / 2.2).powi(2)).exp();
                    h += g * (1.7 * d).cos() * (1.0 - tt / RT_LIFE).powi(2);
                }
                let at = r * width + c;
                let base = value[at] & 15;
                let shade = (base as f64 + h * 6.0).round().clamp(2.0, 15.0) as u8;
                value[at] = (value[at] & 16) | shade;
            }
        }
        for r in 0..rows {
            out.extend_from_slice(&value[r * width..(r + 1) * width]);
            out.extend_from_slice(&glyph[r * width..(r + 1) * width]);
        }
    }
    if let Err(e) = fs::write(out_path, &out) {
        eprintln!("{out_path}: {e}");
        process::exit(1);
    }
}

const VX_COLS: usize = 54;
const VX_ROWS: usize = 25;
const VX_FRAMES: usize = 288;
const VX_LAYERS: usize = 6;
const VX_ZOOM: f64 = 1.9;
const VX_TWIST: f64 = 40.0;
const VX_ASPECT: f64 = 2.17;
const VX_LOGO: f64 = 11.0;
const VX_FOG: f64 = 11.0;
const VX_SS: usize = 3;
const VX_BRAILLE: u8 = 0x80;
const VX_WAVE_WIDTH: f64 = 0.1;
const VX_WAVE_PUSH: f64 = 0.07;
const VX_EDGE: f64 = 23.0;
const VX_EDGE_FADE: f64 = 5.0;

#[derive(Clone, Copy, PartialEq)]
enum Vx {
    Empty,
    Hatch,
    Dots,
    Moon,
    Stripe,
}

fn vx_noise(a: f64, seed: f64) -> f64 {
    value_noise(a.cos() * 1.8 + seed, a.sin() * 1.8 + seed * 0.7)
}

fn vx_wrap(a: f64) -> f64 {
    (a + TAU / 2.0).rem_euclid(TAU) - TAU / 2.0
}

fn vx_layer(f: f64, a: f64, id: usize) -> Vx {
    let seed = id as f64 * 13.0 + 1.0;
    let moon_at = rp_hash(id as u64 + 77) * TAU;
    let moon = id % 3 != 2;
    let span = TAU * 0.55;
    let off = vx_wrap(a - moon_at);
    if moon && off.abs() < span / 2.0 {
        let u = off / span + 0.5;
        let thick = 0.5 * (std::f64::consts::PI * u).sin().powf(0.7);
        let outer = 0.97;
        if f > outer - thick && f < outer {
            let mid = outer - thick * 0.3;
            if thick > 0.3 && (f - mid).abs() < 0.05 {
                return Vx::Stripe;
            }
            return Vx::Moon;
        }
    }
    if (0.0..0.3).contains(&f) && vx_noise(a, seed) > 0.3 {
        return Vx::Hatch;
    }
    if (0.36..0.42).contains(&f) && vx_noise(a, seed + 40.0) > 0.5 {
        return Vx::Hatch;
    }
    if (0.47..0.52).contains(&f) && vx_noise(a, seed + 80.0) > 0.3 {
        return Vx::Dots;
    }
    if (0.66..0.70).contains(&f) && vx_noise(a, seed + 120.0) > 0.4 {
        return Vx::Dots;
    }
    Vx::Empty
}

fn vx_tunnel(x: f64, y: f64, t: f64) -> Vx {
    let r = (x * x + y * y).sqrt().max(1e-6);
    let lr = r.ln() / VX_ZOOM.ln();
    let s = lr - t;
    let k = s.floor();
    let id = (k as i64).rem_euclid(VX_LAYERS as i64) as usize;
    let a = y.atan2(x) - VX_TWIST.to_radians() * lr;
    vx_layer(s - k, a, id)
}

fn vortex_render(mask_path: &str, out_path: &str) {
    let mask = Mask::load(mask_path);
    let n = mask.n as f64;
    let mut out = vec![(VX_FRAMES & 255) as u8, VX_COLS as u8, (VX_ROWS * 2) as u8, (VX_FRAMES >> 8) as u8];
    let (cx, cy) = (VX_COLS as f64 / 2.0, VX_ROWS as f64 / 2.0);
    let logo_at = |x: f64, y: f64, turn: f64, wave: f64| -> (u8, f64) {
        let r = (x * x + y * y).sqrt() / VX_LOGO;
        let (mut push, mut glow) = (0.0, 0.0);
        for j in 1..7 {
            let front = VX_ZOOM.powf(wave - j as f64) * 1.1;
            let d = (r - front) / (VX_WAVE_WIDTH * front);
            let g = (-d * d).exp();
            push += VX_WAVE_PUSH * front * g;
            glow += g;
        }
        let k = if r > 1e-6 { (r - push).max(0.0) / r } else { 1.0 };
        let (x, y) = (x * k, y * k);
        let (c, s) = (turn.cos(), turn.sin());
        let (u, v) = (c * x + s * y, -s * x + c * y);
        let (px, py) = ((u / VX_LOGO * 0.5 + 0.5) * n, (v / VX_LOGO * 0.5 + 0.5) * n);
        if px < 0.0 || py < 0.0 || px >= n || py >= n {
            return (0, glow);
        }
        (mask.id[py as usize * mask.n + px as usize], glow)
    };
    for frame in 0..VX_FRAMES {
        let t = frame as f64 / VX_FRAMES as f64 * VX_LAYERS as f64;
        let turn = VX_TWIST.to_radians() * t;
        let wave = t.fract();
        let mut value = vec![0u8; VX_COLS * VX_ROWS];
        let mut glyph = vec![0u8; VX_COLS * VX_ROWS];
        for row in 0..VX_ROWS {
            for col in 0..VX_COLS {
                let at = row * VX_COLS + col;
                let mut bits = 0u8;
                let mut ids = [0usize; 3];
                let mut glow = 0.0f64;
                for dy in 0..4 {
                    for dx in 0..2 {
                        let mut hits = [0usize; 3];
                        for sy in 0..VX_SS {
                            for sx in 0..VX_SS {
                                let px = col as f64 + (dx as f64 + (sx as f64 + 0.5) / VX_SS as f64) / 2.0 - cx;
                                let py = (row as f64 + (dy as f64 + (sy as f64 + 0.5) / VX_SS as f64) / 4.0 - cy) * VX_ASPECT;
                                let (id, g) = logo_at(px, py, turn, wave);
                                hits[id as usize] += 1;
                                glow = glow.max(g);
                            }
                        }
                        let lit = hits[1] + hits[2];
                        if lit * 2 < VX_SS * VX_SS {
                            continue;
                        }
                        bits |= [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]][dy][dx];
                        ids[if hits[1] >= hits[2] { 1 } else { 2 }] += 1;
                    }
                }
                let inside = {
                    let px = col as f64 + 0.5 - cx;
                    let py = (row as f64 + 0.5 - cy) * VX_ASPECT;
                    (px * px + py * py).sqrt() < VX_LOGO * 1.1
                };
                if bits != 0 {
                    let shade = (12.0 + 3.0 * glow.min(1.0)).round() as u8;
                    value[at] = VX_BRAILLE | if ids[1] >= ids[2] { 16 + shade } else { shade };
                    glyph[at] = bits;
                    continue;
                }
                if inside {
                    continue;
                }
                let mut count = [0usize; 5];
                for sy in 0..VX_SS {
                    for sx in 0..VX_SS {
                        let px = col as f64 + (sx as f64 + 0.5) / VX_SS as f64 - cx;
                        let py = (row as f64 + (sy as f64 + 0.5) / VX_SS as f64 - cy) * VX_ASPECT;
                        count[vx_tunnel(px, py, t) as usize] += 1;
                    }
                }
                let (best, hits) = (1..5).map(|i| (i, count[i])).max_by_key(|&(_, c)| c).unwrap();
                if hits * 2 < VX_SS * VX_SS {
                    continue;
                }
                let px = col as f64 + 0.5 - cx;
                let py = (row as f64 + 0.5 - cy) * VX_ASPECT;
                let r = (px * px + py * py).sqrt();
                let a = py.atan2(px);
                let spin = TAU * t / VX_LAYERS as f64;
                let edge = VX_EDGE + 1.8 * (5.0 * a - 3.0 * spin).sin() + 1.0 * (8.0 * a + 4.0 * spin).sin();
                let fade = ((edge - r) / VX_EDGE_FADE).clamp(0.0, 1.0);
                if fade <= hash(col as i64, row as i64) {
                    continue;
                }
                let depth = ((r - VX_LOGO) / (VX_FOG * 2.0)).clamp(0.0, 1.0);
                let shade = (3.0 + 8.0 * depth.powf(0.7) * fade.sqrt()).round().max(2.0) as u8;
                let ch = match best {
                    1 => b'/',
                    2 => b",;.:;,"[(col * 7 + row * 3) % 6],
                    3 => b'p',
                    _ => b'2',
                };
                value[at] = shade;
                glyph[at] = ch;
            }
        }
        for row in 0..VX_ROWS {
            out.extend_from_slice(&value[row * VX_COLS..(row + 1) * VX_COLS]);
            out.extend_from_slice(&glyph[row * VX_COLS..(row + 1) * VX_COLS]);
        }
    }
    if let Err(e) = fs::write(out_path, &out) {
        eprintln!("{out_path}: {e}");
        process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() == 3 && args[0] == "vortex" {
        return vortex_render(&args[1], &args[2]);
    }
    if args.len() == 3 && args[0] == "raintext" {
        return rain_text(&args[1], &args[2]);
    }
    if args.len() == 3 && args[0] == "rainstill" {
        return rain_still(&args[1], &args[2]);
    }
    if args.len() == 3 && args[0] == "ripple" {
        return ripple_render(&args[1], &args[2]);
    }
    if args.len() == 3 && args[0] == "fetch3d" {
        return fetch3d_render(&args[1], &args[2]);
    }
    if args.len() == 4 && args[0] == "shape" {
        return shape_render(&args[1], &args[2], &args[3]);
    }
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
            matmul(&matmul(&rot_y(ease(TAU * 2.0 * turn)), &rot_x(ease(TAU * 3.0 * turn))), &rot_z(TAU * around * turn + start))
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
                if (hits as f64) < EDGE_COVER * total || (OUTLINES && !lined && (hits as f64) < cover * total) {
                    continue;
                }
                let level = ((sum / hits.max(1) as f64 - 0.14) / 1.02).clamp(0.0, 1.0).powf(FILL_GAMMA);
                let level = if OUTLINES { level } else { level * (hits as f64 / total / COVER).min(1.0) };
                let shade = 1 + (level * 14.0).round() as u8;
                cells[at] = if first * 2 >= hits { shade } else { 16 + shade };
                if lined && OUTLINES {
                    let line = (0.5 * (2.0 * jxy).atan2(jxx - jyy)).to_degrees() + 90.0;
                    let line = line.rem_euclid(180.0);
                    let center = match was_glyph { b'_' => Some(0.0), b'\\' => Some(45.0), b'|' => Some(90.0), b'/' => Some(135.0), _ => None };
                    let near = center.is_some_and(|c: f64| { let d = (line - c).rem_euclid(180.0); d.min(180.0 - d) <= 22.5 + HOLD_ANGLE });
                    glyphs[at] = if near { was_glyph } else { match line {
                        a if !(22.5..157.5).contains(&a) => b'_',
                        a if a < 67.5 => b'\\',
                        a if a < 112.5 => b'|',
                        _ => b'/',
                    } };
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
            data[cell(f, 3, 1)] = 9;
            data[cell(f, 3, 1) + WIDTH] = b'/';
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
    fn lone_outline_marks_drop_and_strokes_stay() {
        let mut data = blank();
        for f in 0..FRAMES {
            for c in 3..8 {
                data[cell(f, 6, c)] = 9;
                data[cell(f, 6, c) + WIDTH] = b'_';
            }
            data[cell(f, 10, 10)] = 9;
            data[cell(f, 10, 11)] = 9;
            data[cell(f, 10, 10) + WIDTH] = b'|';
        }
        let (out, _) = steady(&data);
        assert_eq!(out[cell(40, 6, 5) + WIDTH], b'_');
        assert_eq!(out[cell(40, 10, 10) + WIDTH], 0);
        assert_eq!(out[cell(40, 10, 10)], 9);
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
            data[cell(f, 4, 6)] = 9;
            data[cell(f, 6, 6)] = 9;
            if f >= 120 { data[cell(f, 6, 6) + WIDTH] = b'\\'; } else { data[cell(f, 4, 6) + WIDTH] = b'/'; }
            data[cell(f, 5, 5) + WIDTH] = if f == 30 || f == 31 { b'_' } else if f >= 120 { b'\\' } else { b'/' };
        }
        let (out, _) = steady(&data);
        assert_eq!(out[cell(30, 5, 5) + WIDTH], b'/');
        assert_eq!(out[cell(31, 5, 5) + WIDTH], b'/');
        assert_eq!(out[cell(150, 5, 5) + WIDTH], b'\\');
    }
}
