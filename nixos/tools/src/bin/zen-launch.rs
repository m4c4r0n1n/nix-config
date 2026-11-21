use serde_json::Value;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};
use std::{env, fs, thread};

const APP_ID: &str = "zen-twilight";
const LOCK_AGE: Duration = Duration::from_secs(30);
const MAP_WAIT: Duration = Duration::from_secs(40);
const WATCH: Duration = Duration::from_secs(10);

fn zen_windows() -> Vec<String> {
    let out = match Command::new("umbriel").args(["windows", "--json"]).output() {
        Ok(o) => o.stdout,
        Err(_) => return Vec::new(),
    };
    let list: Value = serde_json::from_slice(&out).unwrap_or(Value::Null);
    list.as_array()
        .map(|a| {
            a.iter()
                .filter(|w| w["app_id"] == APP_ID)
                .filter_map(|w| w["id"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn msg(args: &[&str]) {
    let _ = Command::new("umbriel").arg("msg").args(args).status();
}

fn focused_id() -> Option<String> {
    let out = Command::new("umbriel").args(["focused-window", "--json"]).output().ok()?;
    let w: Value = serde_json::from_slice(&out.stdout).ok()?;
    w["id"].as_str().map(String::from)
}

fn close(id: &str) {
    msg(&["window-focus", id]);
    if focused_id().as_deref() == Some(id) {
        msg(&["window-close"]);
    }
}

fn spawn(args: &[String]) {
    let mut cmd = Command::new(APP_ID);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let _ = cmd.spawn();
}

fn lock_path() -> String {
    let dir = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    format!("{dir}/zen-launch.lock")
}

fn lock_fresh(path: &str) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map(|age| age < LOCK_AGE)
        .unwrap_or(false)
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let lock = lock_path();

    if !zen_windows().is_empty() {
        if !lock_fresh(&lock) {
            spawn(&args);
        }
        return;
    }
    if lock_fresh(&lock) {
        return;
    }
    let _ = fs::write(&lock, "");
    spawn(&args);

    let start = Instant::now();
    let mut keep: Option<String> = None;
    let mut first_seen: Option<Instant> = None;
    while start.elapsed() < MAP_WAIT {
        thread::sleep(Duration::from_millis(150));
        let wins = zen_windows();
        if wins.is_empty() {
            continue;
        }
        let kept = keep.get_or_insert_with(|| wins[0].clone()).clone();
        let seen = *first_seen.get_or_insert_with(Instant::now);
        let extra: Vec<&String> = wins.iter().filter(|w| **w != kept).collect();
        if !wins.contains(&kept) {
            keep = None;
            continue;
        }
        if !extra.is_empty() {
            for id in extra {
                close(id);
            }
            msg(&["window-focus", &kept]);
        }
        if seen.elapsed() > WATCH {
            break;
        }
    }
    let _ = fs::remove_file(&lock);
}
