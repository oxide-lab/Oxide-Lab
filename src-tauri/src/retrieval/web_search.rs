use crate::retrieval::types::RetrievalSource;
use scraper::{Html, Selector};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct LiteSearchOptions {
    pub max_results: usize,
    pub max_snippet_chars: usize,
    pub timeout: Duration,
}

impl Default for LiteSearchOptions {
    fn default() -> Self {
        Self {
            max_results: 8,
            max_snippet_chars: 420,
            timeout: Duration::from_secs(10),
        }
    }
}

fn ua() -> &'static str {
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36"
}

fn trunc_chars(input: &str, max: usize) -> String {
    if input.chars().count() <= max {
        return input.to_string();
    }
    input.chars().take(max).collect()
}

fn ddg_endpoints() -> [&'static str; 3] {
    [
        "https://html.duckduckgo.com/html/",
        "https://duckduckgo.com/html/",
        "https://lite.duckduckgo.com/lite/",
    ]
}

pub async fn duckduckgo_lite_search(
    query: &str,
    options: &LiteSearchOptions,
) -> Result<Vec<RetrievalSource>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }

    let client = reqwest::Client::builder()
        .timeout(options.timeout)
        .connect_timeout(Duration::from_secs(7))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| e.to_string())?;

    let mut failures = Vec::new();
    for endpoint in ddg_endpoints() {
        // Retry endpoint because DDG can be intermittently unavailable or rate-limited.
        for attempt in 1..=2 {
            let response = client
                .get(endpoint)
                .query(&[("q", q)])
                .header(reqwest::header::USER_AGENT, ua())
                .header(
                    reqwest::header::ACCEPT,
                    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                )
                .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9,ru;q=0.8")
                .header(reqwest::header::REFERER, "https://duckduckgo.com/")
                .send()
                .await;

            let response = match response {
                Ok(resp) => resp,
                Err(err) => {
                    failures.push(format!("{endpoint} send failed (attempt {attempt}): {err}"));
                    sleep(Duration::from_millis(180)).await;
                    continue;
                }
            };

            if !response.status().is_success() {
                failures.push(format!(
                    "{endpoint} responded with status {} (attempt {attempt})",
                    response.status()
                ));
                sleep(Duration::from_millis(180)).await;
                continue;
            }

            let body = match response.text().await {
                Ok(text) => text,
                Err(err) => {
                    failures.push(format!("{endpoint} body read failed (attempt {attempt}): {err}"));
                    sleep(Duration::from_millis(180)).await;
                    continue;
                }
            };

            match parse_ddg_html(&body, options.max_results, options.max_snippet_chars) {
                Ok(rows) if !rows.is_empty() => return Ok(rows),
                Ok(_) => failures.push(format!("{endpoint} returned no parsable results (attempt {attempt})")),
                Err(err) => failures.push(format!("{endpoint} parse failed (attempt {attempt}): {err}")),
            }
            sleep(Duration::from_millis(180)).await;
        }
    }

    Err(format!(
        "web search failed across all DuckDuckGo endpoints: {}",
        failures.join(" | ")
    ))
}

pub fn parse_ddg_html(
    html: &str,
    max_results: usize,
    max_snippet_chars: usize,
) -> Result<Vec<RetrievalSource>, String> {
    let doc = Html::parse_document(html);
    let result_selector = Selector::parse(".result").map_err(|e| e.to_string())?;
    let title_selector = Selector::parse(".result__title").map_err(|e| e.to_string())?;
    let snippet_selector = Selector::parse(".result__snippet").map_err(|e| e.to_string())?;
    let link_selector = Selector::parse(".result__a").map_err(|e| e.to_string())?;
    let lite_link_selector = Selector::parse("a.result-link").map_err(|e| e.to_string())?;
    let lite_snippet_selector = Selector::parse("td.result-snippet").map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for result in doc.select(&result_selector) {
        if out.len() >= max_results {
            break;
        }
        let title = result
            .select(&title_selector)
            .next()
            .map(|v| v.text().collect::<String>())
            .unwrap_or_default()
            .trim()
            .to_string();

        let snippet = result
            .select(&snippet_selector)
            .next()
            .map(|v| v.text().collect::<String>())
            .unwrap_or_default()
            .trim()
            .to_string();

        let url = result
            .select(&link_selector)
            .next()
            .and_then(|a| a.value().attr("href"))
            .unwrap_or_default()
            .trim()
            .to_string();

        if title.is_empty() || snippet.is_empty() || url.is_empty() {
            continue;
        }

        out.push(RetrievalSource {
            source_type: "web".to_string(),
            title,
            url: Some(url),
            path: None,
            snippet: trunc_chars(&snippet, max_snippet_chars),
            score: None,
        });
    }

    // Fallback parser for DDG Lite layout.
    if out.is_empty() {
        for link in doc.select(&lite_link_selector) {
            if out.len() >= max_results {
                break;
            }
            let title = link.text().collect::<String>().trim().to_string();
            let url = link.value().attr("href").unwrap_or_default().trim().to_string();
            if title.is_empty() || url.is_empty() {
                continue;
            }
            let snippet = doc
                .select(&lite_snippet_selector)
                .next()
                .map(|v| v.text().collect::<String>())
                .unwrap_or_default()
                .trim()
                .to_string();
            out.push(RetrievalSource {
                source_type: "web".to_string(),
                title,
                url: Some(url),
                path: None,
                snippet: trunc_chars(&snippet, max_snippet_chars),
                score: None,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lite_results() {
        let html = r#"
        <div class="result">
          <h2 class="result__title"><a class="result__a" href="https://example.com">Example</a></h2>
          <a class="result__url" href="https://example.com">https://example.com</a>
          <a class="result__snippet">Snippet text</a>
        </div>
        "#;
        let rows = parse_ddg_html(html, 5, 100).expect("parse");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "Example");
    }

    #[test]
    fn parses_lite_layout_result_link() {
        let html = r#"
        <table>
          <tr><td><a class="result-link" href="https://example.org">Example Org</a></td></tr>
          <tr><td class="result-snippet">Short snippet</td></tr>
        </table>
        "#;
        let rows = parse_ddg_html(html, 5, 120).expect("parse");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "Example Org");
        assert_eq!(rows[0].url.as_deref(), Some("https://example.org"));
    }
}
