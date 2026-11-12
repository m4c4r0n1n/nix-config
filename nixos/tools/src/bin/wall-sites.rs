use regex::Regex;
use serde_json::{json, Value};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::LazyLock;
use std::time::{Duration, SystemTime};
use std::{env, fs, thread};
use tools::{capture, expand, run_quiet, uid, wait_timeout, which};

type Error = Box<dyn std::error::Error>;
type Item = (String, String, String, String, String);
type Listing = Result<(i64, Vec<Item>), Error>;

const AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0";
const LIVEBGS_PAGE: i64 = 24;
const WPE_PAGE: usize = 24;

static WPE_ITEMS: LazyLock<String> = LazyLock::new(|| expand("~/.local/share/Steam/steamapps/workshop/content/431960"));
static WPE_ASSETS: LazyLock<String> = LazyLock::new(|| expand("~/.local/share/Steam/steamapps/common/wallpaper_engine/assets"));
static THUMBS: LazyLock<String> = LazyLock::new(|| expand("~/.local/state/noctalia/wallhaven/thumbs"));
static STEAM_HOME: LazyLock<String> = LazyLock::new(|| expand("~/.local/share/wall-sites-steam"));
static STEAM_USER: LazyLock<String> = LazyLock::new(|| expand("~/.config/wall-sites/steam-user"));
static WPE_LAST: LazyLock<String> = LazyLock::new(|| expand("~/.local/state/noctalia/wpe-last"));
static WPE_HOLD: LazyLock<String> = LazyLock::new(|| format!("/run/user/{}/wpe-hold", uid()));
const WPE_PATTERN: &str = "^(.*/)?[.]?linux-wallpaperengine --screen-root";

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap()
}

fn get(url: &str) -> Result<String, Error> {
    let response = ureq::get(url).set("User-Agent", AGENT).timeout(Duration::from_secs(20)).call()?;
    let mut body = Vec::new();
    std::io::Read::read_to_end(&mut response.into_reader(), &mut body)?;
    Ok(String::from_utf8_lossy(&body).into_owned())
}

fn quote(text: &str, safe: &str, plus: bool) -> String {
    let mut out = String::new();
    for &b in text.as_bytes() {
        match b {
            b' ' if plus => out.push('+'),
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => out.push(b as char),
            _ if safe.as_bytes().contains(&b) => out.push(b as char),
            _ => out += &format!("%{b:02X}"),
        }
    }
    out
}

