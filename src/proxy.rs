use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use hudsucker::certificate_authority::RcgenAuthority;
use hudsucker::hyper::{Request, Response};
use hudsucker::rustls::crypto::aws_lc_rs;
use hudsucker::tokio_tungstenite::tungstenite::Message as WsMessage;
use hudsucker::{
    Body, HttpContext, HttpHandler, Proxy, RequestOrResponse, WebSocketContext, WebSocketHandler,
};
use prost::Message as ProstMessage;
use tokio::sync::{mpsc, oneshot, Mutex, Notify};
use tokio::time::{timeout, Duration};

use crate::agent_wire::{self, LocalRun};
use crate::agent_ws::{self, AgentWsRunIdentity, ClientWs};
use crate::ca;
use crate::catalog::{self, request_uses_injected_model};
use crate::client::SandClient;
use crate::error::{Error, Result};
use crate::sand::Account;

const PROXY_BIND: &str = "127.0.0.1:47822";

pub struct ProxyRuntime {
    pub url: Option<String>,
    stop: Option<oneshot::Sender<()>>,
}

impl ProxyRuntime {
    pub fn new() -> Self {
        Self {
            url: None,
            stop: None,
        }
    }

    pub fn running(&self) -> bool {
        self.url.is_some()
    }

    pub async fn start(
        &mut self,
        account: Arc<Mutex<Option<Account>>>,
        enabled: Arc<Mutex<Vec<String>>>,
        providers: Arc<Mutex<Vec<crate::providers::Provider>>>,
        client: SandClient,
    ) -> Result<String> {
        if let Some(url) = &self.url {
            return Ok(url.clone());
        }
        let (url, stop) = spawn_proxy(account, enabled, providers, client).await?;
        self.stop = Some(stop);
        self.url = Some(url.clone());
        Ok(url)
    }

    pub async fn stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.url = None;
    }
}

async fn spawn_proxy(
    account: Arc<Mutex<Option<Account>>>,
    enabled: Arc<Mutex<Vec<String>>>,
    providers: Arc<Mutex<Vec<crate::providers::Provider>>>,
    client: SandClient,
) -> Result<(String, oneshot::Sender<()>)> {
    let ca = ca::ensure_ca()?;
    let listener = tokio::net::TcpListener::bind(PROXY_BIND)
        .await
        .map_err(|error| Error::Msg(format!("bind {PROXY_BIND}: {error}")))?;
    let authority = RcgenAuthority::new(ca.issuer, 1_000, aws_lc_rs::default_provider());
    let (stop, done) = oneshot::channel::<()>();
    let handler = CursorHandler {
        account,
        enabled,
        providers,
        client,
        last_injected: Arc::new(Mutex::new(false)),
        local_runs: Arc::new(Mutex::new(HashMap::new())),
        queued_users: Arc::new(Mutex::new(HashMap::new())),
        exec_hub: crate::agent_session::ExecHub::new(),
        last_path: String::new(),
        ws_inject: Arc::new(Mutex::new(HashMap::new())),
        ws_hold: Arc::new(Mutex::new(HashMap::new())),
        ws_local: Arc::new(Mutex::new(HashSet::new())),
        ws_alias: Arc::new(Mutex::new(HashMap::new())),
        sse_waiters: Arc::new(Mutex::new(HashMap::new())),
    };
    let proxy = Proxy::builder()
        .with_listener(listener)
        .with_ca(authority)
        .with_rustls_connector(aws_lc_rs::default_provider())
        .with_http_handler(handler.clone())
        .with_websocket_handler(handler)
        .with_graceful_shutdown(async move {
            let _ = done.await;
        })
        .build()
        .map_err(|error| Error::Msg(format!("build proxy: {error}")))?;
    tokio::spawn(async move {
        if let Err(error) = proxy.start().await {
            eprintln!("grok-bot-auth proxy stopped: {error}");
        }
    });
    Ok((format!("http://{PROXY_BIND}"), stop))
}

fn is_cursor_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    matches!(host.as_str(), "api2.cursor.sh" | "api3.cursor.sh") || host.ends_with(".cursor.sh")
}

#[derive(Clone)]
struct CursorHandler {
    account: Arc<Mutex<Option<Account>>>,
    enabled: Arc<Mutex<Vec<String>>>,
    providers: Arc<Mutex<Vec<crate::providers::Provider>>>,
    client: SandClient,
    last_injected: Arc<Mutex<bool>>,
    local_runs: Arc<Mutex<HashMap<String, LocalRun>>>,
    queued_users: Arc<Mutex<HashMap<String, String>>>,
    exec_hub: crate::agent_session::ExecHub,
    last_path: String,
    ws_inject: Arc<Mutex<HashMap<SocketAddr, mpsc::UnboundedSender<WsMessage>>>>,
    ws_hold: Arc<Mutex<HashMap<SocketAddr, (AgentWsRunIdentity, Vec<u8>)>>>,
    ws_local: Arc<Mutex<HashSet<String>>>,
    ws_alias: Arc<Mutex<HashMap<String, String>>>,
    sse_waiters: Arc<Mutex<HashMap<String, SseWaiter>>>,
}

