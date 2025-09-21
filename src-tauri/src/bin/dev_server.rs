use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
};

use anyhow::{Context, Result};

fn main() -> Result<()> {
    let root = resolve_root()?;
    let root = Arc::new(root);
    let addr = "127.0.0.1:1420";
    let listener = TcpListener::bind(addr).context("Failed to bind dev-server address")?;
    println!("Dev server listening on http://{addr}");

    for stream in listener.incoming() {
        let root = Arc::clone(&root);
        thread::spawn(move || {
            if let Err(err) = handle_connection(stream, &root) {
                eprintln!("[dev-server] {err}");
            }
        });
    }

    Ok(())
}

fn resolve_root() -> Result<PathBuf> {
    let candidates = [
        PathBuf::from("frontend"),
        PathBuf::from("../frontend"),
        PathBuf::from("../../frontend"),
    ];

    for candidate in candidates.iter() {
        if candidate.exists() {
            return candidate
                .canonicalize()
                .with_context(|| format!("Failed to canonicalise path {candidate:?}"));
        }
    }

    anyhow::bail!(
        "Unable to locate frontend directory (looked in {:#?})",
        candidates
    );
}

fn handle_connection(stream: std::io::Result<std::net::TcpStream>, root: &Path) -> Result<()> {
    let mut stream = stream?;
    let mut buffer = [0_u8; 4096];
    let read = stream.read(&mut buffer)?;
    if read == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..read]);
    let line = request.lines().next().unwrap_or("");
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");

    if method != "GET" && method != "HEAD" {
        return respond(&mut stream, 405, "Method Not Allowed", b"", "text/plain");
    }

    let path = sanitize_path(path);
    if path.contains("../") || path.contains("./") {
        return respond(&mut stream, 400, "Bad Request", b"", "text/plain");
    }

    let file_path = if path == "/" {
        root.join("index.html")
    } else {
        root.join(path.trim_start_matches('/'))
    };

    if !file_path.exists() {
        let body = b"Not Found";
        return respond(&mut stream, 404, "Not Found", body, "text/plain");
    }

    let mime = content_type(file_path.extension().and_then(|e| e.to_str()));
    let body = if method == "HEAD" {
        Vec::new()
    } else {
        fs::read(&file_path).with_context(|| format!("Failed to read {file_path:?}"))?
    };

    respond(&mut stream, 200, "OK", &body, mime)
}

fn respond(
    stream: &mut std::net::TcpStream,
    status: u16,
    text: &str,
    body: &[u8],
    mime: &str,
) -> Result<()> {
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        status,
        text,
        mime,
        body.len()
    )?;
    if !body.is_empty() {
        stream.write_all(body)?;
    }
    stream.flush()?;
    Ok(())
}

fn sanitize_path(path: &str) -> String {
    let without_query = path.split('?').next().unwrap_or("");
    percent_encoding::percent_decode_str(without_query)
        .decode_utf8_lossy()
        .into_owned()
}

fn content_type(ext: Option<&str>) -> &'static str {
    match ext.unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "ttf" => "font/ttf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}
