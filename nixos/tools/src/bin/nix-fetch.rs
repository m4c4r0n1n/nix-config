use regex::{bytes, Regex};
use std::collections::HashMap;
use std::ffi::CString;
use std::io::Write;
use std::os::fd::FromRawFd;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime};
use std::{env, fs, thread};
use tools::home;

const LOGO_COLUMNS: usize = 46;
const FPS: f64 = 60.0;
const STEP: f64 = 0.8;
const RAMP: &[u8] = b" .:-=+*#%@";

type Rgb = (u8, u8, u8);

fn frames_file() -> String {
    home() + "/.config/fastfetch/nix-spin.bin"
}

fn palette_file() -> String {
    home() + "/.config/ghostty/themes/noctalia-extra"
}

fn palette() -> (Rgb, Rgb) {
    let mut found = HashMap::new();
    if let Ok(text) = fs::read_to_string(palette_file()) {
        let line = Regex::new(r"palette = (\d+)=#([0-9a-fA-F]{6})").unwrap();
        for c in line.captures_iter(&text) {
            let v = |i: usize| u8::from_str_radix(&c[2][i..i + 2], 16).unwrap();
            if let Ok(slot) = c[1].parse::<u32>() {
                found.insert(slot, (v(0), v(2), v(4)));
            }
        }
    }
    (*found.get(&1).unwrap_or(&(126, 186, 228)), *found.get(&5).unwrap_or(&(82, 119, 195)))
}

fn shades(base: Rgb) -> Vec<Option<Rgb>> {
    let scale = |c: u8, s: usize| (c as f64 * (0.3 + 0.7 * s as f64 / 15.0)) as u8;
    std::iter::once(None).chain((1..16).map(|s| Some((scale(base.0, s), scale(base.1, s), scale(base.2, s))))).collect()
}

struct Flake {
    count: usize,
    width: usize,
    height: usize,
    rows: usize,
    data: Vec<u8>,
    table: Vec<Option<Rgb>>,
    cache: HashMap<usize, Vec<String>>,
}

impl Flake {
    fn load() -> Option<Flake> {
        let raw = fs::read(frames_file()).ok()?;
        if raw.len() < 4 {
            return None;
        }
        let mut flake = Flake {
            count: raw[0] as usize + raw[3] as usize * 256,
            width: raw[1] as usize,
            height: raw[2] as usize,
            rows: raw[2] as usize / 2,
            data: raw[4..].to_vec(),
            table: Vec::new(),
            cache: HashMap::new(),
        };
        flake.colors();
        Some(flake)
    }

    fn colors(&mut self) {
        let (first, second) = palette();
        self.table = shades(first).into_iter().chain(shades(second)).collect();
        self.cache.clear();
    }

    fn lines(&mut self, index: usize) -> &Vec<String> {
        if !self.cache.contains_key(&index) {
            let width = self.width;
            let at = index * width * self.height;
            let pixel = |i: usize| self.data.get(i).copied().unwrap_or(0);
            let mut result = Vec::with_capacity(self.rows);
            for row in 0..self.rows {
                let mut line = String::new();
                for x in 0..width {
                    let value = pixel(at + row * 2 * width + x).max(pixel(at + (row * 2 + 1) * width + x)) as usize;
                    match self.table.get(value).copied().flatten() {
                        Some((r, g, b)) => {
                            let shade = value & 15;
                            let ch = RAMP[1 + shade * 8 / 15] as char;
                            line += &format!("\x1b[38;2;{r};{g};{b}m{ch}");
                        }
                        None => line.push(' '),
                    }
                }
                line += "\x1b[0m";
                result.push(line);
            }
            self.cache.insert(index, result);
        }
        &self.cache[&index]
    }
}

fn terminal_size() -> (usize, usize) {
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    let ok = unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) } == 0;
    let from_env = |var: &str| env::var(var).ok().and_then(|v| v.parse::<usize>().ok()).filter(|&n| n > 0);
    let cols = from_env("COLUMNS").or((ok && ws.ws_col > 0).then_some(ws.ws_col as usize)).unwrap_or(80);
    let lines = from_env("LINES").or((ok && ws.ws_row > 0).then_some(ws.ws_row as usize)).unwrap_or(24);
    (cols, lines)
}

fn write_all(bytes: &[u8]) -> std::io::Result<()> {
    let mut out = std::mem::ManuallyDrop::new(unsafe { fs::File::from_raw_fd(1) });
    out.write_all(bytes)
}

fn cursor_row() -> Option<usize> {
    unsafe {
        let mut saved: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(0, &mut saved) != 0 {
            return None;
        }
        let mut raw = saved;
        raw.c_lflag &= !(libc::ECHO | libc::ICANON);
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        libc::tcsetattr(0, libc::TCSANOW, &raw);
        let _ = write_all(b"\x1b[6n");
        let mut reply = Vec::new();
        let end = Instant::now() + Duration::from_millis(300);
        while Instant::now() < end && !reply.ends_with(b"R") {
            let mut fd = libc::pollfd { fd: 0, events: libc::POLLIN, revents: 0 };
            if libc::poll(&mut fd, 1, 50) > 0 {
                let mut buf = [0u8; 32];
                let n = libc::read(0, buf.as_mut_ptr().cast(), buf.len());
                if n > 0 {
                    reply.extend_from_slice(&buf[..n as usize]);
                }
            }
        }
        libc::tcsetattr(0, libc::TCSANOW, &saved);
        let found = bytes::Regex::new(r"\[(\d+);\d+R").unwrap().captures(&reply)?;
        std::str::from_utf8(&found[1]).ok()?.parse().ok()
    }
}

