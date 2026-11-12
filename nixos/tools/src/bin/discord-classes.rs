use regex::{bytes, Regex};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;
use std::time::{Duration, UNIX_EPOCH};
use std::{env, fs, thread};
use tools::home;

const START_WAIT: u64 = 120;
const BUILD_WINDOW: f64 = 600.0;
const MAIN_SHEET: u64 = 50_000;
const MIN_MODULES: usize = 500;
const MIN_MATCH: f64 = 0.5;

static ASSET: LazyLock<bytes::Regex> =
    LazyLock::new(|| bytes::Regex::new(r"https://discord\.com/assets/[\w.\-]+\.(css|js)").unwrap());
static CSS_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.([A-Za-z][A-Za-z0-9_-]*)").unwrap());
static CSS_SHAPE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z][A-Za-z0-9-]*_{1,2}[A-Za-z0-9_-]{5,6}$").unwrap());
static JS_CLASS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"["']?[A-Za-z]\w*["']?:["']([A-Za-z][A-Za-z0-9-]*_{1,2}[A-Za-z0-9_-]{5,6})(?: [^"']*)?["']"#).unwrap()
});
static HASH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^_[0-9a-f]{5}$").unwrap());

type Modules = BTreeMap<String, BTreeSet<String>>;

fn setting(var: &str, default: &str) -> String {
    env::var(var).unwrap_or_else(|_| home() + default)
}

fn css_classes(text: &str) -> Vec<String> {
    CSS_RUN.captures_iter(text).map(|c| c[1].to_string()).filter(|n| CSS_SHAPE.is_match(n)).collect()
}

fn unpack(data: &[u8]) -> Vec<u8> {
    let mut reader = brotli_decompressor::Decompressor::new(data, 4096);
    let (mut out, mut buf) = (Vec::new(), [0u8; 4096]);
    while let Ok(n) = reader.read(&mut buf) {
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
    }
    if out.is_empty() { data.to_vec() } else { out }
}

fn split_class(name: &str) -> (String, String) {
    let cut = name.len() - 6;
    let stem = name[..cut].trim_end_matches('_').to_string();
    let hash = &name[cut..];
    let hash = if HASH.is_match(hash) { &hash[1..] } else { hash };
    (stem, hash.to_string())
}

fn mtime(meta: &fs::Metadata) -> f64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0.0, |d| d.as_secs_f64())
}

fn current_build() -> Modules {
    let cache = setting("DISCORD_CACHE", "/.config/vesktop/sessionData/Cache/Cache_Data");
    let mut entries = Vec::new();
    for entry in fs::read_dir(&cache).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || !name.ends_with("_0") {
            continue;
        }
        let Ok(mut file) = fs::File::open(entry.path()) else { continue };
        let mut head = Vec::with_capacity(600);
        let _ = (&mut file).take(600).read_to_end(&mut head);
        let Some(found) = ASSET.captures(&head) else { continue };
        let Ok(meta) = entry.metadata() else { continue };
        entries.push((mtime(&meta), meta.len(), entry.path(), found[0].to_vec(), &found[1] == b"css"));
    }
    let mut modules = Modules::new();
    let sheets: Vec<_> = entries.iter().filter(|e| e.4 && e.1 > MAIN_SHEET).collect();
    let pool = if sheets.is_empty() { entries.iter().collect() } else { sheets };
    let Some(newest) = pool.iter().map(|e| e.0).reduce(f64::max) else { return modules };
    for (changed, _, path, url, css) in &entries {
        if *changed < newest - BUILD_WINDOW {
            continue;
        }
        let Ok(data) = fs::read(path) else { continue };
        let Some(at) = data.windows(url.len()).position(|w| w == &url[..]) else { continue };
        let body = String::from_utf8_lossy(&unpack(&data[at + url.len()..])).replace('\u{FFFD}', "");
        let names: Vec<String> = if *css {
            css_classes(&body)
        } else {
            JS_CLASS.captures_iter(&body).map(|c| c[1].to_string()).collect()
        };
        for name in names {
            let (stem, hash) = split_class(&name);
            modules.entry(hash).or_default().insert(stem);
        }
    }
    modules
}

