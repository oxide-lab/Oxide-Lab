use reqwest::Url;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct WebFetchOptions {
    pub timeout: Duration,
    pub max_body_bytes: usize,
}

impl Default for WebFetchOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(12),
            max_body_bytes: 1_500_000,
        }
    }
}

fn ua() -> &'static str {
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36"
}

fn is_ipv6_documentation(v6: Ipv6Addr) -> bool {
    let seg = v6.segments();
    seg[0] == 0x2001 && seg[1] == 0x0db8
}

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4 == Ipv4Addr::new(0, 0, 0, 0)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || is_ipv6_documentation(v6)
                || v6 == Ipv6Addr::LOCALHOST
        }
    }
}

fn validate_host(host: &str) -> Result<(), String> {
    let h = host.to_ascii_lowercase();
    if h == "localhost" || h.ends_with(".localhost") {
        return Err("localhost is not allowed".to_string());
    }
    Ok(())
}

async fn guard_private_network(url: &Url) -> Result<(), String> {
    let Some(host) = url.host_str() else {
        return Err("missing host".to_string());
    };
    validate_host(host)?;

    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(ip) {
            return Err("private or loopback addresses are blocked".to_string());
        }
        return Ok(());
    }

    let port = url.port_or_known_default().unwrap_or(443);
    let addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| format!("dns lookup failed: {e}"))?;
    for addr in addrs {
        if is_private_ip(addr.ip()) {
            return Err("private or loopback addresses are blocked".to_string());
        }
    }
    Ok(())
}

fn is_allowed_mime(mime: &str) -> bool {
    let lower = mime.to_ascii_lowercase();
    lower.starts_with("text/html") || lower.starts_with("text/plain")
}

pub async fn fetch_page_text(url: &str, options: &WebFetchOptions) -> Result<String, String> {
    let parsed = Url::parse(url).map_err(|e| format!("invalid url: {e}"))?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err("only http/https URLs are allowed".to_string()),
    }
    guard_private_network(&parsed).await?;

    let client = reqwest::Client::builder()
        .timeout(options.timeout)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(parsed.clone())
        .header(reqwest::header::USER_AGENT, ua())
        .send()
        .await
        .map_err(|e| format!("fetch failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("fetch failed with status {}", response.status()));
    }

    let mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("text/plain")
        .to_string();
    if !is_allowed_mime(&mime) {
        return Err(format!("unsupported content type: {mime}"));
    }

    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > options.max_body_bytes {
        return Err("response body is too large".to_string());
    }

    let raw = String::from_utf8_lossy(&bytes).to_string();
    if mime.starts_with("text/html") {
        return html2text::from_read(raw.as_bytes(), 120)
            .map_err(|e| format!("html extraction failed: {e}"));
    }
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_localhost() {
        let url = Url::parse("http://localhost/a").expect("url");
        let err = validate_host(url.host_str().unwrap_or_default()).expect_err("must fail");
        assert!(err.contains("localhost"));
    }
}