fn tidy(title: &str) -> String {
    let title = html_escape::decode_html_entities(title);
    let title = re(r"(?i)\s*live wallpapers?.*$").replace_all(&title, "");
    title.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn item(id: String, title: String, thumb: String, url: String, ext: &str) -> Item {
    (id, title, thumb, url, ext.to_string())
}

fn motionbgs(query: &str, page: i64) -> Listing {
    let url = if query.is_empty() {
        format!("https://motionbgs.com/tag:anime/{page}/")
    } else {
        format!("https://motionbgs.com/search?q={}&page={page}", quote(query, "", true))
    };
    Ok(motionbgs_parse(&get(&url)?, page))
}

fn motionbgs_parse(body: &str, page: i64) -> (i64, Vec<Item>) {
    let card = re(r#"(?s)<a title="([^"]*)" href=/[\w-]+> <figure>(.*?)</figure>"#);
    let image = re(r"/i/c/546x308/media/(\d+)/([^\s>]+?\.jpg)\s");
    let mut items = Vec::new();
    for found in card.captures_iter(body) {
        if let Some(img) = image.captures(&found[2]) {
            let (media, name) = (&img[1], &img[2]);
            items.push(item(
                format!("motionbgs-{media}"),
                tidy(&found[1]),
                format!("https://motionbgs.com/i/c/546x308/media/{media}/{name}"),
                format!("https://motionbgs.com/dl/4k/{media}/"),
                ".mp4",
            ));
        }
    }
    let fallback = if items.len() >= 36 { page + 1 } else { page };
    let pages = re(r"/tag:[\w-]+/(\d+)/").captures_iter(body).filter_map(|c| c[1].parse().ok()).chain([fallback]).max();
    (pages.unwrap(), items)
}

fn text_of(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn truthy(value: Option<&Value>) -> Option<&Value> {
    value.filter(|v| match v {
        Value::Null | Value::Bool(false) => false,
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        _ => true,
    })
}

fn livebgs(query: &str, page: i64) -> Listing {
    let mut params = json!({"limit": LIVEBGS_PAGE, "offset": (page - 1) * LIVEBGS_PAGE});
    let method = if query.is_empty() {
        "getLatest"
    } else {
        params["query"] = json!(query);
        "search"
    };
    let text = json!({"0": {"json": params}}).to_string();
    let url = format!("https://livebgs.com/api/trpc/wallpapers.{method}?batch=1&input={}", quote(&text, "/", false));
    livebgs_parse(&get(&url)?, page)
}

fn livebgs_parse(body: &str, page: i64) -> Listing {
    let data: Value = serde_json::from_str(body)?;
    let entries = data[0]["result"]["data"]["json"].as_array().cloned().unwrap_or_default();
    let mut items = Vec::new();
    for entry in &entries {
        let link = truthy(entry.get("fourKUrl")).or(truthy(entry.get("hdUrl")));
        if let Some(link) = link.filter(|_| entry.get("type") == Some(&json!("LIVE_MP4"))) {
            items.push(item(
                format!("livebgs-{}", text_of(&entry["id"])),
                tidy(&truthy(entry.get("title")).map(text_of).unwrap_or_default()),
                truthy(entry.get("thumbnailImageUrl")).map(text_of).unwrap_or_default(),
                text_of(link),
                ".mp4",
            ));
        }
    }
    let pages = if entries.len() as i64 >= LIVEBGS_PAGE { page + 1 } else { page };
    Ok((pages, items))
}

fn moewalls(query: &str, page: i64) -> Listing {
    let mut url = format!("https://moewalls.com/page/{page}/");
    if !query.is_empty() {
        url += &format!("?s={}", quote(query, "", true));
    }
    Ok(moewalls_parse(&get(&url)?, page))
}

fn moewalls_parse(body: &str, page: i64) -> (i64, Vec<Item>) {
    let card = re(concat!(
        r#"<a title="([^"]*)" class="g1-frame" href="(https://moewalls\.com/[^"]+)">"#,
        r#"<div class="g1-frame-inner"><img [^>]*?src="([^"]+)""#
    ));
    let (mut items, mut seen) = (Vec::new(), std::collections::HashSet::new());
    for c in card.captures_iter(body) {
        let link = c[2].to_string();
        if seen.insert(link.clone()) {
            let slug = link.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string();
            items.push(item(format!("moewalls-{slug}"), tidy(&c[1]), c[3].to_string(), link, ".webm"));
        }
    }
    let pages = re(r"/page/(\d+)/").captures_iter(body).filter_map(|c| c[1].parse().ok()).chain([page]).max();
    (pages.unwrap(), items)
}

fn wpe_catalog(query: &str) -> Result<Vec<(String, String, String, String)>, Error> {
    let mut items = Vec::new();
    for entry in fs::read_dir(&*WPE_ITEMS)?.flatten() {
        let id = entry.file_name().to_string_lossy().into_owned();
        let Ok(raw) = fs::read(format!("{}/{id}/project.json", *WPE_ITEMS)) else { continue };
        let Ok(project) = serde_json::from_str::<Value>(&String::from_utf8_lossy(&raw)) else { continue };
        let title = truthy(project.get("title")).map(text_of).unwrap_or_else(|| id.clone());
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
        let kind = project.get("type").map(text_of).unwrap_or_default().to_lowercase();
        if (kind == "scene" || kind == "video") && title.to_lowercase().contains(&query.to_lowercase()) {
            let preview = project.get("preview").and_then(Value::as_str).unwrap_or("").to_string();
            items.push((title.to_lowercase(), title, id, preview));
        }
    }
    items.sort();
    Ok(items)
}

fn wpe_thumb(id: &str, preview: &str) -> String {
    let source = format!("{}/{id}/{preview}", *WPE_ITEMS);
    if !preview.to_lowercase().ends_with(".gif") {
        return source;
    }
    let target = format!("{}/wpe-{id}.jpg", *THUMBS);
    if let (false, Some(ffmpeg)) = (Path::new(&target).exists(), which("ffmpeg")) {
        let _ = fs::create_dir_all(&*THUMBS);
        if let Ok(mut child) = Command::new(ffmpeg)
            .args(["-v", "error", "-y", "-i", &source, "-frames:v", "1", &target])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            wait_timeout(&mut child, Duration::from_secs(15));
        }
    }
    target
}

fn wpe(query: &str, page: i64) -> Listing {
    let catalog = wpe_catalog(query)?;
    let start = ((page.max(1) - 1) as usize * WPE_PAGE).min(catalog.len());
    let chosen = &catalog[start..(start + WPE_PAGE).min(catalog.len())];
    let thumbs: Vec<std::sync::Mutex<String>> = chosen.iter().map(|_| Default::default()).collect();
    let next = AtomicUsize::new(0);
    thread::scope(|scope| {
        for _ in 0..6 {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(row) = chosen.get(i) else { break };
                *thumbs[i].lock().unwrap() = wpe_thumb(&row.2, &row.3);
            });
        }
    });
    let items = chosen
        .iter()
        .zip(thumbs)
        .map(|(row, thumb)| item(format!("wpe-{}", row.2), tidy(&row.1), thumb.into_inner().unwrap(), row.2.clone(), ".wpe"))
        .collect();
    Ok((1.max(catalog.len().div_ceil(WPE_PAGE) as i64), items))
}

