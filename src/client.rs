use std::future::Future;
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::connect::{parse_connect_payload, take_connect_frames, ChatDelta};
use crate::error::{Error, Result};
use crate::sand::{
    now_ms, sand_client_version, sand_headers, Account, OAUTH_CLIENT_ID, SAND_BACKEND,
};

#[derive(Clone)]
pub struct SandClient {
    http: Arc<Mutex<Client>>,
}

fn build_http(proxy: Option<&str>) -> Result<Client> {
    let mut builder = Client::builder()
        .use_rustls_tls()
        .http1_only()
        .connect_timeout(std::time::Duration::from_secs(15));
    match proxy.map(str::trim).filter(|value| !value.is_empty()) {
        Some(url) => {
            builder = builder.proxy(
                reqwest::Proxy::all(url)
                    .map_err(|error| Error::Msg(format!("outbound proxy: {error}")))?,
            );
        }
        None => {
            builder = builder.no_proxy();
        }
    }
    Ok(builder.build()?)
}

impl SandClient {
    pub fn new() -> Result<Self> {
        Self::with_outbound(None)
    }

    pub fn with_outbound(proxy: Option<&str>) -> Result<Self> {
        Ok(Self {
            http: Arc::new(Mutex::new(build_http(proxy)?)),
        })
    }

    pub fn set_outbound(&self, proxy: Option<&str>) -> Result<()> {
        let built = build_http(proxy)?;
        *self
            .http
            .lock()
            .map_err(|error| Error::Msg(error.to_string()))? = built;
        Ok(())
    }

    fn req(&self) -> Result<Client> {
        Ok(self
            .http
            .lock()
            .map_err(|error| Error::Msg(error.to_string()))?
            .clone())
    }

    async fn send_json(&self, account: &Account, path: &str, body: Value) -> Result<(u16, Value)> {
        let mut request = self
            .req()?
            .post(format!("{SAND_BACKEND}{path}"))
            .timeout(std::time::Duration::from_secs(30))
            .json(&body);
        for (name, value) in sand_headers(account, u128::from(now_ms())) {
            request = request.header(name, value);
        }
        let response = request.send().await?;
        let status = response.status().as_u16();
        let text = response.text().await?;
        let json = serde_json::from_str(&text).unwrap_or(json!({ "raw": text }));
        Ok((status, json))
    }

