use image::{imageops::FilterType, DynamicImage};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};
use std::{env, fs, thread};
use tools::{expand, wait_timeout};

const STATIC_DIR: &str = "~/Pictures/Wallpapers";
const LIVE_DIR: &str = "~/Pictures/Live Wallpapers";
const WPE_DIR: &str = "~/.local/share/Steam/steamapps/workshop/content/431960";
const CACHE: &str = "~/.cache/walls";
const THUMB_WIDTH: u32 = 384;
const IMAGE_EXT: [&str; 6] = ["jpg", "jpeg", "png", "webp", "gif", "bmp"];
const VIDEO_EXT: [&str; 5] = ["mp4", "webm", "mkv", "mov", "gif"];

struct Entry {
    kind: &'static str,
    key: String,
    name: String,
    source: String,
    stamp: u64,
}

fn stamp(path: &str) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

fn extension(path: &str) -> String {
    Path::new(path).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn stem(path: &str) -> String {
    Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

fn files(dir: &str, exts: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(expand(dir))
        .map(|rd| rd.flatten().map(|e| e.path().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    out.retain(|p| exts.contains(&extension(p).as_str()) && Path::new(p).is_file());
    out.sort_by_key(|p| stem(p).to_lowercase());
    out
}

fn text_of(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn workshop() -> Vec<Entry> {
    let root = expand(WPE_DIR);
    let mut out = Vec::new();
    for dir in fs::read_dir(&root).into_iter().flatten().flatten() {
        let id = dir.file_name().to_string_lossy().into_owned();
        let Ok(raw) = fs::read(format!("{root}/{id}/project.json")) else { continue };
        let Ok(project) = serde_json::from_str::<Value>(&String::from_utf8_lossy(&raw)) else { continue };
        let kind = project.get("type").map(text_of).unwrap_or_default().to_lowercase();
        if kind != "scene" && kind != "video" {
            continue;
        }
        let title = project.get("title").map(text_of).filter(|t| !t.trim().is_empty()).unwrap_or_else(|| id.clone());
        let preview = project.get("preview").and_then(Value::as_str).unwrap_or("");
        let source = format!("{root}/{id}/{preview}");
        out.push(Entry {
            kind: "wpe",
            name: title.split_whitespace().collect::<Vec<_>>().join(" "),
            stamp: stamp(&source),
            key: id,
            source,
        });
    }
    out.sort_by_key(|e| e.name.to_lowercase());
    out
}

fn local(kind: &'static str, dir: &str, exts: &[&str]) -> Vec<Entry> {
    files(dir, exts)
        .into_iter()
        .map(|path| Entry {
            kind,
            name: stem(&path),
            stamp: stamp(&path),
            source: path.clone(),
            key: path,
        })
        .collect()
}

fn video_frame(path: &str, dir: &str) -> Option<DynamicImage> {
    let _ = fs::create_dir_all(dir);
    let mut child = Command::new("mpv")
        .arg(path)
        .args(["--no-config", "--no-terminal", "--really-quiet", "--frames=1", "--start=10%", "--no-audio"])
        .args(["--vo=image", "--vo-image-format=png", &format!("--vo-image-outdir={dir}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    wait_timeout(&mut child, Duration::from_secs(30));
    let frame = image::open(format!("{dir}/00000001.png")).ok();
    let _ = fs::remove_dir_all(dir);
    frame
}

fn hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let sat = if max == 0.0 { 0.0 } else { delta / max };
    (hue, sat, max)
}

const GROUPS: [&str; 10] = ["red", "orange", "yellow", "green", "teal", "sky", "blue", "purple", "magenta", "pink"];
const CHROMA_MIN: f32 = 0.035;
const DARK_BELOW: f32 = 0.4;
const ORDER: [&str; 12] = ["red", "orange", "yellow", "green", "teal", "sky", "blue", "purple", "magenta", "pink", "dark", "light"];

fn bucket(hue: f32) -> usize {
    match hue {
        h if !(15.0..345.0).contains(&h) => 0,
        h if h < 40.0 => 1,
        h if h < 65.0 => 2,
        h if h < 160.0 => 3,
        h if h < 195.0 => 4,
        h if h < 225.0 => 5,
        h if h < 255.0 => 6,
        h if h < 290.0 => 7,
        h if h < 320.0 => 8,
        _ => 9,
    }
}

struct Analysis {
    group: &'static str,
    colors: Vec<([u8; 3], f32)>,
}

fn bin_key(c: [u8; 3]) -> u16 {
    ((c[0] as u16 >> 4) << 8) | ((c[1] as u16 >> 4) << 4) | (c[2] as u16 >> 4)
}

fn top_colors(bins: HashMap<u16, [f64; 4]>, total: f64, keep: usize) -> Vec<([u8; 3], f32)> {
    let mut list: Vec<[f64; 4]> = bins.into_values().collect();
    list.sort_by(|a, b| b[3].total_cmp(&a[3]));
    list.into_iter()
        .take(keep)
        .map(|[r, g, b, w]| ([(r / w).round() as u8, (g / w).round() as u8, (b / w).round() as u8], (w / total * 100.0) as f32))
        .collect()
}

fn analyze(img: &DynamicImage) -> Analysis {
    let small = img.resize_exact(96, 54, FilterType::Triangle).to_rgb8();
    let total = (small.width() * small.height()) as f32;
    let mut weights = [0.0_f32; 10];
    let mut light = 0.0_f32;
    let mut bins: HashMap<u16, [f64; 4]> = HashMap::new();
    for p in small.pixels() {
        let (h, s, v) = hsv(p[0], p[1], p[2]);
        light += v;
        if s > 0.2 && v > 0.15 && s * v > 0.08 {
            weights[bucket(h)] += s;
        }
        let bin = bins.entry(bin_key(p.0)).or_insert([0.0; 4]);
        bin[0] += p[0] as f64;
        bin[1] += p[1] as f64;
        bin[2] += p[2] as f64;
        bin[3] += 1.0;
    }
    let (best, weight) = weights.iter().copied().enumerate().fold((0, 0.0), |a, b| if b.1 > a.1 { b } else { a });
    let group = if weight / total >= CHROMA_MIN {
        GROUPS[best]
    } else if light / total < DARK_BELOW {
        "dark"
    } else {
        "light"
    };
    Analysis { group, colors: top_colors(bins, total as f64, 5) }
}

#[cfg(test)]
fn analyze_path(path: &str) -> Analysis {
    match image::open(path) {
        Ok(img) => analyze(&img),
        Err(_) => Analysis { group: "dark", colors: Vec::new() },
    }
}

fn encode_colors(colors: &[([u8; 3], f32)]) -> String {
    colors.iter().map(|(c, s)| format!("{:02x}{:02x}{:02x}:{:.1}", c[0], c[1], c[2], s)).collect::<Vec<_>>().join(",")
}

fn decode_colors(text: &str) -> Vec<([u8; 3], f32)> {
    text.split(',')
        .filter_map(|part| {
            let (hex, share) = part.split_once(':')?;
            let v = u32::from_str_radix(hex, 16).ok()?;
            Some(([(v >> 16) as u8, (v >> 8) as u8, v as u8], share.parse().ok()?))
        })
        .collect()
}

fn palette_lines(scope: &str, members: &[(&str, Vec<([u8; 3], f32)>)]) -> Vec<String> {
    let mut groups: HashMap<&str, (usize, HashMap<u16, [f64; 4]>)> = HashMap::new();
    for (group, colors) in members {
        let slot = groups.entry(group).or_default();
        slot.0 += 1;
        for (c, share) in colors {
            let (_, s, _) = hsv(c[0], c[1], c[2]);
            let w = *share as f64 * if *group == "dark" || *group == "light" { 1.0 } else { 0.15 + s as f64 * 2.0 };
            let bin = slot.1.entry(bin_key([c[0] & 0xe0, c[1] & 0xe0, c[2] & 0xe0])).or_insert([0.0; 4]);
            bin[0] += c[0] as f64 * w;
            bin[1] += c[1] as f64 * w;
            bin[2] += c[2] as f64 * w;
            bin[3] += w;
        }
    }
    let mut list: Vec<(&str, usize, HashMap<u16, [f64; 4]>)> = groups.into_iter().map(|(g, (n, b))| (g, n, b)).collect();
    list.sort_by_key(|(g, n, _)| (std::cmp::Reverse(*n), ORDER.iter().position(|o| o == g).unwrap_or(99)));
    list.into_iter()
        .map(|(g, n, bins)| {
            let mut top = top_colors(bins, 1.0, 5);
            top.sort_by_key(|(c, _)| c[0] as u32 * 299 + c[1] as u32 * 587 + c[2] as u32 * 114);
            let hexes: Vec<String> = top.iter().map(|(c, _)| format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2])).collect();
            format!("palette\t{scope}\t{g}\t{n}\t{}", hexes.join(","))
        })
        .collect()
}

struct CacheRow {
    thumb: String,
    colors: String,
    stamp: u64,
    group: String,
    meta: String,
}

fn parse_cache_line(line: &str) -> Option<(String, CacheRow)> {
    let f: Vec<&str> = line.split('\t').collect();
    let [kind, key, stamp, thumb, group, colors, meta] = f[..] else { return None };
    if !ORDER.contains(&group) {
        return None;
    }
    let row = CacheRow { stamp: stamp.parse().ok()?, thumb: thumb.into(), group: group.into(), colors: colors.into(), meta: meta.into() };
    Some((format!("{kind}\t{key}"), row))
}

fn human_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1}M", bytes as f64 / 1048576.0)
    } else {
        format!("{}K", (bytes / 1024).max(1))
    }
}

fn process(entry: &Entry, thumbs: &str, worker: usize) -> Option<CacheRow> {
    let img = if entry.kind == "live" && extension(&entry.source) != "gif" {
        video_frame(&entry.source, &format!("{thumbs}/.frame-{worker}"))?
    } else {
        image::open(&entry.source).ok()?
    };
    let size = fs::metadata(&entry.source).map(|m| m.len()).unwrap_or(0);
    let meta = format!("{}x{} · {} · {}", img.width(), img.height(), extension(&entry.source), human_size(size));
    let thumb = img.resize(THUMB_WIDTH, THUMB_WIDTH, FilterType::Triangle);
    let name = format!("{thumbs}/{}.jpg", sha1_smol::Sha1::from(format!("{}:{}", entry.kind, entry.key)).digest().to_string());
    let tmp = format!("{name}.part");
    thumb.to_rgb8().save_with_format(&tmp, image::ImageFormat::Jpeg).ok()?;
    fs::rename(&tmp, &name).ok()?;
    let a = analyze(&thumb);
    Some(CacheRow { stamp: entry.stamp, thumb: name, group: a.group.into(), colors: encode_colors(&a.colors), meta })
}

fn index(kinds: &[&str]) {
    let cache = expand(CACHE);
    let thumbs = format!("{cache}/thumbs");
    let _ = fs::create_dir_all(&thumbs);
    let index_file = format!("{cache}/index.tsv");
    let mut known: HashMap<String, CacheRow> = fs::read_to_string(&index_file).unwrap_or_default().lines().filter_map(parse_cache_line).collect();
    let mut entries = Vec::new();
    for kind in kinds {
        match *kind {
            "static" => entries.extend(local("static", STATIC_DIR, &IMAGE_EXT)),
            "live" => entries.extend(local("live", LIVE_DIR, &VIDEO_EXT)),
            "wpe" => entries.extend(workshop()),
            _ => {}
        }
    }
    let mut todo = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        match known.get(&format!("{}\t{}", e.kind, e.key)) {
            Some(row) if row.stamp == e.stamp && Path::new(&row.thumb).exists() => {}
            _ => todo.push(i),
        }
    }
    let results: Vec<Mutex<Option<CacheRow>>> = todo.iter().map(|_| Mutex::new(None)).collect();
    let next = AtomicUsize::new(0);
    let workers = thread::available_parallelism().map_or(4, |n| n.get().min(8));
    thread::scope(|scope| {
        for worker in 0..workers {
            let (entries, todo, results, next, thumbs) = (&entries, &todo, &results, &next, &thumbs);
            scope.spawn(move || loop {
                let n = next.fetch_add(1, Ordering::SeqCst);
                let Some(&i) = todo.get(n) else { break };
                *results[n].lock().unwrap() = process(&entries[i], thumbs, worker);
            });
        }
    });
    for (n, &i) in todo.iter().enumerate() {
        if let Some(row) = results[n].lock().unwrap().take() {
            known.insert(format!("{}\t{}", entries[i].kind, entries[i].key), row);
        }
    }
    let mut saved: Vec<String> = known
        .iter()
        .map(|(k, r)| [k.as_str(), &r.stamp.to_string(), &r.thumb, &r.group, &r.colors, &r.meta].join("\t"))
        .collect();
    saved.sort();
    let _ = fs::write(&index_file, saved.join("\n") + "\n");
    let mut out = String::new();
    let mut members: Vec<(&str, &str, Vec<([u8; 3], f32)>)> = Vec::new();
    for e in &entries {
        let row = known.get(&format!("{}\t{}", e.kind, e.key));
        let (thumb, group, colors, meta) = match row {
            Some(r) => (r.thumb.as_str(), ORDER.iter().copied().find(|g| *g == r.group).unwrap_or("dark"), r.colors.as_str(), r.meta.as_str()),
            None => ("", "dark", "", ""),
        };
        members.push((e.kind, group, decode_colors(colors)));
        let fields = [e.kind, &e.key, thumb, &e.name, group, colors, meta].map(|f| f.replace(['\t', '\n'], " "));
        out += "entry\t";
        out += &fields.join("\t");
        out.push('\n');
    }
    let mut scopes: Vec<&str> = kinds.to_vec();
    scopes.push("all");
    for scope in scopes {
        let picked: Vec<(&str, Vec<([u8; 3], f32)>)> =
            members.iter().filter(|(k, _, _)| scope == "all" || *k == scope).map(|(_, g, c)| (*g, c.clone())).collect();
        for line in palette_lines(scope, &picked) {
            out += &line;
            out.push('\n');
        }
    }
    print!("{out}");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("index") => {
            let kinds: Vec<&str> = if args.len() > 1 { args[1..].iter().map(String::as_str).collect() } else { vec!["static", "live", "wpe"] };
            index(&kinds);
        }
        _ => {
            eprintln!("usage: walls index [static] [live] [wpe]");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn solid(w: u32, h: u32, parts: &[([u8; 3], u32)]) -> DynamicImage {
        let mut img = RgbImage::new(w, h);
        let mut x0 = 0;
        for (c, cols) in parts {
            for x in x0..x0 + cols {
                for y in 0..h {
                    img.put_pixel(x, y, Rgb(*c));
                }
            }
            x0 += cols;
        }
        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn sky_is_not_blue() {
        assert_eq!(analyze(&solid(64, 36, &[([0x3d, 0x66, 0x99], 64)])).group, "sky");
        assert_eq!(analyze(&solid(64, 36, &[([0x50, 0x50, 0xb0], 64)])).group, "blue");
    }

    #[test]
    fn grey_splits_dark_and_light() {
        assert_eq!(analyze(&solid(64, 36, &[([0x16, 0x16, 0x16], 64)])).group, "dark");
        assert_eq!(analyze(&solid(64, 36, &[([0xe6, 0xe5, 0xe6], 64)])).group, "light");
    }

    #[test]
    fn colors_have_shares_largest_first() {
        let a = analyze(&solid(64, 36, &[([0x01, 0x01, 0x02], 48), ([0x0b, 0x9a, 0xf4], 16)]));
        assert!(a.colors[0].1 > a.colors[1].1);
        assert!((a.colors.iter().map(|c| c.1).sum::<f32>() - 100.0).abs() < 0.5);
        assert!(a.colors.len() <= 5);
    }

    #[test]
    fn palette_skips_empty_groups() {
        let lines = palette_lines("static", &[("sky", vec![([0x2d, 0x44, 0x5f], 60.0), ([0x10, 0x17, 0x20], 40.0)])]);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("palette\tstatic\tsky\t1\t"));
    }

    #[test]
    fn old_cache_line_is_rescanned() {
        assert!(parse_cache_line("static\t/a.jpg\t1\t/t.jpg\tblue,red").is_none());
    }

    #[test]
    fn index_survives_bad_file() {
        let a = analyze_path("/nonexistent/broken.jpg");
        assert_eq!(a.group, "dark");
        assert!(a.colors.is_empty());
    }
}