fn wpe_launch(id: &str) {
    let mut command = Command::new("linux-wallpaperengine");
    command
        .args(["--screen-root", "DP-1", "--bg", &format!("{}/{id}", *WPE_ITEMS), "--assets-dir", &WPE_ASSETS])
        .args(["--scaling", "fill", "--silent"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let _ = command.spawn();
}

fn wpe_stop() {
    let held = fs::metadata(&*WPE_HOLD).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
    if held.is_some_and(|age| age < Duration::from_secs(30)) {
        return;
    }
    let _ = Command::new("pkill").args(["-f", WPE_PATTERN]).stderr(Stdio::null()).status();
    let _ = fs::remove_file(&*WPE_LAST);
}

fn wpe_restore() {
    let Ok(id) = fs::read_to_string(&*WPE_LAST) else { return };
    let id = id.trim();
    if id.is_empty() {
        return;
    }
    let _ = fs::File::create(&*WPE_HOLD);
    thread::sleep(Duration::from_secs(20));
    if env::var_os("WPE_DRY").is_some_and(|v| !v.is_empty()) {
        println!("would launch {id}");
        return;
    }
    wpe_launch(id);
}

fn wpe_hold(seconds: u64) {
    if let Ok(file) = fs::File::create(&*WPE_HOLD) {
        let when = SystemTime::now() - Duration::from_secs(30 - seconds);
        let _ = file.set_times(fs::FileTimes::new().set_accessed(when).set_modified(when));
    }
}

fn forget_static() {
    let _ = fs::remove_file(format!("/run/user/{}/wall-static-last", uid()));
}

fn wpe_apply(id: &str) {
    forget_static();
    wpe_hold(30);
    run_quiet("noctalia", &["msg", "plugin", "noctalia/mpvpaper:service", "all", "clear-all"]);
    let _ = Command::new("pkill").args(["-f", WPE_PATTERN]).stderr(Stdio::null()).status();
    wpe_launch(id);
    let _ = fs::write(&*WPE_LAST, id);
    wpe_hold(8);
}

fn steam(query: &str, page: i64) -> Listing {
    let mut params = vec![
        ("appid", "431960".to_string()),
        ("section", "readytouseitems".into()),
        ("numperpage", "24".into()),
        ("p", page.to_string()),
    ];
    if query.is_empty() {
        params.extend([("browsesort", "trend".into()), ("actualsort", "trend".into()), ("days", "7".into())]);
    } else {
        params.extend([("searchtext", query.to_string()), ("browsesort", "textsearch".into()), ("actualsort", "textsearch".into())]);
    }
    let encoded: Vec<String> = params.iter().map(|(k, v)| format!("{}={}", quote(k, "", true), quote(v, "", true))).collect();
    Ok(steam_parse(&get(&format!("https://steamcommunity.com/workshop/browse/?{}", encoded.join("&")))?, page))
}

fn steam_parse(body: &str, page: i64) -> (i64, Vec<Item>) {
    let data = body.replace("\\\\\"", "\0").replace("\\\"", "\"");
    let header = re(r#""publishedfileid":"(\d+)""#);
    let card = re(r#""preview_url":"([^"]+)","title":"([^"]*)""#);
    let (mut items, mut seen, mut pos) = (Vec::new(), std::collections::HashSet::new(), 0);
    while let Some(head) = header.captures_at(&data, pos) {
        let whole = head.get(0).unwrap();
        let limit = data[whole.end()..].find("\"publishedfileid\"").map_or(data.len(), |i| whole.end() + i);
        match card.captures_at(&data, whole.end()).filter(|c| c.get(0).unwrap().start() < limit) {
            Some(c) => {
                let id = head[1].to_string();
                if seen.insert(id.clone()) {
                    items.push(item(
                        format!("steam-{id}"),
                        tidy(&c[2].replace('\0', "\"")),
                        c[1].replace("\\/", "/") + "?imw=480&imh=270&ima=fit",
                        id,
                        ".steam",
                    ));
                }
                pos = c.get(0).unwrap().end();
            }
            None => pos = whole.start() + 1,
        }
    }
    let count = re(r"([\d,]+) entries")
        .captures(body)
        .and_then(|c| c[1].replace(',', "").parse::<i64>().ok())
        .unwrap_or(items.len() as i64);
    items.truncate(24);
    (page.max((count + 23).div_euclid(24)), items)
}

fn notify(title: &str, message: &str, urgent: bool) {
    let mut command = Command::new("notify-send");
    command.args(["-a", "Wallpapers"]);
    if urgent {
        command.args(["-u", "critical"]);
    }
    let _ = command.args([title, message]).stderr(Stdio::null()).status();
}

fn steam_download(id: &str) -> Option<String> {
    let user = fs::read_to_string(&*STEAM_USER).map(|u| u.trim().to_string()).unwrap_or_default();
    if user.is_empty() {
        return Some("Steam login needed: run steamcmd-login in a terminal.".into());
    }
    let target = format!("{}/{id}", *WPE_ITEMS);
    let _ = fs::create_dir_all(&*STEAM_HOME);
    let lock = fs::File::create(format!("{}/download.lock", *STEAM_HOME)).ok()?;
    unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) };
    if Path::new(&target).is_dir() {
        return None;
    }
    let out = capture(
        Command::new("steamcmd").args(["+login", &user, "+workshop_download_item", "431960", id, "+quit"]).env("HOME", &*STEAM_HOME),
        Duration::from_secs(3000),
    )
    .unwrap_or_default();
    if out.contains("Invalid Password") {
        return Some("Steam login expired: run steamcmd-login in a terminal.".into());
    }
    let Some(found) = re(r#"Success\. Downloaded item \d+ to "([^"]+)""#).captures(&out) else {
        let plain = re(r"\x1b\[[0-9;]*m").replace_all(&out, "");
        let last = plain.lines().filter(|l| l.contains("ERROR")).last();
        return Some(last.map_or("steamcmd did not finish the download.".into(), |l| l.chars().take(160).collect()));
    };
    let from = found[1].to_string();
    if fs::canonicalize(&from).ok() != fs::canonicalize(&target).ok() {
        let _ = fs::create_dir_all(&*WPE_ITEMS);
        if fs::rename(&from, &target).is_err() {
            let _ = Command::new("mv").args(["--", &from, &target]).status();
        }
    }
    None
}

fn applying() -> bool {
    env::var_os("WALL_SITES_NO_APPLY").is_none_or(|v| v.is_empty())
}

fn steam_fetch(id: &str, title: &str) {
    if let Some(problem) = steam_download(id) {
        return notify("Steam Workshop", &problem, true);
    }
    if applying() {
        wpe_apply(id);
    }
    notify("Steam Workshop", &format!("{title} is set."), false);
}

fn moewalls_video(page: &str) -> Option<String> {
    let body = get(page).ok()?;
    let found = re(r#"<source src="(/wp-content/uploads/preview/[^"]+)""#).captures(&body)?;
    Some(format!("https://moewalls.com{}", &found[1]))
}

fn video_fetch(id: &str, extension: &str, url: &str, title: &str) {
    let folder = expand("~/Pictures/Live Wallpapers");
    let _ = fs::create_dir_all(&folder);
    let name = re("[^a-z0-9]+").replace_all(&title.to_lowercase(), "-").trim_matches('-').to_string();
    let prefix = if name.is_empty() { String::new() } else { name + "_" };
    let mut dest = format!("{folder}/{prefix}{id}{extension}");
    let (exact, suffix) = (format!("{id}{extension}"), format!("_{id}{extension}"));
    for entry in fs::read_dir(&folder).into_iter().flatten().flatten() {
        let file = entry.file_name().to_string_lossy().into_owned();
        if file == exact || file.ends_with(&suffix) {
            dest = format!("{folder}/{file}");
        }
    }
    if fs::metadata(&dest).map_or(true, |m| m.len() == 0) {
        let mut url = url.to_string();
        if extension == ".webm" {
            match moewalls_video(&url) {
                Some(video) => url = video,
                None => return notify("Wallpapers", &format!("No video found for {title}"), true),
            }
        }
        let part = format!("{dest}.part");
        let ok = Command::new("curl")
            .args(["-fsSL", "-A", AGENT, "-o", &part, &url])
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            let _ = fs::remove_file(&part);
            return notify("Wallpapers", &format!("Download failed: {title}"), true);
        }
        let _ = fs::rename(&part, &dest);
    }
    if applying() {
        forget_static();
        wpe_stop();
        run_quiet("noctalia", &["msg", "plugin", "noctalia/mpvpaper:service", "all", "set", &dest]);
    }
    notify("Wallpapers", &format!("{title} is set."), false);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
    match arg(1) {
        "preview" => match moewalls_video(arg(2)) {
            Some(video) => println!("{video}"),
            None => std::process::exit(1),
        },
        "steam-fetch" => steam_fetch(arg(2), arg(3)),
        "video-fetch" => video_fetch(arg(2), arg(3), arg(4), arg(5)),
        "wpe-apply" => wpe_apply(arg(2)),
        "wpe-stop" => wpe_stop(),
        "wpe-restore" => wpe_restore(),
        site => {
            let lister: fn(&str, i64) -> Listing = match site {
                "motionbgs" => motionbgs,
                "livebgs" => livebgs,
                "moewalls" => moewalls,
                "wpe" => wpe,
                "steam" => steam,
                _ => {
                    eprintln!("usage: wall-sites <motionbgs|livebgs|moewalls|wpe|steam> PAGE [QUERY...]");
                    eprintln!("       wall-sites <preview|steam-fetch|video-fetch|wpe-apply|wpe-stop|wpe-restore> ...");
                    std::process::exit(2);
                }
            };
            let Ok(page) = arg(2).trim().parse::<i64>() else {
                eprintln!("wall-sites: bad page number {:?}", arg(2));
                std::process::exit(2);
            };
            let query = args.get(3..).unwrap_or_default().join(" ").trim().to_string();
            match lister(&query, page) {
                Ok((pages, items)) => {
                    let mut out = format!("{pages}\n");
                    for (a, b, c, d, e) in items {
                        let fields = [a, b, c, d, e].map(|f| f.replace('\t', " "));
                        out += &fields.join("\t");
                        out.push('\n');
                    }
                    print!("{out}");
                }
                Err(e) => {
                    eprintln!("wall-sites: {e}");
                    std::process::exit(1);
                }
            }
        }
    }
}
