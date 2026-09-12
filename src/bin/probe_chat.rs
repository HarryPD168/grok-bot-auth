//! Live sand Stream probe. Does not print tokens.
use grok_bot_auth::chat::{decode_stream_payload, encode_get_chat_request, encode_stream_request};
use grok_bot_auth::client::SandClient;
use grok_bot_auth::connect::take_connect_frames;
use grok_bot_auth::sand::{now_ms, sand_headers};
use grok_bot_auth::store;
use reqwest::Client;

const BACKEND: &str = "https://api2.cursor.sh";

struct Case {
    label: &'static str,
    path: &'static str,
    model: &'static str,
    version: Option<&'static str>,
    client_type: Option<&'static str>,
    ghost: Option<&'static str>,
    ua: Option<&'static str>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let account = store::load_account()
        .await?
        .ok_or("no ~/.grok-bot-auth/account.json")?;
    let http = Client::builder()
        .use_rustls_tls()
        .no_proxy()
        .connect_timeout(std::time::Duration::from_secs(15))
        .build()?;
    let cases = [
        Case {
            label: "unified+sand+3.19.7+opus",
            path: "/aiserver.v1.ChatService/StreamUnifiedChat",
            model: "claude-opus-4-6",
            version: Some("3.19.7"),
            client_type: Some("sand"),
            ghost: Some("true"),
            ua: None,
        },
        Case {
            label: "unified+sand+0.47.0+opus",
            path: "/aiserver.v1.ChatService/StreamUnifiedChat",
            model: "claude-opus-4-6",
            version: Some("0.47.0"),
            client_type: Some("sand"),
            ghost: Some("true"),
            ua: None,
        },
        Case {
            label: "unified+sand+0.99.0+opus",
            path: "/aiserver.v1.ChatService/StreamUnifiedChat",
            model: "claude-opus-4-6",
            version: Some("0.99.0"),
            client_type: Some("sand"),
            ghost: Some("false"),
            ua: None,
        },
        Case {
            label: "unified+sand+3.19.7+grok",
            path: "/aiserver.v1.ChatService/StreamUnifiedChat",
            model: "grok-4.6",
            version: Some("3.19.7"),
            client_type: Some("sand"),
            ghost: Some("true"),
            ua: None,
        },
        Case {
            label: "unified+no-type+3.19.7+opus",
            path: "/aiserver.v1.ChatService/StreamUnifiedChat",
            model: "claude-opus-4-6",
            version: Some("3.19.7"),
            client_type: None,
            ghost: Some("false"),
            ua: None,
        },
        Case {
            label: "streamchat+sand+3.19.7+opus",
            path: "/aiserver.v1.AiService/StreamChat",
            model: "claude-opus-4-6",
            version: Some("3.19.7"),
            client_type: Some("sand"),
            ghost: Some("true"),
            ua: None,
        },
        Case {
            label: "inference+sand+3.19.7+opus",
            path: "/aiserver.v1.InferenceService/Stream",
            model: "claude-opus-4-6",
            version: Some("3.19.7"),
            client_type: Some("sand"),
            ghost: Some("true"),
            ua: None,
        },
        Case {
            label: "unified+nochecksum+opus",
            path: "/aiserver.v1.ChatService/StreamUnifiedChat",
            model: "claude-opus-4-6",
            version: Some("3.19.7"),
            client_type: Some("sand"),
            ghost: Some("false"),
            ua: None,
        },
        Case {
            label: "unified+electron-ua+opus",
            path: "/aiserver.v1.ChatService/StreamUnifiedChat",
            model: "claude-opus-4-6",
            version: Some("0.47.0"),
            client_type: Some("sand"),
            ghost: Some("false"),
            ua: Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) GrokBot/0.47.0 Chrome/132.0.0.0 Electron/34.0.0 Safari/537.36"),
        },
        Case {
            label: "streamchat+getchat+electron-ua+opus",
            path: "/aiserver.v1.AiService/StreamChat",
            model: "claude-opus-4-6",
            version: Some("0.47.0"),
            client_type: Some("sand"),
            ghost: Some("false"),
            ua: Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) GrokBot/0.47.0 Chrome/132.0.0.0 Electron/34.0.0 Safari/537.36"),
        },
        Case {
            label: "unified+rich-proto+sand+opus",
            path: "/aiserver.v1.ChatService/StreamUnifiedChat",
            model: "claude-opus-4-6",
            version: Some("3.19.7"),
            client_type: Some("sand"),
            ghost: Some("false"),
            ua: None,
        },
    ];
    let _ = SandClient::new();
    for case in cases {
        let framed = if case.path.contains("StreamChat") && !case.path.contains("Unified") {
            encode_get_chat_request(case.model, "Reply with exactly: pong", false)
        } else {
            encode_stream_request(case.model, "Reply with exactly: pong", Some("high"), false)
        };
        let mut req = http.post(format!("{BACKEND}{}", case.path));
        for (name, mut value) in sand_headers(&account, u128::from(now_ms())) {
            if name == "content-type" {
                continue;
            }
            if name == "x-cursor-client-version" {
                if let Some(version) = case.version {
                    value = version.to_owned();
                }
            }
            if name == "x-cursor-client-type" {
                match case.client_type {
                    None => continue,
                    Some(kind) => value = kind.to_owned(),
                }
            }
            if name == "x-ghost-mode" {
                if let Some(ghost) = case.ghost {
                    value = ghost.to_owned();
                }
            }
            if case.label.contains("nochecksum") && name == "x-cursor-checksum" {
                continue;
            }
            req = req.header(name, value);
        }
        if let Some(ua) = case.ua {
            req = req.header("user-agent", ua);
        }
        let response = req
            .header("content-type", "application/connect+proto")
            .header("connect-content-encoding", "identity")
            .body(framed)
            .send()
            .await?;
        let status = response.status().as_u16();
        let ctype = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let bytes = response.bytes().await.unwrap_or_default();
        let mut buffered = bytes.to_vec();
        let mut preview = String::new();
        let mut hex = String::new();
        if let Ok(frames) = take_connect_frames(&mut buffered) {
            if let Some(first) = frames.first() {
                hex = first.iter().take(24).map(|b| format!("{b:02x}")).collect::<Vec<_>>().join("");
                let (thinking, text) = decode_stream_payload(first);
                preview = format!("{thinking}{text}");
            }
        } else {
            preview = String::from_utf8_lossy(&bytes).chars().take(180).collect();
        }
        let outdated = preview.contains("outdated") || preview.contains("Please upgrade");
        let preview: String = preview.chars().take(160).collect();
        println!(
            "{} | HTTP {status} | ct={ctype} | outdated={outdated} | hex={hex} | {preview:?}",
            case.label
        );
    }
    Ok(())
}
