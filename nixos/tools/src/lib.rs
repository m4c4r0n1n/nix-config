use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};
use std::{env, fs, thread};

pub fn home() -> String {
    env::var("HOME").unwrap_or_default()
}

pub fn expand(path: &str) -> String {
    match path.strip_prefix("~/") {
        Some(rest) => format!("{}/{}", home(), rest),
        None => path.to_string(),
    }
}

pub fn uid() -> u32 {
    unsafe { libc::getuid() }
}

pub fn current_wallpaper() -> Option<String> {
    let text = fs::read_to_string(home() + "/.local/state/noctalia/settings.toml").ok()?;
    let data: toml::Table = text.parse().ok()?;
    let wall = data.get("wallpaper")?.as_table()?;
    let mut entries: Vec<&toml::Value> = Vec::new();
    if let Some(monitors) = wall.get("monitors").and_then(|v| v.as_table()) {
        entries.extend(monitors.values());
    }
    entries.extend(["last", "default"].iter().filter_map(|k| wall.get(*k)));
    entries
        .into_iter()
        .filter_map(|e| e.as_table()?.get("path")?.as_str())
        .find(|p| !p.is_empty())
        .map(String::from)
}

pub fn which(name: &str) -> Option<String> {
    env::var("PATH").ok()?.split(':').map(|dir| format!("{dir}/{name}")).find(|p| {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(p).map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
    })
}

pub fn wait_timeout(child: &mut Child, limit: Duration) -> Option<ExitStatus> {
    let end = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if Instant::now() < end => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

pub fn capture(cmd: &mut Command, limit: Duration) -> Option<String> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
    let mut out = child.stdout.take()?;
    let mut err = child.stderr.take()?;
    let reader = thread::spawn(move || {
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut out, &mut buf).ok();
        buf
    });
    thread::spawn(move || std::io::copy(&mut err, &mut std::io::sink()));
    wait_timeout(&mut child, limit)?;
    Some(String::from_utf8_lossy(&reader.join().ok()?).into_owned())
}

pub fn run_quiet(program: &str, args: &[&str]) {
    let _ = Command::new(program).args(args).stdout(Stdio::null()).stderr(Stdio::null()).status();
}
