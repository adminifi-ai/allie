use super::{DiscoveredSurface, html_title_from_text, route_to_id};
use crate::{AllieError, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

const LIVE_DISCOVERY_MAX_PAGES: usize = 24;
const LIVE_DISCOVERY_MAX_BYTES: usize = 1024 * 1024;
const LIVE_DISCOVERY_TIMEOUT_MS: u64 = 2_000;

pub(super) fn discover_live_base_url_surfaces(base_url: &str) -> Result<Vec<DiscoveredSurface>> {
    let base = LiveBaseUrl::parse(base_url)?;
    let mut queue = vec![base.start_route.clone()];
    let mut queued = BTreeSet::from([base.start_route.clone()]);
    let mut visited = BTreeSet::new();
    let mut surfaces = BTreeMap::new();

    for route in discover_sitemap_routes(&base)? {
        if queued.len() >= LIVE_DISCOVERY_MAX_PAGES {
            break;
        }
        if queued.insert(route.clone()) {
            queue.push(route);
        }
    }

    while let Some(route) = queue.pop() {
        if visited.len() >= LIVE_DISCOVERY_MAX_PAGES || !visited.insert(route.clone()) {
            continue;
        }
        let page = match fetch_live_html_page(&base, &route) {
            Ok(Some(page)) => page,
            Ok(None) | Err(AllieError::Io { .. }) => continue,
            Err(source) => return Err(source),
        };
        surfaces.insert(
            route.clone(),
            DiscoveredSurface {
                id: route_to_id(&route),
                route: route.clone(),
                title: html_title_from_text(&page.body).unwrap_or_else(|| route_to_id(&route)),
                source: "base-url-crawl".to_string(),
                confidence: "live_http_discovered".to_string(),
                user_stories: vec![format!("As an application user, I can reach {}", route)],
                provenance: vec![base.url_for_route(&route)],
            },
        );
        for href in html_links(&page.body) {
            if visited.len() + queued.len() >= LIVE_DISCOVERY_MAX_PAGES {
                break;
            }
            if let Some(next_route) = resolve_live_link(&base, &route, &href)
                && queued.insert(next_route.clone())
            {
                queue.push(next_route);
            }
        }
    }

    Ok(surfaces.into_values().collect())
}

#[derive(Debug)]
struct LiveBaseUrl {
    host: String,
    port: u16,
    start_route: String,
}

impl LiveBaseUrl {
    fn parse(value: &str) -> Result<Self> {
        let lower = value.to_ascii_lowercase();
        let rest = if lower.starts_with("http://") {
            &value["http://".len()..]
        } else {
            return Err(AllieError::InvalidManifest(
                "live base_url discovery currently supports http:// targets".to_string(),
            ));
        };
        let (authority, path) = rest
            .split_once('/')
            .map(|(authority, path)| (authority, format!("/{path}")))
            .unwrap_or((rest, "/".to_string()));
        let (host, port) = parse_host_port(authority)?;
        Ok(Self {
            host,
            port,
            start_route: normalize_live_route(&path),
        })
    }

    fn authority(&self) -> String {
        if self.port == 80 {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    fn url_for_route(&self, route: &str) -> String {
        format!("http://{}{}", self.authority(), route)
    }
}

struct LiveHtmlPage {
    body: String,
}

struct LiveHttpResponse {
    headers: String,
    body: String,
}

impl LiveHttpResponse {
    fn content_type(&self) -> Option<String> {
        self.headers
            .lines()
            .find(|line| line.to_ascii_lowercase().starts_with("content-type:"))
            .map(|line| line.to_ascii_lowercase())
    }
}

fn parse_host_port(authority: &str) -> Result<(String, u16)> {
    if authority.trim().is_empty() || authority.contains('@') {
        return Err(AllieError::InvalidManifest(format!(
            "invalid live discovery base_url authority {authority}"
        )));
    }
    let (host, port) = authority
        .rsplit_once(':')
        .map(|(host, port)| {
            let parsed = port.parse::<u16>().map_err(|_| {
                AllieError::InvalidManifest(format!("invalid live discovery base_url port {port}"))
            })?;
            Ok((host.to_string(), parsed))
        })
        .unwrap_or_else(|| Ok((authority.to_string(), 80)))?;
    if host.trim().is_empty() {
        return Err(AllieError::InvalidManifest(
            "live discovery base_url requires a host".to_string(),
        ));
    }
    Ok((host, port))
}

fn fetch_live_response(base: &LiveBaseUrl, route: &str) -> Result<Option<LiveHttpResponse>> {
    match fetch_live_response_once(base, route) {
        Ok(response) => Ok(response),
        Err(AllieError::Io { .. }) => {
            std::thread::sleep(Duration::from_millis(10));
            fetch_live_response_once(base, route)
        }
        Err(source) => Err(source),
    }
}

fn fetch_live_response_once(base: &LiveBaseUrl, route: &str) -> Result<Option<LiveHttpResponse>> {
    let address = format!("{}:{}", base.host, base.port);
    let mut stream = TcpStream::connect(&address).map_err(|source| AllieError::Io {
        context: format!(
            "connect live discovery target {}",
            base.url_for_route(route)
        ),
        source,
    })?;
    let timeout = Duration::from_millis(LIVE_DISCOVERY_TIMEOUT_MS);
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|source| AllieError::Io {
            context: format!("set read timeout for live discovery target {address}"),
            source,
        })?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|source| AllieError::Io {
            context: format!("set write timeout for live discovery target {address}"),
            source,
        })?;
    let request = format!(
        "GET {route} HTTP/1.1\r\nHost: {}\r\nAccept: text/html, application/xml;q=0.9, text/xml;q=0.9, */*;q=0.1\r\nConnection: close\r\nUser-Agent: allie-live-discovery/0.1\r\n\r\n",
        base.authority()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|source| AllieError::Io {
            context: format!(
                "request live discovery target {}",
                base.url_for_route(route)
            ),
            source,
        })?;
    let mut response = Vec::new();
    stream
        .take(LIVE_DISCOVERY_MAX_BYTES as u64)
        .read_to_end(&mut response)
        .map_err(|source| AllieError::Io {
            context: format!("read live discovery target {}", base.url_for_route(route)),
            source,
        })?;
    let response = String::from_utf8_lossy(&response);
    let Some((headers, body)) = response.split_once("\r\n\r\n") else {
        return Ok(None);
    };
    if !headers
        .lines()
        .next()
        .is_some_and(|status| status.contains(" 2"))
    {
        return Ok(None);
    }
    Ok(Some(LiveHttpResponse {
        headers: headers.to_string(),
        body: body.to_string(),
    }))
}

fn fetch_live_html_page(base: &LiveBaseUrl, route: &str) -> Result<Option<LiveHtmlPage>> {
    let Some(response) = fetch_live_response(base, route)? else {
        return Ok(None);
    };
    if response
        .content_type()
        .is_some_and(|content_type| !content_type.contains("text/html"))
    {
        return Ok(None);
    }
    Ok(Some(LiveHtmlPage {
        body: response.body,
    }))
}

fn discover_sitemap_routes(base: &LiveBaseUrl) -> Result<Vec<String>> {
    let response = match fetch_live_response(base, "/sitemap.xml") {
        Ok(Some(response)) => response,
        Ok(None) | Err(AllieError::Io { .. }) => return Ok(Vec::new()),
        Err(source) => return Err(source),
    };
    if response.content_type().is_some_and(|content_type| {
        !content_type.contains("xml") && !content_type.contains("text/plain")
    }) {
        return Ok(Vec::new());
    }
    Ok(sitemap_routes(base, &response.body))
}

fn sitemap_routes(base: &LiveBaseUrl, text: &str) -> Vec<String> {
    let mut routes = BTreeSet::new();
    let lower = text.to_ascii_lowercase();
    let mut index = 0;
    while let Some(position) = lower[index..].find("<loc>") {
        let start = index + position + "<loc>".len();
        let Some(end_offset) = lower[start..].find("</loc>") else {
            break;
        };
        let end = start + end_offset;
        if let Some(route) = resolve_live_link(base, "/", text[start..end].trim()) {
            routes.insert(route);
        }
        index = end + "</loc>".len();
    }
    routes.into_iter().collect()
}

fn html_links(text: &str) -> Vec<String> {
    let mut links = Vec::new();
    let lower = text.to_ascii_lowercase();
    let mut index = 0;
    while let Some(position) = lower[index..].find("href") {
        index += position + "href".len();
        let Some(equals_offset) = lower[index..].find('=') else {
            break;
        };
        index += equals_offset + 1;
        let bytes = text.as_bytes();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        let quote = bytes[index];
        if quote != b'"' && quote != b'\'' {
            continue;
        }
        index += 1;
        let start = index;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if index <= bytes.len() {
            links.push(text[start..index].to_string());
        }
    }
    links
}

fn resolve_live_link(base: &LiveBaseUrl, current_route: &str, href: &str) -> Option<String> {
    let href = href.trim();
    let lower_href = href.to_ascii_lowercase();
    if href.is_empty()
        || href.starts_with('#')
        || href.starts_with("//")
        || lower_href.starts_with("mailto:")
        || lower_href.starts_with("javascript:")
        || lower_href.starts_with("tel:")
    {
        return None;
    }
    if lower_href.starts_with("http://") {
        let link = LiveBaseUrl::parse(href).ok()?;
        return (link.host == base.host && link.port == base.port).then_some(link.start_route);
    }
    if lower_href.starts_with("https://") {
        return None;
    }
    if href.starts_with('/') {
        return Some(normalize_live_route(href));
    }
    Some(normalize_live_route(&join_relative_live_route(
        current_route,
        href,
    )))
}

fn join_relative_live_route(current_route: &str, href: &str) -> String {
    let base_dir = current_route
        .rsplit_once('/')
        .map(|(prefix, _)| if prefix.is_empty() { "/" } else { prefix })
        .unwrap_or("/");
    if base_dir == "/" {
        format!("/{href}")
    } else {
        format!("{base_dir}/{href}")
    }
}

fn normalize_live_route(value: &str) -> String {
    let without_fragment = value.split('#').next().unwrap_or("/");
    let without_query = without_fragment.split('?').next().unwrap_or("/");
    let mut segments = Vec::new();
    for segment in without_query.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            value => segments.push(value),
        }
    }
    if segments.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", segments.join("/"))
    }
}
