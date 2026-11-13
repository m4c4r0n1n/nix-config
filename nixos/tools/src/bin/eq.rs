use std::{env, fs, process::Command, thread, time::Duration};
use tools::expand;

const FREQS: [&str; 10] = ["60", "170", "310", "600", "1k", "3k", "6k", "12k", "14k", "16k"];

const DEVICES: [(&str, &str); 2] = [("arctis", "eq_arctis"), ("arena9", "eq_arena9")];

const PRESETS: [(&str, [f64; 10]); 18] = [
    ("flat", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ("classical", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -7.2, -7.2, -7.2, -9.6]),
    ("club", [0.0, 0.0, 8.0, 5.6, 5.6, 5.6, 3.2, 0.0, 0.0, 0.0]),
    ("dance", [9.6, 7.2, 2.4, 0.0, 0.0, -5.6, -7.2, -7.2, 0.0, 0.0]),
    ("fullbass", [-8.0, 9.6, 9.6, 5.6, 1.6, -4.0, -8.0, -10.4, -11.2, -11.2]),
    ("fullbasstreble", [7.2, 5.6, 0.0, -7.2, -4.8, 1.6, 8.0, 11.2, 12.0, 12.0]),
    ("fulltreble", [-9.6, -9.6, -9.6, -4.0, 2.4, 11.2, 16.0, 16.0, 16.0, 16.8]),
    ("headphones", [4.8, 11.2, 5.6, -3.2, -2.4, 1.6, 4.8, 9.6, 12.8, 14.4]),
    ("largehall", [10.4, 10.4, 5.6, 5.6, 0.0, -4.8, -4.8, -4.8, 0.0, 0.0]),
    ("live", [-4.8, 0.0, 4.0, 5.6, 5.6, 5.6, 4.0, 2.4, 2.4, 2.4]),
    ("party", [7.2, 7.2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 7.2, 7.2]),
    ("pop", [-1.6, 4.8, 7.2, 8.0, 5.6, 0.0, -2.4, -2.4, -1.6, -1.6]),
    ("reggae", [0.0, 0.0, 0.0, -5.6, 0.0, 6.4, 6.4, 0.0, 0.0, 0.0]),
    ("rock", [8.0, 4.8, -5.6, -8.0, -3.2, 4.0, 8.8, 11.2, 11.2, 11.2]),
    ("ska", [-2.4, -4.8, -4.0, 0.0, 4.0, 5.6, 8.8, 9.6, 11.2, 9.6]),
    ("soft", [4.8, 1.6, 0.0, -2.4, 0.0, 4.0, 8.0, 9.6, 11.2, 12.0]),
    ("softrock", [4.0, 4.0, 2.4, 0.0, -4.0, -5.6, -3.2, 0.0, 2.4, 8.8]),
    ("techno", [8.0, 5.6, 0.0, -5.6, -4.8, 0.0, 8.0, 9.6, 9.6, 8.8]),
];

fn usage() -> ! {
    eprintln!("usage: eq status");
    eprintln!("       eq presets");
    eprintln!("       eq <arctis|arena9> <preset>");
    eprintln!("       eq <arctis|arena9> set <10 gains in dB>");
    eprintln!("       eq restore");
    std::process::exit(2);
}

fn state_file(device: &str) -> String {
    expand(&format!("~/.local/state/eq/{device}"))
}

fn node_id(node: &str) -> Option<u64> {
    let out = Command::new("pw-dump").output().ok()?;
    let data: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    data.as_array()?.iter().find_map(|o| {
        let name = o.pointer("/info/props/node.name")?.as_str()?;
        if name == node {
            o.get("id")?.as_u64()
        } else {
            None
        }
    })
}