    pub async fn proxy_forward(
        &self,
        path: &str,
        headers: &[(String, String)],
        body: Vec<u8>,
    ) -> Result<(u16, String, Vec<u8>)> {
        let mut request = self
            .req()?
            .post(format!("{SAND_BACKEND}{path}"))
            .timeout(std::time::Duration::from_secs(30))
            .body(body);
        for (name, value) in headers {
            let lower = name.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "host"
                    | "connection"
                    | "proxy-connection"
                    | "transfer-encoding"
                    | "content-length"
                    | "accept-encoding"
                    | "connect-accept-encoding"
                    | "content-encoding"
                    | "connect-content-encoding"
            ) {
                continue;
            }
            request = request.header(name, value);
        }
        let response = request
            .header("accept-encoding", "identity")
            .header("connect-accept-encoding", "identity")
            .send()
            .await?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json")
            .to_owned();
        let bytes = response.bytes().await?.to_vec();
        Ok((status, content_type, bytes))
    }

    pub async fn proxy_forward_stream(
        &self,
        path: &str,
        headers: &[(String, String)],
        body: Vec<u8>,
    ) -> Result<reqwest::Response> {
        let mut request = self
            .req()?
            .post(format!("{SAND_BACKEND}{path}"))
            .timeout(std::time::Duration::from_secs(600))
            .body(body);
        for (name, value) in headers {
            let lower = name.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "host"
                    | "connection"
                    | "proxy-connection"
                    | "transfer-encoding"
                    | "content-length"
                    | "accept-encoding"
                    | "connect-accept-encoding"
                    | "content-encoding"
                    | "connect-content-encoding"
            ) {
                continue;
            }
            request = request.header(name, value);
        }
        let response = request
            .header("accept-encoding", "identity")
            .header("connect-accept-encoding", "identity")
            .send()
            .await?;
        Ok(response)
    }

    pub async fn available_models(&self, account: &Account) -> Result<Value> {
        let (status, body) = self
            .send_json(
                account,
                "/aiserver.v1.AiService/AvailableModels",
                json!({
                    "isNightly": false,
                    "includeLongContextModels": true,
                    "useModelParameters": true
                }),
            )
            .await?;
        if !(200..300).contains(&status) {
            return Err(Error::Msg(format!("AvailableModels HTTP {status}: {body}")));
        }
        Ok(body)
    }

    pub async fn period_usage(&self, account: &Account) -> Result<crate::quota::QuotaSnapshot> {
        let (status, body) = self
            .send_json(
                account,
                "/aiserver.v1.DashboardService/GetSandUsageStatus",
                json!({}),
            )
            .await?;
        if !(200..300).contains(&status) {
            return Err(Error::Msg(format!(
                "GetSandUsageStatus HTTP {status}: {body}"
            )));
        }
        Ok(crate::quota::parse_period_usage(&body, now_ms()))
    }

    pub async fn xai_usage(&self, token: &str) -> Result<crate::quota::QuotaSnapshot> {
        let response = self
            .req()?
            .get("https://cli-chat-proxy.grok.com/v1/billing?format=credits")
            .timeout(std::time::Duration::from_secs(20))
            .header("authorization", format!("Bearer {token}"))
            .header("x-xai-token-auth", token)
            .send()
            .await?;
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        if !(200..300).contains(&status) {
            return Err(Error::Msg(format!("xAI billing HTTP {status}")));
        }
        let body: Value = serde_json::from_str(&text).unwrap_or(json!({ "raw": text }));
        Ok(crate::quota::parse_xai_credits(&body, now_ms()))
    }

    pub async fn renew_inference(
        &self,
        _account: &Account,
    ) -> Result<crate::store::InferenceShort> {
        let credential = crate::store::load_renewal_credential()
            .await?
            .ok_or_else(|| {
                Error::Msg(
                    "Stream fail-closed: no SAND_INFERENCE_RENEWAL_CREDENTIAL on disk".into(),
                )
            })?;
        // This endpoint rejects Cursor session Authorization (401 invalid credential).
        // Body `{credential}` alone, plus sand client-type headers, is the live box path.
        let response = self
            .req()?
            .post(format!("{SAND_BACKEND}/sand-box/inference-credential"))
            .timeout(std::time::Duration::from_secs(30))
            .header("content-type", "application/json")
            .header("x-cursor-client-type", "sand")
            .header("x-cursor-client-version", sand_client_version())
            .header("x-cursor-client-source", "sand-desktop")
            .header("x-sand-box-namespace", "prod")
            .json(&json!({ "credential": credential }))
            .send()
            .await?;
        let status = response.status().as_u16();
        let body: Value = response.json().await.unwrap_or(json!({}));
        if !(200..300).contains(&status) {
            let keys: Vec<_> = body
                .as_object()
                .map(|obj| obj.keys().cloned().collect())
                .unwrap_or_default();
            return Err(Error::Msg(format!(
                "inference-credential HTTP {status} keys={keys:?}"
            )));
        }
        let grok_bot_token = body
            .get("grokBotToken")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Msg("inference-credential missing grokBotToken".into()))?
            .to_owned();
        let access_token = body
            .get("accessToken")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        let expires_at_ms = body.get("expiresAtMs").and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        });
        let short = crate::store::InferenceShort {
            access_token,
            grok_bot_token,
            expires_at_ms,
        };
        crate::store::save_inference_short(&short).await?;
        Ok(short)
    }

    pub async fn sand_access(&self, account: &Account) -> Result<Option<String>> {
        let (status, body) = self
            .send_json(
                account,
                "/aiserver.v1.DashboardService/GetSandAccessStatus",
                json!({}),
            )
            .await?;
        if !(200..300).contains(&status) {
            return Ok(None);
        }
        Ok(body
            .get("state")
            .and_then(|v| v.as_str())
            .map(str::to_owned))
    }

    pub async fn refresh(&self, account: &Account) -> Result<Account> {
        let refresh = account
            .refresh_token
            .as_deref()
            .ok_or_else(|| Error::Msg("account has no refresh token".into()))?;
        let response = self
            .req()?
            .post(format!("{SAND_BACKEND}/oauth/token"))
            .header("content-type", "application/json")
            .header("x-cursor-client-type", "sand")
            .json(&json!({
                "client_id": OAUTH_CLIENT_ID,
                "grant_type": "refresh_token",
                "refresh_token": refresh
            }))
            .send()
            .await?;
        let status = response.status().as_u16();
        let body: Value = response.json().await?;
        if status == 401 || status == 403 {
            return Err(Error::Msg("Cursor session expired; sign in again".into()));
        }
        if !(200..300).contains(&status) {
            return Err(Error::Msg(format!("oauth/token HTTP {status}: {body}")));
        }
        let access = body
            .get("access_token")
            .or_else(|| body.get("accessToken"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Msg("oauth/token missing access_token".into()))?;
        let refresh_token = body
            .get("refresh_token")
            .or_else(|| body.get("refreshToken"))
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .or_else(|| account.refresh_token.clone());
        let mut next = account.clone();
        next.access_token = access.to_owned();
        next.refresh_token = refresh_token;
        next.sand_state = self.sand_access(&next).await?;
        Ok(next)
    }

    pub async fn poll_login(&self, uuid: &str, verifier: &str) -> Result<Option<(String, String)>> {
        let url = format!("{SAND_BACKEND}/auth/poll?uuid={uuid}&verifier={verifier}");
        let response = self
            .req()?
            .get(url)
            .timeout(std::time::Duration::from_secs(15))
            .header("content-type", "application/json")
            .header("x-cursor-client-type", "sand")
            .send()
            .await?;
        let status = response.status().as_u16();
        if status == 404 {
            return Ok(None);
        }
        if status == 403 {
            return Err(Error::Msg("Cursor login was denied".into()));
        }
        if !(200..300).contains(&status) {
            return Err(Error::Msg(format!("login poll HTTP {status}")));
        }
        let body: Value = response.json().await?;
        let access = body
            .get("accessToken")
            .or_else(|| body.get("access_token"))
            .and_then(|v| v.as_str());
        let refresh = body
            .get("refreshToken")
            .or_else(|| body.get("refresh_token"))
            .and_then(|v| v.as_str());
        match (access, refresh) {
            (Some(access), Some(refresh)) => Ok(Some((access.to_owned(), refresh.to_owned()))),
            _ => Err(Error::Msg("login poll missing tokens".into())),
        }
    }

    pub async fn stream_chat(
        &self,
        account: &Account,
        model: &str,
        message: &str,
        effort: Option<&str>,
        fast: bool,
        tx: mpsc::Sender<ChatDelta>,
    ) -> Result<()> {
        self.stream_chat_knobs(account, model, message, effort, fast, None, tx)
            .await
    }

    pub async fn stream_chat_knobs(
        &self,
        account: &Account,
        model: &str,
        message: &str,
        effort: Option<&str>,
        fast: bool,
        context: Option<&str>,
        tx: mpsc::Sender<ChatDelta>,
    ) -> Result<()> {
        self.stream_chat_turns(
            account,
            model,
            &[crate::compact::ChatTurn::new("user", message)],
            effort,
            fast,
            context,
            tx,
        )
        .await
    }

    pub async fn stream_chat_turns(
        &self,
        account: &Account,
        model: &str,
        turns: &[crate::compact::ChatTurn],
        effort: Option<&str>,
        fast: bool,
        context: Option<&str>,
        tx: mpsc::Sender<ChatDelta>,
    ) -> Result<()> {
        self.stream_chat_turns_mode(
            account,
            model,
            turns,
            effort,
            fast,
            context,
            1,
            false,
            crate::providers::ToolFlags::default(),
            tx,
        )
        .await
    }

    pub async fn stream_chat_turns_mode(
        &self,
        account: &Account,
        model: &str,
        turns: &[crate::compact::ChatTurn],
        effort: Option<&str>,
        fast: bool,
        context: Option<&str>,
        mode: i32,
        is_subagent: bool,
        flags: crate::providers::ToolFlags,
        tx: mpsc::Sender<ChatDelta>,
    ) -> Result<()> {
        let framed = crate::inference::encode_inference_request_turns_mode(
            model,
            turns,
            effort,
            fast,
            context,
            mode,
            is_subagent,
            flags,
        );
        let loaded = crate::store::load_inference_short().await?;
        let short = match loaded {
            Some(short) if inference_short_usable(&short) => short,
            _ => self.renew_inference(account).await?,
        };
        let bearer = crate::store::grok_bot_bearer(&short)?;
        let sand = crate::sand::stream_headers(account, bearer, u128::from(now_ms()))?;
        // api2 first. Stale cursorvm box-stream returns HTTP 200 + ERROR_NOT_LOGGED_IN
        // and used to be tried first, so the real Stream never ran.
        let mut targets = vec![(inference_stream_url(None), None)];
        if let Some(box_stream) = crate::store::load_box_stream().await? {
            targets.push((
                inference_stream_url(Some(&box_stream)),
                Some(box_stream.network_token),
            ));
        }
        let mut last_http_error = None;
        let mut response = None;
        for (index, (url, network_token)) in targets.iter().enumerate() {
            let mut request = self.req()?.post(url);
            for (name, value) in &sand {
                if name == "content-type" {
                    continue;
                }
                request = request.header(name, value);
            }
            if let Some(token) = network_token {
                request = request
                    .header("x-anyrun-network-token", token)
                    .header(reqwest::header::EXPECT, "");
            }
            match request
                .header("content-type", "application/connect+proto")
                .header("connect-content-encoding", "identity")
                .body(framed.clone())
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    response = Some(resp);
                    break;
                }
                Ok(resp) => {
                    let code = resp.status().as_u16();
                    let snippet: String = resp
                        .text()
                        .await
                        .unwrap_or_default()
                        .chars()
                        .take(200)
                        .collect();
                    last_http_error =
                        Some(format!("InferenceService/Stream HTTP {code}: {snippet}"));
                    let can_fallback = index + 1 < targets.len() && should_fallback_from_box(code);
                    if !can_fallback {
                        return Err(Error::Msg(last_http_error.take().unwrap_or_else(|| {
                            format!("InferenceService/Stream HTTP {code}: {snippet}")
                        })));
                    }
                }
                Err(error) => {
                    last_http_error = Some(error.to_string());
                    if index + 1 >= targets.len() {
                        return Err(error.into());
                    }
                }
            }
        }
        let response = response.ok_or_else(|| {
            Error::Msg(last_http_error.unwrap_or_else(|| "InferenceService/Stream failed".into()))
        })?;
        let mut stream = response.bytes_stream();
        let mut buffered = Vec::new();
        let mut tool_id = String::new();
        let mut tool_name = String::new();
        let mut tool_args = String::new();
        while let Some(chunk) = stream.next().await {
            buffered.extend_from_slice(&chunk?);
            if buffered.len() > crate::connect::MAX_CONNECT_BUFFER {
                return Err(Error::Msg("Connect stream exceeded buffer cap".into()));
            }
            for payload in take_connect_frames(&mut buffered)? {
                let parsed = crate::inference::decode_inference_payload(&payload);
                let json = parse_connect_payload(&payload);
                if !parsed.tool_name.is_empty() || !parsed.tool_args.is_empty() {
                    if !parsed.tool_id.is_empty() {
                        tool_id = parsed.tool_id.clone();
                    }
                    if !parsed.tool_name.is_empty() {
                        tool_name = parsed.tool_name.clone();
                    }
                    tool_args.push_str(&parsed.tool_args);
                    if parsed.tool_complete {
                        if tx
                            .send(ChatDelta {
                                tools: vec![crate::connect::ChatToolCall {
                                    id: tool_id.clone(),
                                    name: tool_name.clone(),
                                    arguments: std::mem::take(&mut tool_args),
                                }],
                                ..ChatDelta::default()
                            })
                            .await
                            .is_err()
                        {
                            return Ok(());
                        }
                    }
                }
                let delta = ChatDelta {
                    thinking: if !parsed.thinking.is_empty() {
                        parsed.thinking
                    } else {
                        json.thinking
                    },
                    text: if !parsed.text.is_empty() {
                        parsed.text
                    } else {
                        json.text
                    },
                    error: parsed.error.or(json.error),
                    tools: Vec::new(),
                    prompt_tokens: parsed.prompt_tokens,
                    completion_tokens: parsed.completion_tokens,
                    reasoning_tokens: 0,
                };
                if tx.send(delta).await.is_err() {
                    return Ok(());
                }
            }
        }
        if !tool_name.is_empty() && !tool_args.is_empty() {
            let _ = tx
                .send(ChatDelta {
                    tools: vec![crate::connect::ChatToolCall {
                        id: tool_id,
                        name: tool_name,
                        arguments: tool_args,
                    }],
                    ..ChatDelta::default()
                })
                .await;
        }
        if !buffered.is_empty() {
            return Err(Error::Msg("truncated Connect envelope".into()));
        }
        Ok(())
    }

    pub async fn collect_inference(
        &self,
        account: &Account,
        model: &str,
        message: &str,
        effort: Option<&str>,
        fast: bool,
    ) -> Result<(String, String, Vec<crate::connect::ChatToolCall>)> {
        collect_inference_from(|tx| self.stream_chat(account, model, message, effort, fast, tx))
            .await
    }

    pub async fn stream_provider(
        &self,
        provider: &crate::providers::Provider,
        model: &str,
        turns: &[crate::compact::ChatTurn],
        tx: mpsc::Sender<ChatDelta>,
    ) -> Result<()> {
        self.stream_provider_mode(
            provider,
            model,
            turns,
            1,
            false,
            &[],
            crate::providers::ToolFlags::default(),
            tx,
        )
        .await
    }

    pub async fn stream_provider_mode(
        &self,
        provider: &crate::providers::Provider,
        model: &str,
        turns: &[crate::compact::ChatTurn],
        mode: i32,
        is_subagent: bool,
        images: &[(String, String)],
        flags: crate::providers::ToolFlags,
        tx: mpsc::Sender<ChatDelta>,
    ) -> Result<()> {
        let mut provider = provider.clone();
        self.ensure_provider_oauth(&mut provider).await?;
        let (url, headers, body) = crate::providers::provider_chat_request_messages_mode(
            &provider,
            model,
            turns,
            mode,
            is_subagent,
            images,
            flags,
        );
        let mut request = self
            .req()?
            .post(url)
            .timeout(std::time::Duration::from_secs(300));
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let response = request.json(&body).send().await?;
        let status = response.status();
        if !status.is_success() {
            let snippet: String = response
                .text()
                .await
                .unwrap_or_default()
                .chars()
                .take(240)
                .collect();
            return Err(Error::Msg(format!(
                "provider HTTP {}: {snippet}",
                status.as_u16()
            )));
        }
        let mut stream = response.bytes_stream();
        let mut leftover = String::new();
        let mut tool_acc: std::collections::BTreeMap<usize, (String, String, String)> =
            std::collections::BTreeMap::new();
        while let Some(chunk) = stream.next().await {
            leftover.push_str(&String::from_utf8_lossy(&chunk?));
            if leftover.len() > crate::connect::MAX_CONNECT_BUFFER {
                return Err(Error::Msg("provider stream exceeded buffer cap".into()));
            }
            while let Some(pos) = leftover.find('\n') {
                let line = leftover[..pos].trim().to_owned();
                leftover = leftover[pos + 1..].to_owned();
                let data = line
                    .strip_prefix("data:")
                    .map(str::trim)
                    .unwrap_or(line.as_str());
                for (index, id, name, arguments) in crate::providers::parse_openai_tool_deltas(data)
                {
                    let entry = tool_acc.entry(index).or_default();
                    if !id.is_empty() {
                        entry.0 = id;
                    }
                    if !name.is_empty() {
                        entry.1 = name;
                    }
                    entry.2.push_str(&arguments);
                }
                if let Some((prompt, completion, reasoning)) =
                    crate::providers::parse_provider_sse_usage(data)
                {
                    if tx
                        .send(ChatDelta {
                            prompt_tokens: prompt,
                            completion_tokens: completion,
                            reasoning_tokens: reasoning,
                            ..ChatDelta::default()
                        })
                        .await
                        .is_err()
                    {
                        return Ok(());
                    }
                }
                if let Some(thinking) = crate::providers::parse_provider_sse_thinking(
                    &provider.kind,
                    provider.wire,
                    data,
                ) {
                    if tx
                        .send(ChatDelta {
                            thinking,
                            ..ChatDelta::default()
                        })
                        .await
                        .is_err()
                    {
                        return Ok(());
                    }
                }
                if let Some(text) =
                    crate::providers::parse_provider_sse_data(&provider.kind, provider.wire, data)
                {
                    if tx
                        .send(ChatDelta {
                            text,
                            ..ChatDelta::default()
                        })
                        .await
                        .is_err()
                    {
                        return Ok(());
                    }
                }
            }
        }
        if !leftover.trim().is_empty() {
            for (index, id, name, arguments) in
                crate::providers::parse_openai_tool_deltas(leftover.trim())
            {
                let entry = tool_acc.entry(index).or_default();
                if !id.is_empty() {
                    entry.0 = id;
                }
                if !name.is_empty() {
                    entry.1 = name;
                }
                entry.2.push_str(&arguments);
            }
            if let Some(thinking) = crate::providers::parse_provider_sse_thinking(
                &provider.kind,
                provider.wire,
                leftover.trim(),
            ) {
                let _ = tx
                    .send(ChatDelta {
                        thinking,
                        ..ChatDelta::default()
                    })
                    .await;
            }
            if let Some(text) = crate::providers::parse_provider_sse_data(
                &provider.kind,
                provider.wire,
                leftover.trim(),
            ) {
                let _ = tx
                    .send(ChatDelta {
                        text,
                        ..ChatDelta::default()
                    })
                    .await;
            }
        }
        let tools: Vec<crate::connect::ChatToolCall> = tool_acc
            .into_iter()
            .filter(|(_, (_id, name, _))| !name.is_empty())
            .map(|(_, (id, name, arguments))| crate::connect::ChatToolCall {
                id,
                name,
                arguments,
            })
            .collect();
        if !tools.is_empty() {
            let _ = tx
                .send(ChatDelta {
                    tools,
                    ..ChatDelta::default()
                })
                .await;
        }
        Ok(())
    }

    pub async fn list_provider_models(
        &self,
        provider: &crate::providers::Provider,
    ) -> Result<Vec<String>> {
        let url = crate::providers::models_list_url(&provider.base_url);
        let token = crate::providers::bearer_token(provider);
        let mut request = self
            .req()?
            .get(url)
            .timeout(std::time::Duration::from_secs(30));
        if provider.kind == crate::providers::ProviderKind::Anthropic {
            request = request
                .header("x-api-key", token)
                .header("anthropic-version", "2023-06-01");
        } else {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = request.send().await?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(Error::Msg(format!(
                "models HTTP {}: {}",
                status.as_u16(),
                text.chars().take(180).collect::<String>()
            )));
        }
        Ok(crate::providers::parse_model_catalog(&text))
    }

    pub async fn ping_provider(&self, provider: &crate::providers::Provider) -> Result<(u64, u16)> {
        let url = crate::providers::models_list_url(&provider.base_url);
        let token = crate::providers::bearer_token(provider);
        let started = std::time::Instant::now();
        let mut request = self
            .req()?
            .get(url)
            .timeout(std::time::Duration::from_secs(8));
        if provider.kind == crate::providers::ProviderKind::Anthropic {
            request = request
                .header("x-api-key", token)
                .header("anthropic-version", "2023-06-01");
        } else if !token.is_empty() {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = request.send().await?;
        let status = response.status().as_u16();
        let ms = started.elapsed().as_millis() as u64;
        Ok((ms, status))
    }

    pub async fn ensure_provider_oauth(
        &self,
        provider: &mut crate::providers::Provider,
    ) -> Result<bool> {
        if provider.auth_mode != crate::providers::AuthMode::Oauth {
            return Ok(false);
        }
        let token = crate::providers::bearer_token(provider);
        if !crate::providers::bearer_expired(&token) {
            return Ok(false);
        }
        if provider.oauth_refresh.is_empty() {
            return Err(Error::Msg(
                "xAI 访问令牌约 6 小时过期，本地没有 refresh。请打开 Grok-Bot-Auth → 供应商，重新授权一次，之后会自动续。".into(),
            ));
        }
        let (access, refresh) = self
            .xai_oauth_refresh(&provider.oauth_refresh)
            .await
            .map_err(|error| {
                Error::Msg(format!("xAI 自动续期失败：{error}。请在供应商里重新授权。"))
            })?;
        provider.oauth_token = access.clone();
        provider.api_key = access;
        if let Some(refresh) = refresh {
            provider.oauth_refresh = refresh;
        }
        if let Ok(mut list) = crate::store::load_providers().await {
            if let Some(slot) = list.iter_mut().find(|item| item.id == provider.id) {
                slot.oauth_token = provider.oauth_token.clone();
                slot.api_key = provider.api_key.clone();
                slot.oauth_refresh = provider.oauth_refresh.clone();
                let _ = crate::store::save_providers(&list).await;
            }
        }
        Ok(true)
    }

    pub async fn xai_oauth_begin(&self) -> Result<(String, String, String)> {
        let response = self
            .req()?
            .post(crate::providers::XAI_DEVICE_CODE_URL)
            .header("content-type", "application/x-www-form-urlencoded")
            .header("accept", "application/json")
            .body(crate::providers::xai_device_begin_body())
            .send()
            .await?;
        let text = response.text().await.unwrap_or_default();
        crate::providers::parse_xai_device_begin(&text).map_err(Error::Msg)
    }

    pub async fn xai_oauth_poll(
        &self,
        device_code: &str,
    ) -> Result<Option<(String, Option<String>)>> {
        let response = self
            .req()?
            .post(crate::providers::XAI_TOKEN_URL)
            .header("content-type", "application/x-www-form-urlencoded")
            .header("accept", "application/json")
            .body(crate::providers::xai_device_poll_body(device_code))
            .send()
            .await?;
        let text = response.text().await.unwrap_or_default();
        if let Ok(tokens) = crate::providers::parse_xai_token_json(&text) {
            return Ok(Some(tokens));
        }
        let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        match value.get("error").and_then(|v| v.as_str()).unwrap_or("") {
            "authorization_pending" | "slow_down" | "" => Ok(None),
            other => Err(Error::Msg(other.to_owned())),
        }
    }

    pub async fn xai_oauth_refresh(&self, refresh: &str) -> Result<(String, Option<String>)> {
        let response = self
            .req()?
            .post(crate::providers::XAI_TOKEN_URL)
            .header("content-type", "application/x-www-form-urlencoded")
            .header("accept", "application/json")
            .body(crate::providers::xai_refresh_body(refresh))
            .send()
            .await?;
        let text = response.text().await.unwrap_or_default();
        crate::providers::parse_xai_token_json(&text).map_err(Error::Msg)
    }
}

