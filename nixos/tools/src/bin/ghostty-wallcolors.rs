use regex::Regex;
use std::collections::BTreeMap;
use std::process::Command;
use std::time::Duration;
use std::{env, fs, path::Path};
use tools::{capture, current_wallpaper, home, run_quiet};

#[derive(Clone, Copy)]
struct Found {
    count: u64,
    h: f64,
    l: f64,
    s: f64,
    chroma: f64,
}

type Hsl = (f64, f64, f64);

fn rgb_to_hls(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let (maxc, minc) = (r.max(g).max(b), r.min(g).min(b));
    let (sumc, rangec) = (maxc + minc, maxc - minc);
    let l = sumc / 2.0;
    if minc == maxc {
        return (0.0, l, 0.0);
    }
    let s = if l <= 0.5 { rangec / sumc } else { rangec / (2.0 - maxc - minc) };
    let (rc, gc, bc) = ((maxc - r) / rangec, (maxc - g) / rangec, (maxc - b) / rangec);
    let h = if r == maxc {
        bc - gc
    } else if g == maxc {
        2.0 + rc - bc
    } else {
        4.0 + gc - rc
    };
    ((h / 6.0).rem_euclid(1.0), l, s)
}

fn hls_to_rgb(h: f64, l: f64, s: f64) -> (f64, f64, f64) {
    if s == 0.0 {
        return (l, l, l);
    }
    let m2 = if l <= 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let m1 = 2.0 * l - m2;
    let v = |hue: f64| {
        let hue = hue.rem_euclid(1.0);
        if hue < 1.0 / 6.0 {
            m1 + (m2 - m1) * hue * 6.0
        } else if hue < 0.5 {
            m2
        } else if hue < 2.0 / 3.0 {
            m1 + (m2 - m1) * (2.0 / 3.0 - hue) * 6.0
        } else {
            m1
        }
    };
    (v(h + 1.0 / 3.0), v(h), v(h - 1.0 / 3.0))
}

fn wallpaper() -> Option<String> {
    match env::var("NOCTALIA_WALLPAPER_PATH") {
        Ok(p) if !p.is_empty() && Path::new(&p).exists() => Some(p),
        _ => current_wallpaper(),
    }
}

fn colours(path: &str) -> Vec<Found> {
    let out = capture(
        Command::new("magick").args([&format!("{path}[0]"), "-sample", "400x", "-colors", "96", "-format", "%c", "histogram:info:"]),
        Duration::from_secs(40),
    )
    .unwrap_or_default();
    let line = Regex::new(r"(?m)^\s*(\d+):.*?#([0-9A-Fa-f]{6})").unwrap();
    line.captures_iter(&out)
        .map(|c| {
            let hex = &c[2];
            let ch = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap() as f64 / 255.0;
            let (r, g, b) = (ch(0), ch(2), ch(4));
            let (h, l, s) = rgb_to_hls(r, g, b);
            Found { count: c[1].parse().unwrap_or(0), h, l, s, chroma: r.max(g).max(b) - r.min(g).min(b) }
        })
        .collect()
}

fn shade((h, s, l): Hsl) -> String {
    let (r, g, b) = hls_to_rgb(h, l.min(0.95), s.min(1.0));
    let c = |x: f64| (x * 255.0).round_ties_even() as u8;
    format!("#{:02x}{:02x}{:02x}", c(r), c(g), c(b))
}

fn apart(a: f64, b: f64) -> f64 {
    (a - b).abs().min(1.0 - (a - b).abs())
}

fn rgb((h, s, l): Hsl) -> (f64, f64, f64) {
    hls_to_rgb(h, l, s)
}

fn distance(a: Hsl, b: Hsl) -> f64 {
    let (one, two) = (rgb(a), rgb(b));
    ((one.0 - two.0).powi(2) + (one.1 - two.1).powi(2) + (one.2 - two.2).powi(2)).sqrt()
}

fn first_max<T: Copy>(items: &[T], key: impl Fn(&T) -> f64) -> T {
    let mut best = items[0];
    for item in &items[1..] {
        if key(item) > key(&best) {
            best = *item;
        }
    }
    best
}