fn read_gains(node: &str) -> Option<Vec<f64>> {
    let id = node_id(node)?;
    let out = Command::new("pw-dump").arg(id.to_string()).output().ok()?;
    let data: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let mut gains = vec![0.0; 10];
    for object in data.as_array()? {
        let Some(props) = object.pointer("/info/params/Props").and_then(|p| p.as_array()) else {
            continue;
        };
        for entry in props {
            let Some(params) = entry.get("params").and_then(|p| p.as_array()) else {
                continue;
            };
            for pair in params.chunks(2) {
                if let [key, value] = pair {
                    let key = key.as_str().unwrap_or("");
                    if let Some(band) = key.strip_prefix('b').and_then(|k| k.strip_suffix(":Gain")) {
                        if let (Ok(i), Some(v)) = (band.parse::<usize>(), value.as_f64()) {
                            if i < 10 {
                                gains[i] = (v * 10.0).round() / 10.0;
                            }
                        }
                    }
                }
            }
        }
    }
    Some(gains)
}

fn apply(node: &str, gains: &[f64]) -> bool {
    let Some(id) = node_id(node) else {
        return false;
    };
    let peak = gains.iter().cloned().fold(0.0_f64, f64::max);
    let mut params = format!("\"pre:Gain\" {:.1}", -peak);
    for (i, g) in gains.iter().enumerate() {
        params += &format!(" \"b{i}:Gain\" {g:.1}");
    }
    Command::new("pw-cli")
        .args(["set-param", &id.to_string(), "Props", &format!("{{ params = [ {params} ] }}")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn save(device: &str, label: &str, gains: &[f64]) {
    let path = state_file(device);
    if let Some(dir) = std::path::Path::new(&path).parent() {
        let _ = fs::create_dir_all(dir);
    }
    let text: Vec<String> = gains.iter().map(|g| format!("{g:.1}")).collect();
    let _ = fs::write(path, format!("{label}\n{}\n", text.join(" ")));
}

fn load(device: &str) -> Option<(String, Vec<f64>)> {
    let text = fs::read_to_string(state_file(device)).ok()?;
    let mut lines = text.lines();
    let label = lines.next()?.to_string();
    let gains: Vec<f64> = lines.next()?.split_whitespace().filter_map(|v| v.parse().ok()).collect();
    (gains.len() == 10).then_some((label, gains))
}

fn node_for(device: &str) -> &'static str {
    DEVICES.iter().find(|(d, _)| *d == device).map(|(_, n)| *n).unwrap_or_else(|| usage())
}

fn show(gains: &[f64]) -> String {
    FREQS.iter().zip(gains).map(|(f, g)| format!("{f}:{g:+.1}")).collect::<Vec<_>>().join(" ")
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["presets"] => {
            for (name, _) in PRESETS {
                println!("{name}");
            }
        }
        ["status"] => {
            for (device, node) in DEVICES {
                let label = load(device).map(|(l, _)| l).unwrap_or_else(|| "flat".into());
                match read_gains(node) {
                    Some(g) => println!("{device} ({label}): {}", show(&g)),
                    None => println!("{device}: EQ not running"),
                }
            }
        }
        ["restore"] => {
            for (device, node) in DEVICES {
                let Some((_, gains)) = load(device) else {
                    continue;
                };
                for _ in 0..60 {
                    if apply(node, &gains) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(500));
                }
            }
        }
        [device, "set", gains @ ..] => {
            let node = node_for(device);
            let gains: Vec<f64> = gains.iter().filter_map(|g| g.parse().ok()).collect();
            if gains.len() != 10 || gains.iter().any(|g| g.abs() > 24.0) {
                eprintln!("eq: give 10 gains between -24 and 24 dB");
                std::process::exit(2);
            }
            if !apply(node, &gains) {
                eprintln!("eq: {device} EQ is not running");
                std::process::exit(1);
            }
            save(device, "custom", &gains);
            println!("{device}: {}", show(&gains));
        }
        [device, preset] => {
            let node = node_for(device);
            let Some((name, gains)) = PRESETS.iter().find(|(n, _)| n == preset) else {
                eprintln!("eq: unknown preset {preset}, see: eq presets");
                std::process::exit(2);
            };
            if !apply(node, gains) {
                eprintln!("eq: {device} EQ is not running");
                std::process::exit(1);
            }
            save(device, name, gains);
            println!("{device} ({name}): {}", show(gains));
        }
        _ => usage(),
    }
}