pub fn inference_stream_url(box_stream: Option<&crate::store::BoxStream>) -> String {
    match box_stream {
        Some(cfg) if !cfg.base_url.is_empty() => format!(
            "{}/aiserver.v1.InferenceService/Stream",
            cfg.base_url.trim_end_matches('/')
        ),
        _ => format!("{SAND_BACKEND}/aiserver.v1.InferenceService/Stream"),
    }
}

fn inference_short_usable(short: &crate::store::InferenceShort) -> bool {
    if crate::store::grok_bot_bearer(short).is_err() {
        return false;
    }
    if !crate::sand::is_grok_bot_jwt(&short.grok_bot_token) {
        return false;
    }
    if let Some(exp) = crate::sand::jwt_exp(&short.grok_bot_token) {
        return exp.saturating_mul(1000) > now_ms() + 15_000;
    }
    short
        .expires_at_ms
        .map(|exp| exp > now_ms() + 15_000)
        .unwrap_or(false)
}

pub fn should_fallback_from_box(status: u16) -> bool {
    matches!(status, 401 | 403 | 417 | 502 | 503 | 504)
}

/// Drain deltas while the producer runs so a bounded(32) channel cannot
/// deadlock local RunSSE on streams with more than 32 frames.
pub async fn collect_inference_from<F, Fut>(
    start: F,
) -> Result<(String, String, Vec<crate::connect::ChatToolCall>)>
where
    F: FnOnce(mpsc::Sender<ChatDelta>) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    let (thinking, text, tools, _usage) = stream_inference_from(start, None).await?;
    Ok((thinking, text, tools))
}