enum SseDecision {
    Local(LocalRun),
    Official,
}

struct SseWaiter {
    tx: Option<oneshot::Sender<SseDecision>>,
    started: Arc<Notify>,
}

impl CursorHandler {
    async fn notify_sse_local(&self, run: LocalRun) {
        let mut waiters = self.sse_waiters.lock().await;
        if let Some(waiter) = waiters.get_mut(&run.request_id) {
            if let Some(tx) = waiter.tx.take() {
                let _ = tx.send(SseDecision::Local(run));
            }
        }
    }

    async fn notify_sse_official(&self, request_id: &str) {
        let started = {
            let mut waiters = self.sse_waiters.lock().await;
            waiters.get_mut(request_id).and_then(|waiter| {
                waiter.tx.take().map(|tx| {
                    let started = waiter.started.clone();
                    let _ = tx.send(SseDecision::Official);
                    started
                })
            })
        };
        if let Some(started) = started {
            let _ = timeout(Duration::from_millis(800), started.notified()).await;
        }
    }
}

impl HttpHandler for CursorHandler {
    async fn handle_request(
        &mut self,
        _ctx: &HttpContext,
        request: Request<Body>,
    ) -> RequestOrResponse {
        let host = request
            .uri()
            .host()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let path = request.uri().path().to_owned();
        self.last_path = path.clone();
        if !is_cursor_host(&host) {
            return request.into();
        }
        if path.contains("/agent/v1/run")
            || path.contains("AvailableModels")
            || path.contains("BootstrapStatsig")
            || path.contains("BidiAppend")
            || path.contains("RunSSE")
            || path.contains("GetNewChatNudge")
            || path.contains("DashboardService")
            || path.contains("stripe_profile")
            || path.contains("GetMe")
        {
            log_proxy(&path);
        }
        // Never serve cached Statsig. CCursor leaves BootstrapStatsig on the official
        // path; a stale cache here emptied Plan & Usage when Grok-Bot-Auth was on.
        if path.contains("/agent/v1/run") {
            let method = request.method().as_str().to_owned();
            let upgrade = request
                .headers()
                .get("upgrade")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("-")
                .to_owned();
            log_proxy(&format!(
                "/agent/v1/run method={method} upgrade={upgrade} ws={}",
                is_websocket_upgrade(&request)
            ));
        }
        if path.contains("/agent/v1/run") && is_websocket_upgrade(&request) {
            log_proxy("ws-upgrade /agent/v1/run (frames inspected for gb-*)");
            return request.into();
        }
        let enabled = self.enabled.lock().await.clone();
        if enabled.is_empty() {
            return request.into();
        }
        if path.ends_with("/aiserver.v1.AiService/AvailableModels")
            || path == "/aiserver.v1.AiService/AvailableModels"
        {
            let mut request = request;
            if let Ok(uri) = "http://127.0.0.1:47821/relay/available-models".parse() {
                *request.uri_mut() = uri;
            }
            return request.into();
        }
        if path.ends_with("/agent.v1.AgentService/GetUsableModels")
            || path == "/agent.v1.AgentService/GetUsableModels"
            || path.ends_with("/aiserver.v1.AiService/GetUsableModels")
            || path == "/aiserver.v1.AiService/GetUsableModels"
        {
            let mut request = request;
            if let Ok(value) = path.parse() {
                request.headers_mut().insert("x-grok-upstream-path", value);
            }
            if let Ok(uri) = "http://127.0.0.1:47821/relay/usable-models".parse() {
                *request.uri_mut() = uri;
            }
            return request.into();
        }
        if path.contains("GetNewChatNudgeParameterizedModelPicker") {
            let (parts, body) = request.into_parts();
            let Ok(collected) = body.collect().await else {
                return Request::from_parts(parts, Body::empty()).into();
            };
            let bytes = collected.to_bytes();
            let hit = request_uses_injected_model(&bytes, &enabled);
            *self.last_injected.lock().await = hit;
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(crate::store::data_dir().join("proxy.log"))
                .and_then(|mut file| {
                    use std::io::Write;
                    writeln!(file, "nudge hit={hit}")
                });
            if hit {
                // Official catalog does not know gb-* ids. Empty nudge = allow current model.
                let mut framed = crate::connect::encode_connect_frame(&[]);
                framed.extend_from_slice(&[2, 0, 0, 0, 0]);
                if let Ok(response) = Response::builder()
                    .status(200)
                    .header("content-type", "application/connect+proto")
                    .body(Body::from(framed))
                {
                    return response.into();
                }
            }
            return Request::from_parts(parts, Body::from(bytes)).into();
        }
        let is_bidi = path.ends_with("/aiserver.v1.BidiService/BidiAppend")
            || path == "/aiserver.v1.BidiService/BidiAppend"
            || path.ends_with("/agent.v1.AgentService/Run")
            || path == "/agent.v1.AgentService/Run";
        let is_run_sse = path.ends_with("/agent.v1.AgentService/RunSSE")
            || path == "/agent.v1.AgentService/RunSSE";
        let is_usage = path.contains("GetPromptContextUsage");
        let is_clone = path.contains("NotifyConversationClone");
        let is_signed = path.contains("GetSignedUrlForAttachedMedia");
        let is_name_agent = path.contains("NameAgent");
        let is_agent_run = path.contains("/agent/v1/run");
        if is_usage {
            let (parts, body) = request.into_parts();
            let Ok(collected) = body.collect().await else {
                return Request::from_parts(parts, Body::empty()).into();
            };
            let bytes = collected.to_bytes();
            let payload = crate::connect::unwrap_connect(&bytes).1;
            let conv = crate::agent_wire::GetPromptContextUsageRequest::decode(payload)
                .ok()
                .map(|req| req.conversation_id)
                .unwrap_or_default();
            if crate::blob::is_local_conversation(&conv) {
                let mut request = Request::from_parts(parts, Body::from(bytes));
                if let Ok(uri) =
                    "http://127.0.0.1:47821/agent.v1.AgentService/GetPromptContextUsage".parse()
                {
                    *request.uri_mut() = uri;
                }
                return request.into();
            }
            return Request::from_parts(parts, Body::from(bytes)).into();
        }
        if is_clone {
            let (parts, body) = request.into_parts();
            let Ok(collected) = body.collect().await else {
                return Request::from_parts(parts, Body::empty()).into();
            };
            let bytes = collected.to_bytes();
            let payload = crate::connect::unwrap_connect(&bytes).1;
            let req = crate::agent_wire::NotifyConversationCloneRequest::decode(payload)
                .unwrap_or_default();
            if crate::blob::is_local_conversation(&req.source_conversation_id)
                || crate::blob::is_local_conversation(&req.conversation_id)
            {
                crate::blob::register_clone(
                    &req.conversation_id,
                    &req.source_conversation_id,
                    &req.source_request_id,
                );
                let mut request = Request::from_parts(parts, Body::from(bytes));
                if let Ok(uri) =
                    "http://127.0.0.1:47821/agent.v1.AgentService/NotifyConversationClone".parse()
                {
                    *request.uri_mut() = uri;
                }
                return request.into();
            }
            return Request::from_parts(parts, Body::from(bytes)).into();
        }
        if is_signed {
            let (parts, body) = request.into_parts();
            let Ok(collected) = body.collect().await else {
                return Request::from_parts(parts, Body::empty()).into();
            };
            let bytes = collected.to_bytes();
            let payload = crate::connect::unwrap_connect(&bytes).1;
            let conv = crate::agent_wire::GetSignedUrlForAttachedMediaRequest::decode(payload)
                .ok()
                .map(|req| req.conversation_id)
                .unwrap_or_default();
            if crate::blob::is_local_conversation(&conv) {
                let mut request = Request::from_parts(parts, Body::from(bytes));
                if let Ok(uri) =
                    "http://127.0.0.1:47821/agent.v1.AgentService/GetSignedUrlForAttachedMedia"
                        .parse()
                {
                    *request.uri_mut() = uri;
                }
                return request.into();
            }
            return Request::from_parts(parts, Body::from(bytes)).into();
        }
        if is_name_agent {
            if *self.last_injected.lock().await {
                let mut request = request;
                if let Ok(uri) = "http://127.0.0.1:47821/agent.v1.AgentService/NameAgent".parse() {
                    *request.uri_mut() = uri;
                }
                return request.into();
            }
            return request.into();
        }
        if !(is_bidi || is_run_sse || is_agent_run) {
            return request.into();
        }
        let (parts, body) = request.into_parts();
        let Ok(collected) = body.collect().await else {
            return Request::from_parts(parts, Body::empty()).into();
        };
        let bytes = Bytes::from(crate::connect::prepare_cursor_body(&collected.to_bytes()));
        let hex_hit = request_uses_injected_model(&bytes, &enabled);
        let raw_gb = bytes.windows(3).any(|window| window == b"gb-");
        if is_bidi {
            let scanned = agent_wire::scan_gb_model(&bytes);
            let mut official_pair_id = None;
            if let Some(decoded) = agent_wire::decode_bidi_append(&bytes) {
                official_pair_id = Some(decoded.request_id.clone());
                if let Some(text) = decoded.queued_user.clone() {
                    self.queued_users
                        .lock()
                        .await
                        .insert(decoded.request_id.clone(), text);
                }
                if let Some(mut run) = decoded.run.clone() {
                    if agent_wire::is_injected_model(&run.model_id, &enabled)
                        || run.model_id.starts_with("gb-")
                    {
                        if run.user_text.is_empty() {
                            if let Some(text) =
                                self.queued_users.lock().await.remove(&decoded.request_id)
                            {
                                run.user_text = text;
                            }
                        }
                        self.local_runs
                            .lock()
                            .await
                            .insert(decoded.request_id.clone(), run.clone());
                        self.exec_hub.put_run(run.clone()).await;
                        *self.last_injected.lock().await = true;
                        log_proxy(&format!(
                            "BidiAppend local request_id={} model={} text_len={}",
                            decoded.request_id,
                            run.model_id,
                            run.user_text.len()
                        ));
                        self.notify_sse_local(run).await;
                        if let Some(response) = empty_proto() {
                            return response.into();
                        }
                    }
                } else if self.exec_hub.is_local(&decoded.request_id).await
                    || self
                        .local_runs
                        .lock()
                        .await
                        .contains_key(&decoded.request_id)
                {
                    if let Some(exec) = decoded.exec.clone() {
                        self.exec_hub
                            .push(
                                &decoded.request_id,
                                crate::agent_session::ClientEvt::Exec(exec),
                            )
                            .await;
                        log_proxy(&format!(
                            "BidiAppend exec local request_id={} id={}",
                            decoded.request_id,
                            decoded.exec.as_ref().map(|e| e.id).unwrap_or(0)
                        ));
                    } else if let Some((id, error)) = decoded.throw.clone() {
                        self.exec_hub
                            .push(
                                &decoded.request_id,
                                crate::agent_session::ClientEvt::Throw { id, error },
                            )
                            .await;
                    } else if decoded.cancel {
                        self.exec_hub
                            .push(&decoded.request_id, crate::agent_session::ClientEvt::Cancel)
                            .await;
                    } else if decoded.heartbeat {
                        self.exec_hub
                            .push(
                                &decoded.request_id,
                                crate::agent_session::ClientEvt::Heartbeat,
                            )
                            .await;
                    } else if let Some(id) = decoded.stream_close {
                        self.exec_hub
                            .push(
                                &decoded.request_id,
                                crate::agent_session::ClientEvt::StreamClose(id),
                            )
                            .await;
                    } else if let Some(kv) = decoded.kv.clone() {
                        self.exec_hub
                            .push(
                                &decoded.request_id,
                                crate::agent_session::ClientEvt::Kv(kv),
                            )
                            .await;
                    } else if let Some(interaction) = decoded.interaction.clone() {
                        self.exec_hub
                            .push(
                                &decoded.request_id,
                                crate::agent_session::ClientEvt::Interaction(interaction),
                            )
                            .await;
                    }
                    if let Some(response) = empty_proto() {
                        return response.into();
                    }
                }
            } else {
                log_proxy(&format!(
                    "BidiAppend decode-fail scanned={} len={}",
                    scanned.as_deref().unwrap_or("-"),
                    bytes.len()
                ));
                let _ = std::fs::write(crate::store::data_dir().join("last-bidi.bin"), &bytes);
            }
            if let Some(id) = official_pair_id {
                self.notify_sse_official(&id).await;
            }
        }
        if is_run_sse {
            if let Some(request_id) = agent_wire::decode_run_sse_id(&bytes) {
                let local = self.local_runs.lock().await.get(&request_id).cloned();
                if let Some(run) = local {
                    log_proxy(&format!(
                        "RunSSE local request_id={} model={}",
                        request_id, run.model_id
                    ));
                    let account = self.account.lock().await.clone();
                    let providers = self.providers.lock().await.clone();
                    return local_run_sse(
                        account,
                        providers,
                        self.client.clone(),
                        self.exec_hub.clone(),
                        run,
                    )
                    .await;
                }
                let (tx, rx) = oneshot::channel();
                let started = Arc::new(Notify::new());
                self.sse_waiters.lock().await.insert(
                    request_id.clone(),
                    SseWaiter {
                        tx: Some(tx),
                        started: started.clone(),
                    },
                );
                log_proxy(&format!(
                    "RunSSE waiting for BidiAppend request_id={request_id}"
                ));
                let decision = timeout(Duration::from_millis(2000), rx).await;
                self.sse_waiters.lock().await.remove(&request_id);
                match decision {
                    Ok(Ok(SseDecision::Local(run))) => {
                        log_proxy(&format!(
                            "RunSSE local request_id={} model={}",
                            request_id, run.model_id
                        ));
                        let account = self.account.lock().await.clone();
                        let providers = self.providers.lock().await.clone();
                        return local_run_sse(
                            account,
                            providers,
                            self.client.clone(),
                            self.exec_hub.clone(),
                            run,
                        )
                        .await;
                    }
                    Ok(Ok(SseDecision::Official)) => {
                        log_proxy(&format!(
                            "RunSSE forward official request_id={request_id} (paired)"
                        ));
                        started.notify_waiters();
                    }
                    _ => {
                        log_proxy(&format!(
                            "RunSSE forward official request_id={request_id} (no gb- bidi)"
                        ));
                        started.notify_waiters();
                    }
                }
            }
        }
        log_proxy(&format!(
            "{} hit={} raw_gb={} hex_hit={} (passthrough, no sand jwt) ctype={}",
            path,
            hex_hit,
            raw_gb,
            hex_hit,
            parts
                .headers
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("-")
        ));
        Request::from_parts(parts, Body::from(bytes)).into()
    }