fn spread(found: &[Found]) -> Vec<Hsl> {
    let mut tinted: Vec<Found> = found.iter().copied().filter(|c| c.chroma > 0.08).collect();
    if tinted.is_empty() {
        return (0..6).map(|n| (0.0, 0.0, 0.58 + 0.05 * n as f64)).collect();
    }
    tinted.sort_by(|a, b| b.count.cmp(&a.count));
    let mut groups: Vec<Vec<Found>> = Vec::new();
    for c in tinted {
        match groups.iter_mut().find(|g| apart(c.h, g[0].h) <= 0.05) {
            Some(group) => group.push(c),
            None => groups.push(vec![c]),
        }
    }
    let total = |g: &Vec<Found>| g.iter().map(|c| c.count).sum::<u64>();
    groups.sort_by(|a, b| total(b).cmp(&total(a)));
    let top = total(&groups[0]) as f64;
    let groups: Vec<_> = groups.into_iter().filter(|g| total(g) as f64 >= top * 0.3).take(6).collect();
    let mut mains = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        let first = first_max(group, |c| c.count as f64);
        let share = total(group) as f64;
        let deep: Vec<Found> = group
            .iter()
            .copied()
            .filter(|c| c.count as f64 >= share * 0.02 && c.chroma > 0.3 && c.l < 0.45 && (c.h <= 0.045 || c.h >= 0.97))
            .collect();
        if !deep.is_empty() && index == 0 {
            let first = first_max(&deep, |c| c.chroma);
            mains.push((first.h, first.s, first.l.max(0.45)));
        } else {
            mains.push((first.h, first.s, first.l.max(0.58)));
        }
    }
    let mut chosen = mains.clone();
    let pair = &mains[..mains.len().min(2)];
    let mut step = 0;
    while chosen.len() < 6 {
        let (h, s, l) = pair[step % pair.len()];
        let level = (step / pair.len() + 1) as f64;
        chosen.push((h, s, (l + (0.9 - l) * level / 5.0).min(0.9)));
        step += 1;
    }
    chosen
}

fn flow(colors: Vec<Hsl>) -> Vec<Hsl> {
    let mut ordered = vec![colors[0]];
    let mut left = colors[1..].to_vec();
    while !left.is_empty() {
        let last = *ordered.last().unwrap();
        let mut at = 0;
        for i in 1..left.len() {
            if distance(left[i], last) < distance(left[at], last) {
                at = i;
            }
        }
        ordered.push(left.remove(at));
    }
    ordered
}

fn palette(found: &[Found]) -> BTreeMap<u32, String> {
    let colors = flow(spread(found));
    let (base_h, base_s) = (colors[0].0, colors[0].1);
    let mut slots = BTreeMap::new();
    slots.insert(0, shade((base_h, base_s * 0.4, 0.14)));
    slots.insert(8, shade((base_h, base_s * 0.4, 0.45)));
    for (slot, &(h, s, l)) in (1..=6).zip(&colors) {
        slots.insert(slot, shade((h, s, l)));
        slots.insert(slot + 8, shade((h, s, (l + 0.08).min(0.92))));
    }
    slots.insert(7, shade((base_h, base_s * 0.3, 0.8)));
    slots.insert(15, shade((base_h, base_s * 0.3, 0.93)));
    let top = first_max(&colors, |&c| {
        let (r, g, b) = rgb(c);
        r.max(g).max(b) - r.min(g).min(b)
    });
    slots.insert(13, shade(top));
    slots.insert(67, slots[&4].clone());
    slots.insert(208, slots[&3].clone());
    slots
}

fn main() {
    let Some(path) = wallpaper() else {
        eprintln!("ghostty-wallcolors: no wallpaper set");
        std::process::exit(1);
    };
    let slots = palette(&colours(&path));
    let mut text = String::from("# Written by ghostty-wallcolors from the wallpaper's real colours.\n");
    for (slot, colour) in &slots {
        text += &format!("palette = {slot}={colour}\n");
    }
    let out = home() + "/.config/ghostty/themes/noctalia-extra";
    if let Err(e) = fs::write(&out, text) {
        eprintln!("ghostty-wallcolors: {out}: {e}");
        std::process::exit(1);
    }
    run_quiet("pkill", &["-USR2", "-x", ".ghostty-wrappe"]);
}
