use std::fs;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub(crate) fn write_live_discovery_manifest(root: &Path, base_url: &str) -> PathBuf {
    let manifest_path = root.join("live-flow.yml");
    fs::write(
        &manifest_path,
        format!(
            r#"id: live-discovery-flow
name: Live discovery flow
app_name: Live Test App
environment: local
target:
  kind: web
  base_url: {base_url}
policy:
  profile: wcag22-aa
  blocking_classes:
    - deterministic
browser:
  viewport:
    width: 1280
    height: 720
  color_scheme: light
  reduced_motion: reduce
  locale: en-US
  zoom: 1.0
flow:
  id: live-home-flow
  description: Live homepage
  states:
    - id: home
      path: /
      description: Live homepage
      required: true
      axe: true
      screenshot: true
      dom_snapshot: true
      accessibility_tree: true
      keyboard: true
      video: false
      trace: true
"#
        ),
    )
    .unwrap();
    manifest_path
}

pub(crate) struct LiveDiscoverySite {
    pub(crate) base_url: String,
    stop: Option<mpsc::Sender<()>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Drop for LiveDiscoverySite {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub(crate) fn start_live_discovery_site() -> LiveDiscoverySite {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop_tx, stop_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        loop {
            if stop_rx.try_recv().is_ok() {
                break;
            }
            match listener.accept() {
                Ok((stream, _)) => serve_live_discovery_connection(stream),
                Err(source) if source.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(source) => panic!("live discovery test server failed: {source}"),
            }
        }
    });

    LiveDiscoverySite {
        base_url: format!("http://{addr}"),
        stop: Some(stop_tx),
        handle: Some(handle),
    }
}

pub(crate) fn unused_local_base_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    format!("http://{addr}")
}

fn serve_live_discovery_connection(mut stream: TcpStream) {
    let mut request = [0_u8; 2048];
    let size = stream.read(&mut request).unwrap_or(0);
    let request = String::from_utf8_lossy(&request[..size]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let host = request
        .lines()
        .find_map(|line| line.strip_prefix("Host: "))
        .unwrap_or("127.0.0.1");
    let body = match path {
        "/" => {
            r#"<!doctype html><html><head><title>Live Home</title></head><body><nav><a href="/settings">Settings</a><a href="/help#top">Help</a><a href="https://example.invalid/offsite">Offsite</a><a href="mailto:a11y@example.invalid">Mail</a></nav></body></html>"#
        }
        "/account" => {
            r#"<!doctype html><html><head><title>Account</title></head><body><a href="/settings">Settings</a></body></html>"#
        }
        "/settings" => {
            r#"<!doctype html><html><head><title>Settings</title></head><body><a href="/help">Help</a></body></html>"#
        }
        "/help" => {
            r#"<!doctype html><html><head><title>Help Center</title></head><body><a href="/settings">Settings</a></body></html>"#
        }
        "/sitemap.xml" => {
            return write_live_discovery_response(
                &mut stream,
                "200 OK",
                "application/xml",
                &format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?><urlset><url><loc>http://{host}/account</loc></url><url><loc>https://example.invalid/offsite</loc></url></urlset>"#
                ),
            );
        }
        _ => {
            r#"<!doctype html><html><head><title>Missing</title></head><body>not found</body></html>"#
        }
    };
    let status = if matches!(path, "/" | "/account" | "/settings" | "/help") {
        "200 OK"
    } else {
        "404 Not Found"
    };
    write_live_discovery_response(&mut stream, status, "text/html; charset=utf-8", body);
}

fn write_live_discovery_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).unwrap();
}
