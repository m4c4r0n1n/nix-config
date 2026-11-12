use std::{fs, path::Path, thread};
use tiny_http::{Header, Request, Response, Server};
use tools::{current_wallpaper, expand};

fn live_wallpaper() -> Option<String> {
    let text = fs::read_to_string(expand("~/.local/state/noctalia/mpvpaper/assignments.json")).ok()?;
    let data: serde_json::Value = serde_json::from_str(&text).ok()?;
    data.get("assignments")?
        .as_object()?
        .values()
        .filter_map(|v| v.as_str())
        .find(|p| Path::new(p).is_file())
        .map(String::from)
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name, value).unwrap()
}

fn common<R: std::io::Read>(response: Response<R>, kind: &str) -> Response<R> {
    response
        .with_header(header("Content-Type", kind))
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("Access-Control-Allow-Origin", "*"))
}

fn handle(request: Request) {
    let route = request.url().split('?').next().unwrap_or("").to_string();
    if route == "/live-id" {
        let id = live_wallpaper().map(|p| sha1_smol::Sha1::from(p).digest().to_string()).unwrap_or_default();
        let _ = request.respond(common(Response::from_string(id), "text/plain"));
        return;
    }
    let path = if route == "/live" { live_wallpaper() } else { current_wallpaper() };
    let file = path.as_ref().and_then(|p| fs::File::open(p).ok());
    match (path, file) {
        (Some(path), Some(file)) => {
            let kind = mime_guess::from_path(&path).first_raw().unwrap_or("image/jpeg");
            let response = Response::from_file(file).with_chunked_threshold(usize::MAX);
            let _ = request.respond(common(response, kind));
        }
        _ => {
            let _ = request.respond(Response::empty(404));
        }
    }
}

fn main() {
    let server = Server::http("127.0.0.1:1928").expect("wall-server: cannot bind 127.0.0.1:1928");
    for request in server.incoming_requests() {
        thread::spawn(move || handle(request));
    }
}