fn template_classes(template: &str) -> Vec<String> {
    let text = fs::read_to_string(template).unwrap_or_else(|e| {
        eprintln!("discord-classes: {template}: {e}");
        std::process::exit(1);
    });
    let set: BTreeSet<String> = css_classes(&text).into_iter().collect();
    set.into_iter().collect()
}

fn save_snapshot(modules: &Modules, template: &str, snapshot: &str) {
    let used: BTreeSet<String> = template_classes(template).iter().map(|n| split_class(n).1).collect();
    let data: BTreeMap<&String, &BTreeSet<String>> = used.iter().filter_map(|k| modules.get(k).map(|v| (k, v))).collect();
    if let Some(dir) = Path::new(snapshot).parent() {
        let _ = fs::create_dir_all(dir);
    }
    let mut out = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut out, serde_json::ser::PrettyFormatter::with_indent(b" "));
    serde::Serialize::serialize(&data, &mut ser).unwrap();
    let _ = fs::write(snapshot, out);
}

fn best_module(stem: &str, old: &BTreeSet<String>, modules: &Modules) -> Option<String> {
    let (mut best, mut score) = (None, 0.0);
    for (key, stems) in modules {
        if stems.contains(stem) {
            let union = old.union(stems).count() as f64;
            let value = old.intersection(stems).count() as f64 / union;
            if value > score {
                best = Some(key.clone());
                score = value;
            }
        }
    }
    if score >= MIN_MATCH { best } else { None }
}

fn rewrite(path: &str, renames: &HashMap<String, String>) {
    let Ok(text) = fs::read_to_string(path) else { return };
    let text = CSS_RUN.replace_all(&text, |c: &regex::Captures| {
        let name = &c[1];
        match renames.get(name) {
            Some(new) if CSS_SHAPE.is_match(name) => format!(".{new}"),
            _ => c[0].to_string(),
        }
    });
    let _ = fs::write(path, text.as_bytes());
}

fn tell(message: &str) {
    println!("{message}");
    let _ = Command::new("notify-send").args(["Discord theme", message]).status();
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let template = setting("DISCORD_TEMPLATE", "/.config/noctalia/templates/discord-wall.css");
    let theme = setting("DISCORD_THEME", "/.config/vesktop/themes/Discord-Wall.theme.css");
    let snapshot = setting("DISCORD_SNAPSHOT", "/.local/state/discord-classes/modules.json");
    if args.iter().any(|a| a == "--wait") {
        thread::sleep(Duration::from_secs(START_WAIT));
    }
    let modules = current_build();
    if modules.len() < MIN_MODULES {
        return println!("Not enough Discord files in the cache yet.");
    }
    if args.iter().any(|a| a == "--snapshot") || !Path::new(&snapshot).exists() {
        save_snapshot(&modules, &template, &snapshot);
        return println!("Saved class snapshot.");
    }
    let old: BTreeMap<String, BTreeSet<String>> = fs::read_to_string(&snapshot)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let empty = BTreeSet::new();
    let (mut renames, mut failed) = (HashMap::new(), Vec::new());
    for name in template_classes(&template) {
        let (stem, hash) = split_class(&name);
        if modules.get(&hash).is_some_and(|s| s.contains(&stem)) {
            continue;
        }
        match best_module(&stem, old.get(&hash).unwrap_or(&empty), &modules) {
            Some(key) => {
                renames.insert(name, format!("{stem}_{key}"));
            }
            None => failed.push(name),
        }
    }
    if !renames.is_empty() {
        rewrite(&template, &renames);
        rewrite(&theme, &renames);
        save_snapshot(&modules, &template, &snapshot);
    }
    if !renames.is_empty() || !failed.is_empty() {
        tell(&format!("Updated {} class names, {} not found.", renames.len(), failed.len()));
        for name in failed {
            println!("not found: {name}");
        }
    } else {
        println!("Theme classes are current.");
    }
}
