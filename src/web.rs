//! DuckDuckGo HTML search, matching CCursor `web.ts` performWebSearch.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRef {
    pub title: String,
    pub url: String,
    pub chunk: String,
}

pub fn parse_ddg_html(html: &str, max: usize) -> Vec<SearchRef> {
    let mut refs = Vec::new();
    let mut rest = html;
    while refs.len() < max {
        let Some(a_at) = rest.find("result__a") else {
            break;
        };
        let window = &rest[a_at.saturating_sub(80)..];
        let href = attr_after(window, "href=\"").unwrap_or_default();
        let title = strip_tags(&between(window, ">", "</a>").unwrap_or_default());
        let after = rest.get(a_at..).unwrap_or("");
        let snippet = after
            .find("result__snippet")
            .and_then(|i| after.get(i..))
            .map(|s| {
                let inner = s.find('>').and_then(|gt| s.get(gt + 1..)).unwrap_or(s);
                strip_tags(inner.split('<').next().unwrap_or(inner))
            })
            .unwrap_or_default();
        let url = decode_ddg_href(&href);
        if !title.is_empty() && url.starts_with("http") {
            refs.push(SearchRef {
                title,
                url,
                chunk: snippet.chars().take(400).collect(),
            });
        }
        rest = rest.get(a_at + 9..).unwrap_or("");
    }
    refs
}

fn host_is_blocked(host: &str) -> bool {
    let host = host.trim().trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
    if host == "localhost"
        || host == "127.0.0.1"
        || host == "::1"
        || host == "0.0.0.0"
        || host.ends_with(".local")
        || host == "metadata.google.internal"
    {
        return true;
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
            }
            std::net::IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified(),
        };
    }
    false
}

pub async fn fetch_url(url: &str) -> Result<(String, String), String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("url must be http(s)".into());
    }
    if let Ok(parsed) = url.parse::<reqwest::Url>() {
        if host_is_blocked(parsed.host_str().unwrap_or("")) {
            return Err("localhost is not allowed".into());
        }
    }
    #[cfg(test)]
    {
        return Ok((
            url.to_owned(),
            format!("# {url}\n\nstub fetch body"),
        ));
    }
    #[cfg(not(test))]
    {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt
                    .url()
                    .host_str()
                    .is_some_and(host_is_blocked)
                {
                    attempt.error("redirect to blocked host")
                } else if attempt.previous().len() > 5 {
                    attempt.error("too many redirects")
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .map_err(|e| e.to_string())?;
        let response = client
            .get(url)
            .header(
                "user-agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Cursor/3.4",
            )
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        let final_url = response.url().to_string();
        let ctype = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let text = response.text().await.map_err(|e| e.to_string())?;
        let text = if text.len() > 100_000 {
            text.chars().take(100_000).collect()
        } else {
            text
        };
        let markdown = if ctype.contains("text/html") || text.trim_start().starts_with("<!DOCTYPE")
        {
            format!("# {final_url}\n\n{}", strip_tags(&text))
        } else {
            format!("# {final_url}\n\n{text}")
        };
        Ok((final_url, markdown))
    }
}

pub async fn search_web(term: &str) -> Result<Vec<SearchRef>, String> {
    #[cfg(test)]
    {
        let safe = term.replace('<', "").replace('>', "").replace('"', "");
        return Ok(parse_ddg_html(
            &format!(
                r#"<a class="result__a" href="https://example.com/search">Stub {safe}</a>
<div class="result__snippet">stub chunk</div>"#
            ),
            8,
        ));
    }
    #[cfg(not(test))]
    {
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoding_or_query(term)
        );
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| e.to_string())?;
        let html = client
            .get(&url)
            .header(
                "user-agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Cursor/3.4",
            )
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .text()
            .await
            .map_err(|e| e.to_string())?;
        Ok(parse_ddg_html(&html, 8))
    }
}

fn urlencoding_or_query(term: &str) -> String {
    let mut out = String::new();
    for b in term.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn attr_after<'a>(s: &'a str, key: &str) -> Option<String> {
    let at = s.find(key)? + key.len();
    let rest = s.get(at..)?;
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

fn between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let a = s.find(start)? + start.len();
    let rest = s.get(a..)?;
    let b = rest.find(end)?;
    rest.get(..b)
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut skip = false;
    for ch in s.chars() {
        match ch {
            '<' => skip = true,
            '>' => skip = false,
            _ if !skip => out.push(ch),
            _ => {}
        }
    }
    html_unescape(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn decode_ddg_href(href: &str) -> String {
    if let Some(rest) = href.split("uddg=").nth(1) {
        let enc = rest.split('&').next().unwrap_or(rest);
        return enc.replace("%3A", ":").replace("%2F", "/");
    }
    href.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ddg_extracts_result_a() {
        let html = r#"<a class="result__a" href="https://example.com/a">Example Title</a>
<div class="result__snippet">hello chunk</div>"#;
        let refs = parse_ddg_html(html, 3);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].title, "Example Title");
        assert!(refs[0].url.contains("example.com"));
        assert!(refs[0].chunk.contains("hello"));
    }

    #[test]
    fn query_encodes_utf8_percent_not_codepoint() {
        let q = urlencoding_or_query("中");
        assert_eq!(q, "%E4%B8%AD");
        assert!(!q.contains("%4E2D"));
    }

    #[test]
    fn fetch_url_blocks_loopback_and_rfc1918() {
        assert!(host_is_blocked("127.0.0.1"));
        assert!(host_is_blocked("10.0.0.1"));
        assert!(host_is_blocked("192.168.1.1"));
        assert!(host_is_blocked("169.254.169.254"));
        assert!(!host_is_blocked("example.com"));
    }
}