    async fn handle_response(
        &mut self,
        _ctx: &HttpContext,
        response: Response<Body>,
    ) -> Response<Body> {
        if !self.last_path.contains("BootstrapStatsig") {
            return response;
        }
        let encoding = response
            .headers()
            .get("content-encoding")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("-")
            .to_owned();
        let response = if encoding != "-" {
            match hudsucker::decode_response(response) {
                Ok(decoded) => decoded,
                Err(error) => {
                    log_proxy(&format!("BootstrapStatsig gzip decode failed: {error}"));
                    if let Some((content_type, cached)) =
                        crate::store::load_bytes_cache("bootstrap-statsig")
                    {
                        return Response::builder()
                            .status(200)
                            .header("content-type", content_type)
                            .body(Body::from(cached))
                            .unwrap_or_else(|_| Response::new(Body::empty()));
                    }
                    return Response::builder()
                        .status(502)
                        .body(Body::from("statsig decode failed"))
                        .unwrap_or_else(|_| Response::new(Body::empty()));
                }
            }
        } else {
            response
        };
        let (parts, body) = response.into_parts();
        let Ok(collected) = body.collect().await else {
            return Response::from_parts(parts, Body::empty());
        };
        let bytes = collected.to_bytes();
        match crate::statsig::patch_bootstrap_body(&bytes) {
            Some(patched) => {
                log_proxy("BootstrapStatsig: nal_websocket_client=false (HTTP Agent fallback)");
                let content_type = parts
                    .headers
                    .get("content-type")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("application/proto");
                crate::store::save_bytes_cache("bootstrap-statsig", content_type, &patched);
                let mut parts = parts;
                parts.headers.remove("content-length");
                parts.headers.remove("transfer-encoding");
                parts.headers.remove("content-encoding");
                Response::from_parts(parts, Body::from(patched))
            }
            None => {
                log_proxy(&format!(
                    "BootstrapStatsig: patch skipped encoding={encoding} len={}",
                    bytes.len()
                ));
                if let Some((content_type, cached)) =
                    crate::store::load_bytes_cache("bootstrap-statsig")
                {
                    let mut parts = parts;
                    parts.headers.remove("content-length");
                    parts.headers.remove("transfer-encoding");
                    parts.headers.remove("content-encoding");
                    if let Ok(value) = content_type.parse() {
                        parts.headers.insert("content-type", value);
                    }
                    return Response::from_parts(parts, Body::from(cached));
                }
                Response::from_parts(parts, Body::from(bytes))
            }
        }
    }

