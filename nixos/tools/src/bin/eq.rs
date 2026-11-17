use std::{env, fs, process::Command, thread, time::Duration};
use tools::expand;

const FREQS: [&str; 10] = ["60", "170", "310", "600", "1k", "3k", "6k", "12k", "14k", "16k"];

const DEVICES: [(&str, &str); 2] = [("arctis", "eq_arctis"), ("arena9", "eq_arena9")];

const PRESETS: [(&str, [f64; 10]); 21] = [
    ("flat", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ("anime", [3.0, 1.5, -1.0, 0.0, 1.0, 2.5, 0.0, 1.5, 1.0, 0.5]),
    ("movies", [4.0, 2.0, -1.5, 0.0, 1.5, 3.0, 1.5, 0.0, 0.0, 0.0]),
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
    ("smooth", [1.0, 1.5, 0.5, 0.0, -1.0, -3.5, -4.5, -3.0, -3.0, -3.0]),
];

const ARENA_PRESETS: [(&str, [f64; 10]); 14] = [
    ("flat", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ("club", [6.0, 4.0, 0.0, -1.0, 0.0, 1.0, 2.0, 2.0, 1.0, 0.0]),
    ("dance", [7.0, 5.0, 1.0, -1.0, -1.0, 0.0, 2.0, 3.0, 2.0, 1.0]),
    ("fullbass", [8.0, 6.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ("techno", [7.0, 5.0, 0.0, -2.0, -1.0, 1.0, 3.0, 3.0, 2.0, 2.0]),
    ("party", [7.0, 5.0, 0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 3.0, 2.0]),
    ("rock", [5.0, 3.0, -1.0, -2.0, -1.0, 2.0, 3.0, 3.0, 2.0, 2.0]),
    ("pop", [3.0, 3.0, 1.0, 0.0, 1.0, 2.0, 2.0, 1.5, 1.0, 0.5]),
    ("movies", [2.0, 0.5, -2.0, -1.0, 0.5, 2.5, 2.0, 1.0, 0.5, 0.0]),
    ("anime", [4.0, 2.0, -1.0, 0.0, 1.0, 2.5, 1.0, 1.5, 1.0, 0.5]),
    ("gaming", [4.0, 2.0, -1.0, -1.0, 0.0, 2.5, 3.0, 1.5, 1.0, 0.0]),
    ("vocal", [-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 2.0, 0.0, 0.0, 0.0]),
    ("classical", [2.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, -1.0, -1.5, -2.0]),
    ("smooth", [1.0, 1.5, 0.5, 0.0, -1.0, -3.5, -4.5, -3.0, -3.0, -3.0]),
];

fn presets_for(device: &str) -> &'static [(&'static str, [f64; 10])] {
    if device == "arena9" { &ARENA_PRESETS } else { &PRESETS }
}

fn usage() -> ! {
    eprintln!("usage: eq status");
    eprintln!("       eq presets");
    eprintln!("       eq <arctis|arena9> <preset>");
    eprintln!("       eq <arctis|arena9> set <10 gains in dB>");
    eprintln!("       eq <arctis|arena9> save <name>");
    eprintln!("       eq <arctis|arena9> delete <name>");
    eprintln!("       eq presets <arctis|arena9>");
    eprintln!("       eq viz <arctis|arena9>");
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

const OUTPUTS: [(&str, &str); 2] = [
    ("arctis", "alsa_output.usb-SteelSeries_Arctis_Nova_Pro_Wireless-00.iec958-stereo"),
    ("arena9", "alsa_output.usb-SteelSeries_SteelSeries_Arena_9-00.analog-surround-51"),
];
const VIZ_BARS: usize = 32;
const VIZ_WINDOW: usize = 2048;
const VIZ_HOP: usize = 1024;

fn user_file(device: &str) -> String {
    expand(&format!("~/.local/state/eq/presets-{device}"))
}

fn clean_name(name: &str) -> Option<String> {
    let kept: String = name.chars().filter(|c| !c.is_control()).collect();
    let name: String = kept.trim().chars().take(32).collect();
    (!name.is_empty()).then_some(name)
}

fn parse_user(text: &str) -> Vec<(String, Vec<f64>)> {
    text.lines()
        .filter_map(|line| {
            let (name, gains) = line.split_once('\t')?;
            let gains: Vec<f64> = gains.split_whitespace().filter_map(|v| v.parse().ok()).collect();
            (gains.len() == 10).then(|| (name.to_string(), gains))
        })
        .collect()
}

fn format_user(list: &[(String, Vec<f64>)]) -> String {
    list.iter()
        .map(|(name, gains)| format!("{name}\t{}\n", gains.iter().map(|g| format!("{g:.1}")).collect::<Vec<_>>().join(" ")))
        .collect()
}

fn user_presets(device: &str) -> Vec<(String, Vec<f64>)> {
    parse_user(&fs::read_to_string(user_file(device)).unwrap_or_default())
}

fn write_user(device: &str, list: &[(String, Vec<f64>)]) {
    let path = user_file(device);
    if let Some(dir) = std::path::Path::new(&path).parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(path, format_user(list));
}

fn bar_freq(i: usize) -> f32 {
    40.0 * (16000.0_f32 / 40.0).powf(i as f32 / (VIZ_BARS - 1) as f32)
}

fn goertzel(samples: &[f32], freq: f32, rate: f32) -> f32 {
    let w = 2.0 * std::f32::consts::PI * freq / rate;
    let coeff = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0_f32, 0.0_f32);
    for &x in samples {
        let s = x + coeff * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0)
}

fn bars(samples: &[f32], rate: f32) -> Vec<f32> {
    let n = samples.len();
    let windowed: Vec<f32> = samples
        .iter()
        .enumerate()
        .map(|(i, x)| x * (0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos()))
        .collect();
    let half_step = (16000.0_f32 / 40.0).powf(0.5 / (VIZ_BARS - 1) as f32);
    (0..VIZ_BARS)
        .map(|i| {
            let center = bar_freq(i);
            let (lo, hi) = (center / half_step, center * half_step);
            let points = 4;
            let power: f32 = (0..points)
                .map(|k| goertzel(&windowed, lo * (hi / lo).powf((k as f32 + 0.5) / points as f32), rate))
                .sum::<f32>()
                / points as f32;
            let amp = power.sqrt() / (n as f32 / 4.0);
            let db = 20.0 * amp.max(1e-9).log10();
            ((db + 72.0) / 66.0).clamp(0.0, 1.0)
        })
        .collect()
}

fn viz(device: &str) {
    let target = OUTPUTS.iter().find(|(d, _)| *d == device).map(|(_, n)| *n).unwrap_or_else(|| usage());
    let run = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let (out, alive, pid_file) = (format!("{run}/eq-viz"), format!("{run}/eq-viz.alive"), format!("{run}/eq-viz.pid"));
    if let Ok(old) = fs::read_to_string(&pid_file) {
        if let Ok(pid) = old.trim().parse::<i32>() {
            if pid != std::process::id() as i32 && std::path::Path::new(&format!("/proc/{pid}")).exists() {
                unsafe { libc::kill(pid, libc::SIGTERM) };
            }
        }
    }
    let _ = fs::write(&pid_file, std::process::id().to_string());
    let _ = fs::write(&alive, "");
    let Ok(mut child) = Command::new("pw-record")
        .args(["--target", target, "-P", "{ stream.capture.sink = true node.name = eq-viz }", "--channels", "2", "--format", "f32", "--rate", "48000", "-"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return;
    };
    let Some(mut pipe) = child.stdout.take() else { return };
    let mut window = vec![0.0_f32; VIZ_WINDOW];
    let mut shown = vec![0.0_f32; VIZ_BARS];
    let mut raw = vec![0u8; VIZ_HOP * 2 * 4];
    loop {
        if std::io::Read::read_exact(&mut pipe, &mut raw).is_err() {
            break;
        }
        let fresh = fs::metadata(&alive).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|e| e < Duration::from_secs(3));
        let mine = fs::read_to_string(&pid_file).map(|p| p.trim() == std::process::id().to_string()).unwrap_or(false);
        if !fresh || !mine {
            break;
        }
        window.drain(..VIZ_HOP);
        for frame in raw.chunks_exact(8) {
            let l = f32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]);
            let r = f32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]]);
            window.push((l + r) * 0.5);
        }
        for (s, v) in shown.iter_mut().zip(bars(&window, 48000.0)) {
            *s = if v > *s { *s * 0.4 + v * 0.6 } else { *s * 0.82 + v * 0.18 };
        }
        let text: Vec<String> = shown.iter().map(|v| format!("{v:.2}")).collect();
        let tmp = format!("{out}.tmp");
        if fs::write(&tmp, text.join(" ")).is_ok() {
            let _ = fs::rename(&tmp, &out);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    if fs::read_to_string(&pid_file).map(|p| p.trim() == std::process::id().to_string()).unwrap_or(false) {
        let _ = fs::remove_file(&pid_file);
        let _ = fs::remove_file(&out);
    }
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
        ["presets", device] => {
            for (name, _) in presets_for(device) {
                println!("builtin\t{name}");
            }
            for (name, _) in user_presets(device) {
                println!("user\t{name}");
            }
        }
        ["viz", device] => viz(device),
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
        [device, "save", name @ ..] => {
            let node = node_for(device);
            let Some(name) = clean_name(&name.join(" ")) else {
                eprintln!("eq: give the preset a name");
                std::process::exit(2);
            };
            if presets_for(device).iter().any(|(n, _)| *n == name.to_lowercase()) {
                eprintln!("eq: {name} is a built-in preset, pick another name");
                std::process::exit(2);
            }
            let Some(gains) = read_gains(node).or_else(|| load(device).map(|(_, g)| g)) else {
                eprintln!("eq: {device} EQ is not running");
                std::process::exit(1);
            };
            let mut list = user_presets(device);
            list.retain(|(n, _)| *n != name);
            list.push((name.clone(), gains.clone()));
            write_user(device, &list);
            save(device, &name, &gains);
            println!("{device}: saved {name}");
        }
        [device, "delete", name @ ..] => {
            node_for(device);
            let name = name.join(" ");
            let mut list = user_presets(device);
            let before = list.len();
            list.retain(|(n, _)| *n != name);
            if list.len() == before {
                eprintln!("eq: no saved preset named {name}");
                std::process::exit(2);
            }
            write_user(device, &list);
            println!("{device}: deleted {name}");
        }
        [device, preset @ ..] if !preset.is_empty() => {
            let node = node_for(device);
            let preset = preset.join(" ");
            let builtin = presets_for(device).iter().find(|(n, _)| *n == preset).map(|(n, g)| (n.to_string(), g.to_vec()));
            let Some((name, gains)) = builtin.or_else(|| user_presets(device).into_iter().find(|(n, _)| *n == preset)) else {
                eprintln!("eq: unknown preset {preset}, see: eq presets {device}");
                std::process::exit(2);
            };
            let (name, gains) = (name.as_str(), gains.as_slice());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  YouTube\tfix\n "), Some("YouTubefix".to_string()));
        assert_eq!(clean_name("   "), None);
        assert_eq!(clean_name(&"x".repeat(50)).map(|n| n.len()), Some(32));
    }

    #[test]
    fn user_presets_round_trip() {
        let list = vec![("Late night".to_string(), vec![1.0, -2.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0])];
        assert_eq!(parse_user(&format_user(&list)), list);
        assert!(parse_user("bad line\nname\t1 2 3\n").is_empty());
    }

    #[test]
    fn sine_lights_its_own_bar() {
        let rate = 48000.0;
        let samples: Vec<f32> = (0..2048).map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / rate).sin() * 0.5).collect();
        let b = bars(&samples, rate);
        assert_eq!(b.len(), VIZ_BARS);
        let top = b.iter().enumerate().fold((0, 0.0), |a, (i, v)| if *v > a.1 { (i, *v) } else { a }).0;
        assert!((bar_freq(top) / 1000.0).log2().abs() < 0.35, "top bar {top} at {} Hz", bar_freq(top));
    }

    #[test]
    fn smooth_preset_cuts_harsh_band() {
        let (_, g) = ARENA_PRESETS.iter().find(|(n, _)| *n == "smooth").unwrap();
        assert!(g[5] < -2.0 && g[6] < -2.0);
    }
}