fn pidfile_for(shell: i32) -> String {
    format!("{}/nix-spin-{shell}", env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into()))
}

static PAUSED: AtomicBool = AtomicBool::new(false);
static PIDFILE: OnceLock<CString> = OnceLock::new();

extern "C" fn on_stop(_: libc::c_int) {
    if let Some(path) = PIDFILE.get() {
        unsafe { libc::unlink(path.as_ptr()) };
    }
    unsafe { libc::_exit(0) };
}

extern "C" fn on_pause(_: libc::c_int) {
    PAUSED.store(true, Ordering::SeqCst);
}

extern "C" fn on_resume(_: libc::c_int) {
    PAUSED.store(false, Ordering::SeqCst);
}

fn stop() -> ! {
    on_stop(0);
    unreachable!()
}

fn palette_stamp() -> Option<SystemTime> {
    fs::metadata(palette_file()).ok()?.modified().ok()
}

fn spin(mut flake: Flake, top: usize, shell: i32, pidfile: String) -> ! {
    let _ = PIDFILE.set(CString::new(pidfile).unwrap());
    let size = terminal_size();
    unsafe {
        libc::signal(libc::SIGTERM, on_stop as *const () as libc::sighandler_t);
        libc::signal(libc::SIGWINCH, on_stop as *const () as libc::sighandler_t);
        libc::signal(libc::SIGUSR1, on_pause as *const () as libc::sighandler_t);
        libc::signal(libc::SIGUSR2, on_resume as *const () as libc::sighandler_t);
    }
    let Some(mut stamp) = palette_stamp() else { stop() };
    let mut index = 0.0f64;
    loop {
        if unsafe { libc::kill(shell, 0) } != 0 {
            stop();
        }
        if !PAUSED.load(Ordering::SeqCst) {
            let mut frame = String::from("\x1b[?2026h\x1b7");
            for (row, line) in flake.lines(index as usize).iter().enumerate() {
                frame += &format!("\x1b[{};1H{line}", top + row);
            }
            frame += "\x1b8\x1b[?2026l";
            if write_all(frame.as_bytes()).is_err() {
                stop();
            }
            index = (index + STEP) % flake.count as f64;
        }
        match palette_stamp() {
            None => stop(),
            Some(now) if now != stamp => {
                stamp = now;
                flake.colors();
            }
            _ => {}
        }
        if terminal_size() != size {
            stop();
        }
        thread::sleep(Duration::from_secs_f64(1.0 / FPS));
    }
}

fn resume() {
    let pidfile = pidfile_for(unsafe { libc::getppid() });
    let Ok(text) = fs::read_to_string(&pidfile) else { return };
    let values: Vec<i64> = text.split_whitespace().filter_map(|v| v.parse().ok()).collect();
    let &[pid, top, rows] = values.as_slice() else { return };
    let row = cursor_row();
    let lines = terminal_size().1 as i64;
    let gone = match row {
        None => true,
        Some(row) => row as i64 >= lines - 1 || (row as i64) < top + rows,
    };
    unsafe { libc::kill(pid as i32, if gone { libc::SIGTERM } else { libc::SIGUSR2 }) };
}

fn main() {
    if env::args().skip(1).eq(["--resume"]) {
        return resume();
    }
    let info = Command::new("fastfetch")
        .args(["--pipe", "false", "--logo", "none"])
        .stderr(Stdio::inherit())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let info: Vec<&str> = info.trim_end_matches('\n').split('\n').collect();
    let Some(mut flake) = Flake::load() else {
        println!("{}", info.join("\n"));
        return;
    };
    let tall = flake.rows.max(info.len()) + 1;
    let mut out = "\n".repeat(tall + 3) + &format!("\x1b[{}A", tall + 3);
    for line in &info {
        out += &format!("\x1b[{}G{line}\n", LOGO_COLUMNS + 3);
    }
    out += &format!("\x1b[{}A", info.len());
    for line in flake.lines(0) {
        out += &format!("\x1b[1G{line}\n");
    }
    out += &format!("\x1b[{}B\x1b[1G", tall.saturating_sub(flake.rows + 1));
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(out.as_bytes());
    let _ = stdout.flush();
    if unsafe { libc::isatty(0) == 0 || libc::isatty(1) == 0 } {
        return;
    }
    let Some(row) = cursor_row() else { return };
    let top = (row + 1).saturating_sub(tall);
    let shell = unsafe { libc::getppid() };
    let pidfile = pidfile_for(shell);
    unsafe {
        if libc::fork() != 0 {
            return;
        }
        libc::setsid();
        if libc::fork() != 0 {
            libc::_exit(0);
        }
    }
    let _ = fs::write(&pidfile, format!("{} {} {}", std::process::id(), top, flake.rows));
    spin(flake, top, shell, pidfile);
}