    async fn should_intercept_connect(
        &mut self,
        _ctx: &HttpContext,
        request: &Request<Body>,
    ) -> bool {
        request
            .uri()
            .authority()
            .is_some_and(|authority| is_cursor_host(authority.host()))
    }

    async fn should_intercept_tls(
        &mut self,
        _ctx: &HttpContext,
        hello: hudsucker::rustls::server::ClientHello<'_>,
    ) -> bool {
        hello.server_name().is_some_and(is_cursor_host)
    }
}

impl WebSocketHandler for CursorHandler {
    fn handle_websocket(
        mut self,
        ctx: WebSocketContext,
        mut stream: impl futures_util::Stream<
                Item = std::result::Result<
                    WsMessage,
                    hudsucker::tokio_tungstenite::tungstenite::Error,
                >,
            > + Unpin
            + Send
            + 'static,
        mut sink: impl futures_util::Sink<WsMessage, Error = hudsucker::tokio_tungstenite::tungstenite::Error>
            + Unpin
            + Send
            + 'static,
    ) -> impl Future<Output = ()> + Send {
        async move {
            match ctx {
                WebSocketContext::ServerToClient { dst, .. } => {
                    let (tx, mut rx) = mpsc::unbounded_channel();
                    self.ws_inject.lock().await.insert(dst, tx);
                    loop {
                        tokio::select! {
                            inject = rx.recv() => {
                                let Some(message) = inject else { break };
                                if sink.send(message).await.is_err() {
                                    break;
                                }
                            }
                            incoming = stream.next() => {
                                let Some(Ok(message)) = incoming else { break };
                                if sink.send(message).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    self.ws_inject.lock().await.remove(&dst);
                }
                WebSocketContext::ClientToServer { src, dst, .. } => {
                    let agent_run = dst.path().contains("/agent/v1/run");
                    while let Some(incoming) = stream.next().await {
                        let Ok(message) = incoming else { break };
                        if !agent_run {
                            if sink.send(message).await.is_err() {
                                break;
                            }
                            continue;
                        }
                        match self.intercept_client_ws(src, message).await {
                            WsOut::Forward(message) => {
                                if sink.send(message).await.is_err() {
                                    break;
                                }
                            }
                            WsOut::ForwardMany(messages) => {
                                for message in messages {
                                    if sink.send(message).await.is_err() {
                                        return;
                                    }
                                }
                            }
                            WsOut::Drop => {}
                        }
                    }
                    self.ws_hold.lock().await.remove(&src);
                    self.ws_inject.lock().await.remove(&src);
                }
            }
        }
    }
}

enum WsOut {
    Forward(WsMessage),
    ForwardMany(Vec<WsMessage>),
    Drop,
}

impl CursorHandler {
    async fn intercept_client_ws(&mut self, src: SocketAddr, message: WsMessage) -> WsOut {
        let WsMessage::Binary(data) = &message else {
            return WsOut::Forward(message);
        };
        match agent_ws::decode_client_ws(data.as_ref()) {
            ClientWs::Hello | ClientWs::Other => WsOut::Forward(message),
            ClientWs::RunStart(identity, raw) => {
                self.ws_hold.lock().await.insert(src, (identity, raw));
                log_proxy("ws-frame hold run_start");
                WsOut::Drop
            }
            ClientWs::AgentMessage {
                run_id,
                run,
                exec,
                kv,
                interaction,
            } => {
                if exec.is_some() || kv.is_some() || interaction.is_some() {
                    let wait_key = self
                        .ws_alias
                        .lock()
                        .await
                        .get(&run_id)
                        .cloned()
                        .unwrap_or_else(|| {
                            if run_id.is_empty() {
                                src.to_string()
                            } else {
                                run_id.clone()
                            }
                        });
                    if self.exec_hub.is_local(&wait_key).await
                        || self.ws_local.lock().await.contains(&run_id)
                    {
                        if let Some(exec) = exec {
                            self.exec_hub
                                .push(&wait_key, crate::agent_session::ClientEvt::Exec(exec))
                                .await;
                        }
                        if let Some(kv) = kv {
                            self.exec_hub
                                .push(&wait_key, crate::agent_session::ClientEvt::Kv(kv))
                                .await;
                        }
                        if let Some(interaction) = interaction {
                            self.exec_hub
                                .push(
                                    &wait_key,
                                    crate::agent_session::ClientEvt::Interaction(interaction),
                                )
                                .await;
                        }
                        return WsOut::Drop;
                    }
                }
                let Some(run) = run else {
                    return self.release_held(src, message).await;
                };
                let enabled = self.enabled.lock().await.clone();
                if !(agent_wire::is_injected_model(&run.model_id, &enabled)
                    || run.model_id.starts_with("gb-"))
                {
                    return self.release_held(src, message).await;
                }
                let identity = self
                    .ws_hold
                    .lock()
                    .await
                    .remove(&src)
                    .map(|(identity, _)| identity)
                    .unwrap_or_else(|| AgentWsRunIdentity {
                        run_id: run_id.clone(),
                        request_id: run.request_id.clone(),
                    });
                let mut run = run;
                if run.request_id.is_empty() || run.request_id == "ws" {
                    run.request_id = identity.request_id.clone();
                    if run.request_id.is_empty() {
                        run.request_id = identity.run_id.clone();
                    }
                }
                self.ws_alias
                    .lock()
                    .await
                    .insert(identity.run_id.clone(), run.request_id.clone());
                self.ws_local.lock().await.insert(identity.run_id.clone());
                self.exec_hub.put_run(run.clone()).await;
                *self.last_injected.lock().await = true;
                log_proxy(&format!(
                    "ws-frame local run_id={} model={} text_len={}",
                    identity.run_id,
                    run.model_id,
                    run.user_text.len()
                ));
                self.spawn_local_ws(src, identity, run).await;
                WsOut::Drop
            }
            ClientWs::HalfClose(run_id) => {
                if self.ws_local.lock().await.contains(&run_id) {
                    WsOut::Drop
                } else {
                    self.release_held(src, message).await
                }
            }
            ClientWs::Cancel(run_id) => {
                if self.ws_local.lock().await.contains(&run_id) {
                    let wait_key = self
                        .ws_alias
                        .lock()
                        .await
                        .get(&run_id)
                        .cloned()
                        .unwrap_or(run_id);
                    self.exec_hub
                        .push(&wait_key, crate::agent_session::ClientEvt::Cancel)
                        .await;
                    WsOut::Drop
                } else {
                    self.release_held(src, message).await
                }
            }
        }
    }

    async fn release_held(&self, src: SocketAddr, message: WsMessage) -> WsOut {
        if let Some((_identity, raw)) = self.ws_hold.lock().await.remove(&src) {
            WsOut::ForwardMany(vec![WsMessage::Binary(Bytes::from(raw)), message])
        } else {
            WsOut::Forward(message)
        }
    }

    async fn spawn_local_ws(&self, src: SocketAddr, identity: AgentWsRunIdentity, run: LocalRun) {
        let mut tx = self.ws_inject.lock().await.get(&src).cloned();
        if tx.is_none() {
            for _ in 0..50 {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                tx = self.ws_inject.lock().await.get(&src).cloned();
                if tx.is_some() {
                    break;
                }
            }
        }
        let Some(tx) = tx else {
            log_proxy("ws-frame local missing inject channel");
            return;
        };
        let account = self.account.lock().await.clone();
        let providers = self.providers.lock().await.clone();
        let client = self.client.clone();
        let hub = self.exec_hub.clone();
        let ws_local = self.ws_local.clone();
        let ws_alias = self.ws_alias.clone();
        let alias_run = identity.run_id.clone();
        tokio::spawn(async move {
            if run.user_text.trim().is_empty() {
                for frame in agent_ws::encode_local_err(&identity, "grok-bot-auth: empty user text")
                {
                    if tx.send(WsMessage::Binary(Bytes::from(frame))).is_err() {
                        break;
                    }
                }
                return;
            }
            if tx
                .send(WsMessage::Binary(Bytes::from(
                    agent_ws::encode_run_accepted(&identity),
                )))
                .is_err()
            {
                return;
            }
            let request_id = run.request_id.clone();
            let jobs = crate::agent_session::spawn_wait_bridge(
                hub.clone(),
                request_id.clone(),
                run.cancel.clone(),
            );
            let seq = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
            let emit_tx = tx.clone();
            let run_id = identity.run_id.clone();
            let jobs_ix = jobs.clone();
            let result = stream_injected_run(
                account,
                providers,
                client,
                run,
                move |id| {
                    let jobs = jobs.clone();
                    async move { crate::agent_session::wait_via_bridge(&jobs, id).await }
                },
                move |id| {
                    let jobs = jobs_ix.clone();
                    async move { crate::agent_session::wait_ix_via_bridge(&jobs, id).await }
                },
                move |msg| {
                    let n = seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let frame = agent_ws::encode_server_payload(
                        &run_id,
                        n,
                        ProstMessage::encode_to_vec(&msg),
                    );
                    let _ = emit_tx.send(WsMessage::Binary(Bytes::from(frame)));
                },
            )
            .await;
            let end = match result {
                Ok(()) => agent_ws::encode_run_end(&identity, None),
                Err(error) => agent_ws::encode_run_end(&identity, Some((&error, 14))),
            };
            let _ = tx.send(WsMessage::Binary(Bytes::from(end)));
            hub.finish(&request_id).await;
            ws_local.lock().await.remove(&alias_run);
            ws_alias.lock().await.remove(&alias_run);
        });
    }
}

pub fn proxy_url() -> &'static str {
    "http://127.0.0.1:47822"
}

fn is_websocket_upgrade(request: &Request<Body>) -> bool {
    request
        .headers()
        .get("upgrade")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
        || request
            .headers()
            .get("connection")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.to_ascii_lowercase().contains("upgrade"))
}

fn log_proxy(line: &str) {
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(crate::store::data_dir().join("proxy.log"))
        .and_then(|mut file| {
            use std::io::Write;
            writeln!(file, "{line}")
        });
}

fn empty_proto() -> Option<Response<Body>> {
    Response::builder()
        .status(200)
        .header("content-type", "application/proto")
        .body(Body::from(Vec::<u8>::new()))
        .ok()
}

fn sse_response(body: Vec<u8>) -> RequestOrResponse {
    Response::builder()
        .status(200)
        .header("content-type", "application/connect+proto")
        .header("cache-control", "no-cache")
        .header("connect-protocol-version", "1")
        .body(Body::from(body))
        .expect("static SSE headers")
        .into()
}

fn sse_stream(rx: mpsc::UnboundedReceiver<Bytes>) -> RequestOrResponse {
    let stream =
        tokio_stream::wrappers::UnboundedReceiverStream::new(rx).map(Ok::<Bytes, hudsucker::Error>);
    Response::builder()
        .status(200)
        .header("content-type", "application/connect+proto")
        .header("cache-control", "no-cache")
        .header("connect-protocol-version", "1")
        .body(Body::from_stream(stream))
        .expect("stream SSE headers")
        .into()
}

pub(crate) async fn infer_model_turn(
    account: &Option<Account>,
    providers: &[crate::providers::Provider],
    client: &SandClient,
    run: &LocalRun,
    prompt: &str,
    live: tokio::sync::mpsc::UnboundedSender<crate::agent_loop::LlmChunk>,
) -> std::result::Result<crate::agent_loop::ModelTurn, String> {
    if let Some((provider, model)) = catalog::resolve_provider_run(providers, &run.model_id) {
        let turns = crate::agent_loop::outbound_turns_for(run, prompt);
        let model = if model.contains('[') {
            model
        } else if let Some(effort) = run.effort.as_deref() {
            format!(
                "{model}[effort={effort},fast={}]",
                if run.fast { "true" } else { "false" }
            )
        } else {
            model
        };
        let (think, body, tools, usage) = crate::client::stream_inference_from(
            |tx| {
                client.stream_provider_mode(
                    provider,
                    &model,
                    &turns,
                    run.mode,
                    run.subagent_type_name.is_some(),
                    &run.images,
                    run.tool_flags(),
                    tx,
                )
            },
            Some(live),
        )
        .await
        .map_err(|error| format!("{}: {error}", provider.label))?;
        let mut turn = crate::agent_loop::parse_model_turn(think, body, tools);
        turn.usage = usage;
        return Ok(turn);
    }
    let Some(account) = account.as_ref() else {
        return Err("grok-bot-auth: sign in first".into());
    };
    let turns = crate::agent_loop::outbound_turns_for(run, prompt);
    let (think, body, tools, usage) = crate::client::stream_inference_from(
        |tx| {
            client.stream_chat_turns_mode(
                account,
                &run.model_id,
                &turns,
                run.effort.as_deref(),
                run.fast,
                None,
                run.mode,
                run.subagent_type_name.is_some(),
                run.tool_flags(),
                tx,
            )
        },
        Some(live),
    )
    .await
    .map_err(|error| format!("InferenceService/Stream: {error}"))?;
    let mut turn = crate::agent_loop::parse_model_turn(think, body, tools);
    turn.usage = usage;
    Ok(turn)
}

pub(crate) async fn stream_injected_run<W, Wf, I, If, E>(
    account: Option<Account>,
    providers: Vec<crate::providers::Provider>,
    client: SandClient,
    run: LocalRun,
    wait_exec: W,
    wait_ix: I,
    emit: E,
) -> std::result::Result<(), String>
where
    W: FnMut(u32) -> Wf,
    Wf: std::future::Future<Output = Option<crate::agent_proto::ExecClientMessage>>,
    I: FnMut(u32) -> If,
    If: std::future::Future<Output = Option<crate::agent_wire::InteractionResponse>>,
    E: FnMut(crate::agent_wire::AgentServerMessage),
{
    let started = std::time::Instant::now();
    let mut run = run;
    run.max_tokens = catalog::context_tokens_for_model(&run.model_id, &providers, run.max_tokens);
    let source = if catalog::resolve_provider_run(&providers, &run.model_id).is_some() {
        "provider"
    } else {
        "cursor"
    };
    let llm_account = account.clone();
    let llm_providers = providers.clone();
    let llm_client = client.clone();
    let llm_run = run.clone();
    let result = crate::agent_loop::run_injected_agent(
        &run,
        move |prompt: String, live| {
            let account = llm_account.clone();
            let providers = llm_providers.clone();
            let client = llm_client.clone();
            let run = llm_run.clone();
            async move {
                infer_model_turn(&account, &providers, &client, &run, &prompt, live).await
            }
        },
        wait_exec,
        wait_ix,
        emit,
    )
    .await;
    let _ = crate::usage::append(&crate::usage::UsageRow {
        ts_ms: crate::usage::now_ms(),
        source: source.into(),
        model: run.model_id.clone(),
        latency_ms: started.elapsed().as_millis() as u64,
        tokens: 0,
        ok: result.is_ok(),
    });
    result
}

async fn local_run_sse(
    account: Option<Account>,
    providers: Vec<crate::providers::Provider>,
    client: SandClient,
    hub: crate::agent_session::ExecHub,
    run: LocalRun,
) -> RequestOrResponse {
    if run.user_text.trim().is_empty() && run.workspace.is_none() {
        return sse_response(agent_wire::encode_end_stream_error(
            "invalid_argument",
            "grok-bot-auth: BidiAppend had no user text",
        ));
    }
    let (tx, rx) = mpsc::unbounded_channel::<Bytes>();
    tokio::spawn(async move {
        let request_id = run.request_id.clone();
        let jobs = crate::agent_session::spawn_wait_bridge(
            hub.clone(),
            request_id.clone(),
            run.cancel.clone(),
        );
        let emit_tx = tx.clone();
        let jobs_ix = jobs.clone();
        let result = stream_injected_run(
            account,
            providers,
            client,
            run,
            move |id| {
                let jobs = jobs.clone();
                async move { crate::agent_session::wait_via_bridge(&jobs, id).await }
            },
            move |id| {
                let jobs = jobs_ix.clone();
                async move { crate::agent_session::wait_ix_via_bridge(&jobs, id).await }
            },
            move |msg| {
                let _ = emit_tx.send(Bytes::from(agent_wire::encode_server(&msg)));
            },
        )
        .await;
        match result {
            Ok(()) => {
                let _ = tx.send(Bytes::from(agent_wire::encode_end_stream_ok()));
            }
            Err(error) => {
                let code = if error.contains("sign in") {
                    "unauthenticated"
                } else {
                    "unavailable"
                };
                let _ = tx.send(Bytes::from(agent_wire::encode_end_stream_error(
                    code, &error,
                )));
            }
        }
        hub.finish(&request_id).await;
    });
    sse_stream(rx)
}