pub async fn stream_inference_from<F, Fut>(
    start: F,
    live: Option<tokio::sync::mpsc::UnboundedSender<crate::agent_loop::LlmChunk>>,
) -> Result<(
    String,
    String,
    Vec<crate::connect::ChatToolCall>,
    crate::agent_loop::TokenUsage,
)>
where
    F: FnOnce(mpsc::Sender<ChatDelta>) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    let (tx, mut rx) = mpsc::channel(32);
    let produce = start(tx);
    let consume = async {
        let mut thinking = String::new();
        let mut text = String::new();
        let mut tools = Vec::new();
        let mut usage = crate::agent_loop::TokenUsage::default();
        while let Some(delta) = rx.recv().await {
            if let Some(error) = delta.error {
                return Err(Error::Msg(error));
            }
            if delta.prompt_tokens > 0 {
                usage.input = i64::from(delta.prompt_tokens);
            }
            if delta.completion_tokens > 0 {
                usage.output = i64::from(delta.completion_tokens);
            }
            if delta.reasoning_tokens > 0 {
                usage.reasoning = i64::from(delta.reasoning_tokens);
            }
            if !delta.thinking.is_empty() {
                thinking.push_str(&delta.thinking);
                if let Some(live) = live.as_ref() {
                    let _ = live.send(crate::agent_loop::LlmChunk::Thinking(
                        delta.thinking.clone(),
                    ));
                }
            }
            if !delta.text.is_empty() {
                text.push_str(&delta.text);
                if let Some(live) = live.as_ref() {
                    if !text.contains("<tool") && !text.contains("```tool_call") {
                        let _ = live.send(crate::agent_loop::LlmChunk::Text(delta.text.clone()));
                    }
                }
            }
            if !delta.tools.is_empty() {
                if let Some(live) = live.as_ref() {
                    for tool in &delta.tools {
                        let _ = live.send(crate::agent_loop::LlmChunk::ToolStart {
                            id: tool.id.clone(),
                            name: tool.name.clone(),
                        });
                    }
                }
                tools.extend(delta.tools);
            }
        }
        Ok((thinking, text, tools, usage))
    };
    match tokio::join!(produce, consume) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(quad)) => Ok(quad),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn collect_inference_from_pumps_forty_deltas() {
        let result = collect_inference_from(|tx| async move {
            for index in 0..40 {
                tx.send(ChatDelta {
                    text: format!("d{index};"),
                    ..ChatDelta::default()
                })
                .await
                .map_err(|_| Error::Msg("send closed".into()))?;
            }
            Ok(())
        })
        .await
        .expect("forty deltas");
        assert!(result.0.is_empty());
        assert!(result.1.starts_with("d0;"));
        assert!(result.1.contains("d32;"), "must not stall at the bound");
        assert!(result.1.ends_with("d39;"));
        assert_eq!(result.1.matches(';').count(), 40);
        assert!(result.2.is_empty());
    }

    #[tokio::test]
    async fn stream_inference_from_forwards_thinking_live() {
        let (live_tx, mut live_rx) = tokio::sync::mpsc::unbounded_channel();
        let result = stream_inference_from(
            |tx| async move {
                tx.send(ChatDelta {
                    thinking: "plan".into(),
                    prompt_tokens: 40,
                    completion_tokens: 2,
                    ..ChatDelta::default()
                })
                .await
                .map_err(|_| Error::Msg("send closed".into()))?;
                tx.send(ChatDelta {
                    text: "pong".into(),
                    ..ChatDelta::default()
                })
                .await
                .map_err(|_| Error::Msg("send closed".into()))?;
                Ok(())
            },
            Some(live_tx),
        )
        .await
        .expect("stream");
        assert_eq!(result.0, "plan");
        assert_eq!(result.1, "pong");
        assert_eq!(result.3.input, 40);
        assert_eq!(result.3.output, 2);
        let first = live_rx.recv().await.expect("thinking chunk");
        match first {
            crate::agent_loop::LlmChunk::Thinking(text) => assert_eq!(text, "plan"),
            other => panic!("expected thinking, got {other:?}"),
        }
    }

    #[test]
    fn inference_stream_url_prefers_box_when_configured() {
        let box_stream = crate::store::BoxStream {
            base_url: "https://pod-8765.example".into(),
            network_token: "n".into(),
        };
        assert_eq!(
            inference_stream_url(Some(&box_stream)),
            "https://pod-8765.example/aiserver.v1.InferenceService/Stream"
        );
        assert!(inference_stream_url(None).starts_with("https://api2.cursor.sh/"));
    }

    #[test]
    fn box_417_and_502_fall_back_to_api2() {
        assert!(should_fallback_from_box(417));
        assert!(should_fallback_from_box(502));
        assert!(should_fallback_from_box(401));
        assert!(!should_fallback_from_box(200));
    }
}
