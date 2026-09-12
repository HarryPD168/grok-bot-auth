use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderName, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use futures_util::StreamExt;
use prost::Message;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, Mutex, Notify};
use tokio_stream::wrappers::{ReceiverStream, UnboundedReceiverStream};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

use crate::accounts::{AccountPool, PendingOauth, PoolMode};
use crate::catalog;
use crate::client::SandClient;
use crate::connect::ChatDelta;
use crate::error::{Error, Result};
use crate::proxy::ProxyRuntime;
use crate::sand::{
    account_id, begin_login, fill_account_identity, first_available_id, grok_bot_inference_allowed,
    import_account, parse_sand_families, present_catalog, public_account, Account, ImportBody,
    LoginSession, SandFamily,
};
use crate::{settings, store};

#[derive(Clone)]
pub struct AppState {
    pub client: SandClient,
    pub account: Arc<Mutex<Option<Account>>>,
    pub grok_pool: Arc<Mutex<AccountPool<Account>>>,
    pub pending_oauth: Arc<Mutex<Option<PendingOauth>>>,
    pub oauth_cancel: Arc<Mutex<bool>>,
    pub oauth_kick: Arc<tokio::sync::Notify>,
    pub models: Arc<Mutex<Vec<SandFamily>>>,
    pub logins: Arc<Mutex<HashMap<String, LoginSession>>>,
    pub proxy: Arc<Mutex<ProxyRuntime>>,
    pub imported: Arc<Mutex<Vec<String>>>,
    pub enabled: Arc<Mutex<Vec<String>>>,
    pub coexist: Arc<Mutex<bool>>,
    pub preferred_model: Arc<Mutex<Option<String>>>,
    pub providers: Arc<Mutex<Vec<crate::providers::Provider>>>,
    pub prompt_pool: Arc<Mutex<Vec<String>>>,
    pub effort: Arc<Mutex<String>>,
    pub context: Arc<Mutex<String>>,
    pub compact_budget: Arc<Mutex<usize>>,
    pub local_runs: Arc<Mutex<HashMap<String, crate::agent_wire::LocalRun>>>,
    pub queued_users: Arc<Mutex<HashMap<String, VecDeque<crate::agent_wire::QueuedUser>>>>,
    pub exec_hub: crate::agent_session::ExecHub,
    pub sse_waiters: Arc<Mutex<HashMap<String, SseWaiter>>>,
}

enum SseDecision {
    Local(crate::agent_wire::LocalRun),
    Official,
}

pub struct SseWaiter {
    tx: Option<oneshot::Sender<SseDecision>>,
    started: Arc<Notify>,
}

impl AppState {
    pub fn new(
        client: SandClient,
        account: Option<Account>,
        prefs: crate::store::Prefs,
        grok_pool: AccountPool<Account>,
    ) -> Self {
        Self {
            client,
            account: Arc::new(Mutex::new(account)),
            grok_pool: Arc::new(Mutex::new(grok_pool)),
            pending_oauth: Arc::new(Mutex::new(None)),
            oauth_cancel: Arc::new(Mutex::new(false)),
            oauth_kick: Arc::new(tokio::sync::Notify::new()),
            models: Arc::new(Mutex::new(present_catalog(prefs.catalog.clone()))),
            logins: Arc::new(Mutex::new(HashMap::new())),
            proxy: Arc::new(Mutex::new(ProxyRuntime::new())),
            imported: Arc::new(Mutex::new(prefs.imported)),
            enabled: Arc::new(Mutex::new(crate::catalog::normalize_enabled(
                &prefs.catalog,
                prefs.enabled,
            ))),
            coexist: Arc::new(Mutex::new(prefs.coexist)),
            preferred_model: Arc::new(Mutex::new(prefs.preferred_model)),
            providers: Arc::new(Mutex::new(prefs.providers)),
            prompt_pool: Arc::new(Mutex::new(prefs.prompt_pool)),
            effort: Arc::new(Mutex::new(if prefs.effort.is_empty() {
                "high".into()
            } else {
                prefs.effort
            })),
            context: Arc::new(Mutex::new(prefs.context)),
            compact_budget: Arc::new(Mutex::new(if prefs.compact_budget == 0 {
                crate::compact::OFFICIAL_CHAR_BUDGET
            } else {
                prefs.compact_budget
            })),
            local_runs: Arc::new(Mutex::new(HashMap::new())),
            queued_users: Arc::new(Mutex::new(HashMap::new())),
            exec_hub: crate::agent_session::ExecHub::new(),
            sse_waiters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn commit_grok_account(&self, mut account: Account) -> Result<()> {
        fill_account_identity(&mut account);
        account.exhausted = false;
        account.last_error = None;
        {
            let mut pool = self.grok_pool.lock().await;
            pool.upsert(account.clone());
            store::save_grok_pool(&pool).await?;
        }
        *self.account.lock().await = Some(account);
        Ok(())
    }

    pub async fn pick_grok_account(&self) -> Option<Account> {
        let mut pool = self.grok_pool.lock().await;
        let picked = pool.pick().or_else(|| pool.recover_if_all_exhausted());
        let picked = match picked {
            Some(account) => account,
            None => {
                return self
                    .account
                    .lock()
                    .await
                    .clone()
                    .filter(|a| !a.access_token.is_empty())
            }
        };
        let _ = store::save_grok_pool(&pool).await;
        *self.account.lock().await = Some(picked.clone());
        Some(picked)
    }

    pub async fn set_grok_mode(&self, mode: PoolMode) -> Result<()> {
        let mut pool = self.grok_pool.lock().await;
        pool.mode = mode;
        store::save_grok_pool(&pool).await
    }

    pub async fn activate_grok_email(&self, email: &str) -> Result<()> {
        let mut pool = self.grok_pool.lock().await;
        if let Some(account) = pool.activate(email) {
            store::save_grok_pool(&pool).await?;
            *self.account.lock().await = Some(account);
        }
        Ok(())
    }

    pub async fn remove_grok_email(&self, email: &str) -> Result<()> {
        let mut pool = self.grok_pool.lock().await;
        pool.remove(email);
        store::save_grok_pool(&pool).await?;
        *self.account.lock().await = pool
            .slots
            .iter()
            .find(|slot| account_id(slot) == pool.active_id)
            .cloned();
        Ok(())
    }

    pub async fn mark_grok_quota(&self, email: &str, error: &str) -> Result<()> {
        let mut pool = self.grok_pool.lock().await;
        pool.mark_exhausted(email, Some(error.to_owned()));
        store::save_grok_pool(&pool).await
    }

    pub async fn refresh_grok_quotas(&self) -> Result<usize> {
        let slots = self.grok_pool.lock().await.slots.clone();
        let mut ok = 0usize;
        for mut slot in slots {
            match self.client.period_usage(&slot).await {
                Ok(quota) => {
                    slot.quota = Some(quota.clone());
                    if quota.grok_exhausted() {
                        slot.exhausted = true;
                        slot.last_error = Some("Grok Bot 周期额度用尽".into());
                    } else {
                        slot.exhausted = false;
                        slot.last_error = None;
                    }
                    let mut pool = self.grok_pool.lock().await;
                    pool.update_slot(slot);
                    ok += 1;
                }
                Err(error) => {
                    let mut pool = self.grok_pool.lock().await;
                    if let Some(existing) = pool
                        .slots
                        .iter_mut()
                        .find(|item| account_id(item) == account_id(&slot))
                    {
                        existing.last_error = Some(error.to_string());
                    }
                }
            }
        }
        let pool = self.grok_pool.lock().await.clone();
        store::save_grok_pool(&pool).await?;
        if let Some(active) = pool
            .slots
            .iter()
            .find(|slot| account_id(slot) == pool.active_id)
            .cloned()
        {
            *self.account.lock().await = Some(active);
        }
        Ok(ok)
    }

    pub async fn set_provider_pool_mode(&self, provider_id: &str, mode: PoolMode) -> Result<()> {
        {
            let mut list = self.providers.lock().await;
            if let Some(provider) = list.iter_mut().find(|item| item.id == provider_id) {
                provider.pool_mode = mode;
            }
        }
        self.persist_prefs().await
    }

    pub async fn refresh_all_quotas(&self) -> Result<usize> {
        let grok = self.refresh_grok_quotas().await.unwrap_or(0);
        let xai = self.refresh_xai_quotas().await.unwrap_or(0);
        Ok(grok + xai)
    }

    pub async fn refresh_xai_quotas(&self) -> Result<usize> {
        let mut n = 0usize;
        {
            let mut list = self.providers.lock().await;
            for provider in list.iter_mut() {
                if provider.kind != crate::providers::ProviderKind::Xai {
                    continue;
                }
                let token = crate::providers::bearer_token(provider);
                if token.is_empty() {
                    continue;
                }
                if provider.oauth_accounts.is_empty() {
                    crate::providers::upsert_provider_account(
                        provider,
                        crate::providers::provider_account_from_token(&token, &provider.label),
                    );
                }
                match self.client.xai_usage(&token).await {
                    Ok(quota) => {
                        for acc in &mut provider.oauth_accounts {
                            acc.quota = Some(quota.clone());
                            acc.exhausted =
                                quota.remaining_percent.map(|p| p <= 0.0).unwrap_or(false);
                            acc.last_error = None;
                        }
                        n += 1;
                    }
                    Err(error) => {
                        for acc in &mut provider.oauth_accounts {
                            acc.last_error = Some(error.to_string());
                        }
                    }
                }
            }
        }
        let snapshot = self.providers.lock().await.clone();
        store::save_providers(&snapshot).await?;
        self.persist_prefs().await?;
        Ok(n)
    }

    pub async fn persist_prefs(&self) -> Result<()> {
        let imported = self.imported.lock().await.clone();
        let enabled = self.enabled.lock().await.clone();
        let catalog = self.models.lock().await.clone();
        let coexist = *self.coexist.lock().await;
        let preferred_model = self.preferred_model.lock().await.clone();
        let providers = self.providers.lock().await.clone();
        let prompt_pool = self.prompt_pool.lock().await.clone();
        let compact_budget = *self.compact_budget.lock().await;
        let effort = self.effort.lock().await.clone();
        let context = self.context.lock().await.clone();
        let existing = store::load_prefs().await.unwrap_or_default();
        store::save_prefs(&store::Prefs {
            imported,
            enabled,
            catalog,
            coexist,
            preferred_model,
            providers: providers.clone(),
            prompt_pool,
            compact_budget,
            effort,
            context,
            mcp: existing.mcp,
            grok_maps: Vec::new(),
            api_port: existing.api_port,
            mitm_port: existing.mitm_port,
            outbound_proxy: existing.outbound_proxy,
        })
        .await?;
        store::save_providers(&providers).await
    }
}

pub async fn start_coexist(state: &AppState) -> Result<String> {
    if let Some(existing) = settings::current_proxy()? {
        if !existing.contains("47822")
            && (existing.contains("127.0.0.1") || existing.contains("localhost"))
        {
            return Err(Error::Msg(format!(
                "Cursor http.proxy 已是 {existing}，先停 cursor-byok"
            )));
        }
    }
    // Official account/plan must 直连. Whole-process MITM fails Windows
    // CERT_TRUST_REVOCATION_STATUS_UNKNOWN and empties Plan & Usage.
    settings::clear_proxy_settings()?;
    state.proxy.lock().await.stop().await;
    let auth_note = crate::cursor_ws::ensure_always_local_early_auth()
        .unwrap_or_else(|error| error.to_string());
    let redirect_note = crate::cursor_redirect::install()?;
    let gate_note =
        crate::cursor_ws::disable_agent_websocket().unwrap_or_else(|error| error.to_string());
    *state.coexist.lock().await = true;
    state.persist_prefs().await?;
    let families = inject_families(state).await;
    let picker_note = catalog::publish_injected_to_cursor_picker(&families)
        .unwrap_or_else(|error| error.to_string());
    Ok(format!(
        "共存已开：官方模型直连 api2；本软件未运行时 Cursor 不受影响。gb-* 走 {LOCAL}。{auth_note} {gate_note} {redirect_note} {picker_note}",
        LOCAL = crate::cursor_redirect::LOCAL
    ))
}

pub async fn stop_coexist(state: &AppState) -> Result<()> {
    park_cursor_mitm(state).await?;
    *state.coexist.lock().await = false;
    state.persist_prefs().await?;
    Ok(())
}

/// Drop Cursor MITM settings so official Cursor 直连. Keeps `coexist` pref so the
/// next Grok-Bot-Auth launch can plug 反代 back in. Does not touch Clash outbound.
pub async fn park_cursor_mitm(state: &AppState) -> Result<()> {
    settings::clear_proxy_settings()?;
    let _ = crate::cursor_redirect::restore();
    let _ = crate::cursor_ws::restore_agent_websocket();
    let _ = crate::cursor_ws::scrub_transport_leftovers();
    let _ = crate::catalog::unpublish_injected_from_cursor_picker();
    state.proxy.lock().await.stop().await;
    Ok(())
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/favicon.ico", get(|| async { StatusCode::NO_CONTENT }))
        .route("/app.css", get(css))
        .route("/app.js", get(js))
        .route("/api/status", get(status))
        .route("/api/import", post(import))
        .route("/api/logout", post(logout))
        .route("/api/login/start", post(login_start))
        .route("/api/login/poll", post(login_poll))
        .route("/api/refresh", post(refresh))
        .route("/api/models/sync", post(sync_models))
        .route("/api/chat", post(chat))
        .route("/api/cursor/enable", post(cursor_enable))
        .route("/api/cursor/disable", post(cursor_disable))
        .route("/relay/available-models", post(relay_available_models))
        .route("/relay/usable-models", post(relay_usable_models))
        .route("/relay/merge-catalog", post(relay_merge_catalog))
        .route("/relay/default-model", post(relay_default_model))
        .route("/relay/default-nudge", post(relay_default_nudge))
        .route("/relay/bootstrap-statsig", post(relay_bootstrap_statsig))
        .route(
            "/aiserver.v1.AiService/AvailableModels",
            post(relay_available_models),
        )
        .route(
            "/agent.v1.AgentService/GetUsableModels",
            post(relay_usable_models),
        )
        .route(
            "/aiserver.v1.AiService/GetUsableModels",
            post(relay_usable_models),
        )
        .route(
            "/aiserver.v1.BidiService/BidiAppend",
            post(cursor_bidi_append),
        )
        .route("/agent.v1.AgentService/Run", post(cursor_agent_run))
        .route("/agent.v1.AgentService/RunSSE", post(cursor_run_sse))
        .route(
            "/agent.v1.AgentService/GetPromptContextUsage",
            post(cursor_prompt_context_usage),
        )
        .route(
            "/agent.v1.AgentService/UploadConversationBlobs",
            post(cursor_upload_conversation_blobs),
        )
        .route(
            "/agent.v1.AgentService/NotifyConversationClone",
            post(cursor_notify_conversation_clone),
        )
        .route(
            "/agent.v1.AgentService/GetNewChatNudgeParameterizedModelPicker",
            post(cursor_nudge),
        )
        .route(
            "/agent.v1.AgentService/NameAgent",
            post(cursor_name_agent),
        )
        .route(
            "/agent.v1.AgentService/GetSignedUrlForAttachedMedia",
            post(cursor_signed_media_url),
        )
        .route(
            "/agent.v1.AgentService/GetDefaultModelForCli",
            post(cursor_default_model_cli),
        )
        .route("/gba-media/{*key}", put(gba_media_put).get(gba_media_get))
        .route("/health", get(health))
        .route("/v1/models", get(openai_models))
        .route("/v1/chat/completions", post(openai_chat))
        .layer(local_cors())
        .with_state(state)
}

fn local_cors() -> CorsLayer {
    CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_origin(AllowOrigin::predicate(|origin, _| {
            let value = origin.as_bytes();
            value.starts_with(b"http://127.0.0.1")
                || value.starts_with(b"http://localhost")
                || value.starts_with(b"https://127.0.0.1")
                || value.starts_with(b"https://localhost")
        }))
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../web/index.html"))
}

async fn css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../web/app.css"),
    )
}

async fn js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../web/app.js"),
    )
}

async fn status(State(state): State<AppState>) -> Json<Value> {
    let account = state.account.lock().await;
    let proxy = state.proxy.lock().await;
    Json(json!({
        "ok": true,
        "name": "grok-bot-auth",
        "version": env!("CARGO_PKG_VERSION"),
        "account": public_account(account.as_ref()),
        "models": present_catalog(state.models.lock().await.clone()),
        "cursor": {
            "coexist": *state.coexist.lock().await,
            "proxyUrl": proxy.url,
            "settingsProxy": settings::current_proxy().ok().flatten(),
            "note": "Official account and official models stay on api2. Catalog/gb-* go to 127.0.0.1:47821 only while Grok-Bot-Auth is running. Restart Cursor after enabling."
        },
        "openai": {
            "baseUrl": "http://127.0.0.1:47821/v1",
            "note": "OpenAI-compatible local endpoint; does not use cursor-byok"
        }
    }))
}

async fn health() -> Json<Value> {
    Json(json!({
        "ok": true,
        "name": "Grok-Bot-Auth",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn import(
    State(state): State<AppState>,
    Json(body): Json<ImportBody>,
) -> Result<Json<Value>> {
    let mut account = import_account(body)?;
    if let Ok(Some(access)) = state.client.sand_access(&account).await {
        account.sand_state = Some(access);
    }
    fill_account_identity(&mut account);
    let public = public_account(Some(&account));
    state.commit_grok_account(account).await?;
    Ok(Json(json!({ "account": public })))
}

async fn logout(State(state): State<AppState>) -> Result<Json<Value>> {
    let email = state
        .account
        .lock()
        .await
        .as_ref()
        .map(account_id)
        .unwrap_or_default();
    if email.is_empty() {
        store::clear_account().await?;
        *state.account.lock().await = None;
    } else {
        state.remove_grok_email(&email).await?;
    }
    if state.account.lock().await.is_none() {
        state.models.lock().await.clear();
    }
    Ok(Json(json!({ "ok": true })))
}

async fn login_start(State(state): State<AppState>) -> Json<Value> {
    let (start, session) = begin_login();
    state.logins.lock().await.insert(start.id.clone(), session);
    Json(json!(start))
}

#[derive(Debug, Deserialize)]
struct PollBody {
    id: String,
}

async fn login_poll(
    State(state): State<AppState>,
    Json(body): Json<PollBody>,
) -> Result<Json<Value>> {
    let session = state
        .logins
        .lock()
        .await
        .get(&body.id)
        .cloned()
        .ok_or_else(|| Error::Msg("unknown login session".into()))?;
    match state
        .client
        .poll_login(&session.uuid, &session.verifier)
        .await?
    {
        None => Ok(Json(json!({ "status": "pending" }))),
        Some((access, refresh)) => {
            let mut account = Account {
                access_token: access,
                refresh_token: Some(refresh),
                machine_id: session.machine_id,
                display_name: "Grok Bot".into(),
                ..Account::default()
            };
            if let Ok(Some(access_state)) = state.client.sand_access(&account).await {
                account.sand_state = Some(access_state);
            }
            fill_account_identity(&mut account);
            let public = public_account(Some(&account));
            state.commit_grok_account(account).await?;
            state.logins.lock().await.remove(&body.id);
            Ok(Json(json!({ "status": "completed", "account": public })))
        }
    }
}

async fn refresh(State(state): State<AppState>) -> Result<Json<Value>> {
    let current = state
        .account
        .lock()
        .await
        .clone()
        .ok_or_else(|| Error::Msg("sign in first".into()))?;
    let mut next = state.client.refresh(&current).await?;
    fill_account_identity(&mut next);
    let public = public_account(Some(&next));
    state.commit_grok_account(next).await?;
    Ok(Json(json!({ "account": public })))
}

async fn sync_models(State(state): State<AppState>) -> Result<Json<Value>> {
    let account = state
        .account
        .lock()
        .await
        .clone()
        .ok_or_else(|| Error::Msg("sign in first".into()))?;
    let body = state.client.available_models(&account).await?;
    let families = parse_sand_families(&body);
    *state.models.lock().await = families.clone();
    {
        let mut enabled = state.enabled.lock().await;
        *enabled = crate::catalog::normalize_enabled(&families, enabled.clone());
    }
    if let Some(id) = first_available_id(&families) {
        let mut preferred = state.preferred_model.lock().await;
        let stale = match preferred.as_ref() {
            Some(current) => !grok_bot_inference_allowed(current),
            None => true,
        };
        if stale {
            *preferred = Some(id);
        }
    }
    let _ = state.persist_prefs().await;
    Ok(Json(json!({ "models": families })))
}

#[derive(Debug, Deserialize, Clone)]
struct ChatBody {
    message: String,
    #[serde(default = "default_model")]
    model: String,
    effort: Option<String>,
    #[serde(default)]
    fast: bool,
    context: Option<String>,
    provider_id: Option<String>,
}

fn default_model() -> String {
    "grok-4.6".into()
}

async fn chat(
    State(state): State<AppState>,
    Json(body): Json<ChatBody>,
) -> Result<impl IntoResponse> {
    let provider = if let Some(id) = body.provider_id.as_deref() {
        state
            .providers
            .lock()
            .await
            .iter()
            .find(|item| item.id == id)
            .cloned()
    } else {
        None
    };
    let account = state.account.lock().await.clone();
    if provider.is_none() && account.is_none() {
        return Err(Error::Msg("sign in first".into()));
    }
    let (event_tx, event_rx) = mpsc::channel::<std::result::Result<Event, Infallible>>(64);
    let (delta_tx, mut delta_rx) = mpsc::channel::<ChatDelta>(64);
    let client = state.client.clone();
    tokio::spawn(async move {
        let fail_tx = event_tx.clone();
        let pump = tokio::spawn(async move {
            while let Some(delta) = delta_rx.recv().await {
                if let Some(error) = delta.error {
                    let _ = event_tx
                        .send(Ok(Event::default().event("error").data(error)))
                        .await;
                    continue;
                }
                if !delta.thinking.is_empty() {
                    let _ = event_tx
                        .send(Ok(Event::default().event("thinking").data(delta.thinking)))
                        .await;
                }
                if !delta.text.is_empty() {
                    let _ = event_tx
                        .send(Ok(Event::default().event("text").data(delta.text)))
                        .await;
                }
            }
            let _ = event_tx
                .send(Ok(Event::default().event("done").data("[done]")))
                .await;
        });
        let pool = state.prompt_pool.lock().await.clone();
        let budget = crate::compact::official_budget();
        let turns = crate::compact::prepare_outbound(&pool, &[], &body.message, budget);
        let effort = body.effort.clone();
        let context = body.context.clone();
        let model = body.model.clone();
        let fast = body.fast;
        let send = if let Some(provider) = provider {
            client
                .stream_provider(&provider, &model, &turns, delta_tx)
                .await
        } else if let Some(account) = account.as_ref() {
            client
                .stream_chat_turns(
                    account,
                    &model,
                    &turns,
                    effort.as_deref(),
                    fast,
                    context.as_deref(),
                    delta_tx,
                )
                .await
        } else {
            Err(Error::Msg("sign in first".into()))
        };
        if let Err(error) = send {
            let _ = fail_tx
                .send(Ok(Event::default().event("error").data(error.to_string())))
                .await;
        }
        let _ = pump.await;
    });
    Ok(Sse::new(ReceiverStream::new(event_rx))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

async fn cursor_enable(State(state): State<AppState>) -> Result<Json<Value>> {
    let url = start_coexist(&state).await?;
    Ok(Json(json!({
        "proxyUrl": url,
        "restartCursor": true
    })))
}

async fn cursor_disable(State(state): State<AppState>) -> Result<Json<Value>> {
    stop_coexist(&state).await?;
    Ok(Json(json!({ "ok": true })))
}

async fn relay_default_model(State(state): State<AppState>) -> impl IntoResponse {
    let catalog = present_catalog(state.models.lock().await.clone());
    let preferred = state.preferred_model.lock().await.clone();
    let id = preferred
        .filter(|id| grok_bot_inference_allowed(id))
        .or_else(|| first_available_id(&catalog))
        .map(|id| catalog::injected_id(&id))
        .unwrap_or_else(|| catalog::injected_id("grok-4.6"));
    let body = catalog::encode_default_model(&id);
    let mut framed = crate::connect::encode_connect_frame(&body);
    framed.extend_from_slice(&[2, 0, 0, 0, 0]);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/connect+proto")],
        framed,
    )
}

async fn relay_default_nudge(State(state): State<AppState>) -> impl IntoResponse {
    let enabled = state.enabled.lock().await.clone();
    let body = catalog::encode_default_nudge(&enabled);
    let mut framed = crate::connect::encode_connect_frame(&body);
    framed.extend_from_slice(&[2, 0, 0, 0, 0]);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/connect+proto")],
        framed,
    )
}

async fn relay_bootstrap_statsig(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let forwarded: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(name, value)| {
            Some((name.as_str().to_string(), value.to_str().ok()?.to_string()))
        })
        .collect();
    match state
        .client
        .proxy_forward(
            "/aiserver.v1.AnalyticsService/BootstrapStatsig",
            &forwarded,
            body.to_vec(),
        )
        .await
    {
        Ok((status, content_type, bytes)) => {
            let body = match crate::statsig::patch_bootstrap_body(&bytes) {
                Some(patched) => {
                    let _ = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(crate::store::data_dir().join("proxy.log"))
                        .and_then(|mut file| {
                            use std::io::Write;
                            writeln!(file, "relay BootstrapStatsig: nal_websocket_client=false")
                        });
                    patched
                }
                None => {
                    let _ = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(crate::store::data_dir().join("proxy.log"))
                        .and_then(|mut file| {
                            use std::io::Write;
                            writeln!(file, "relay BootstrapStatsig skipped")
                        });
                    bytes
                }
            };
            (
                StatusCode::from_u16(status).unwrap_or(StatusCode::OK),
                [(header::CONTENT_TYPE, content_type)],
                body,
            )
                .into_response()
        }
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

async fn inject_families(state: &AppState) -> Vec<crate::sand::SandFamily> {
    let catalog = present_catalog(state.models.lock().await.clone());
    let enabled = state.enabled.lock().await.clone();
    let providers = state.providers.lock().await.clone();
    let mut families = catalog::started_families(&catalog, &enabled);
    families.extend(catalog::started_provider_families(
        &providers, &enabled, &catalog,
    ));
    families
}

async fn relay_merge_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let kind = headers
        .get("x-gb-catalog")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("available");
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/proto")
        .to_owned();
    let families = inject_families(&state).await;
    let merged = if kind == "usable" {
        catalog::merge_usable_models(&body, &content_type, &families)
    } else if kind == "nudge" {
        body.to_vec()
    } else {
        catalog::merge_available_models(&body, &content_type, &families)
    };
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, content_type)],
        merged,
    )
        .into_response()
}

async fn relay_available_models(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let forwarded: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(name, value)| {
            Some((name.as_str().to_string(), value.to_str().ok()?.to_string()))
        })
        .collect();
    if let Some((content_type, bytes)) = crate::store::load_bytes_cache("available-models") {
        let refresh = state.clone();
        let forwarded = forwarded.clone();
        let body = body.to_vec();
        tokio::spawn(async move {
            if let Ok(ok) = refresh
                .client
                .proxy_forward("/aiserver.v1.AiService/AvailableModels", &forwarded, body)
                .await
            {
                crate::store::save_bytes_cache("available-models", &ok.1, &ok.2);
            }
        });
        let families = inject_families(&state).await;
        let merged = catalog::merge_available_models(&bytes, &content_type, &families);
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, content_type)],
            merged,
        )
            .into_response();
    }
    let upstream = tokio::time::timeout(
        Duration::from_secs(12),
        state.client.proxy_forward(
            "/aiserver.v1.AiService/AvailableModels",
            &forwarded,
            body.to_vec(),
        ),
    )
    .await;
    let (status, content_type, bytes) = match upstream {
        Ok(Ok(ok)) => {
            crate::store::save_bytes_cache("available-models", &ok.1, &ok.2);
            ok
        }
        _ => {
            return (
                StatusCode::BAD_GATEWAY,
                "AvailableModels upstream timeout, no cache",
            )
                .into_response();
        }
    };
    let families = inject_families(&state).await;
    let merged = catalog::merge_available_models(&bytes, &content_type, &families);
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::OK),
        [(header::CONTENT_TYPE, content_type)],
        merged,
    )
        .into_response()
}

async fn relay_usable_models(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let forwarded: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(name, value)| {
            Some((name.as_str().to_string(), value.to_str().ok()?.to_string()))
        })
        .collect();
    let path = headers
        .get("x-grok-upstream-path")
        .and_then(|v| v.to_str().ok())
        .filter(|p| p.contains("GetUsableModels"))
        .unwrap_or("/agent.v1.AgentService/GetUsableModels");
    if let Some((content_type, bytes)) = crate::store::load_bytes_cache("usable-models") {
        let refresh = state.clone();
        let forwarded = forwarded.clone();
        let body = body.to_vec();
        let path = path.to_owned();
        tokio::spawn(async move {
            if let Ok(ok) = refresh.client.proxy_forward(&path, &forwarded, body).await {
                crate::store::save_bytes_cache("usable-models", &ok.1, &ok.2);
            }
        });
        let families = inject_families(&state).await;
        let merged = catalog::merge_usable_models(&bytes, &content_type, &families);
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, content_type)],
            merged,
        )
            .into_response();
    }
    let upstream = tokio::time::timeout(
        Duration::from_secs(12),
        state.client.proxy_forward(path, &forwarded, body.to_vec()),
    )
    .await;
    let (status, content_type, bytes) = match upstream {
        Ok(Ok(ok)) => {
            crate::store::save_bytes_cache("usable-models", &ok.1, &ok.2);
            ok
        }
        _ => {
            return (
                StatusCode::BAD_GATEWAY,
                "GetUsableModels upstream timeout, no cache",
            )
                .into_response();
        }
    };
    let families = inject_families(&state).await;
    let merged = catalog::merge_usable_models(&bytes, &content_type, &families);
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::OK),
        [(header::CONTENT_TYPE, content_type)],
        merged,
    )
        .into_response()
}

fn header_pairs(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            Some((name.as_str().to_string(), value.to_str().ok()?.to_string()))
        })
        .collect()
}

fn log_cursor_rpc(line: &str) {
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(crate::store::data_dir().join("proxy.log"))
        .and_then(|mut file| {
            use std::io::Write;
            writeln!(file, "{line}")
        });
}

async fn forward_cursor_rpc(
    state: &AppState,
    path: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> axum::response::Response {
    match state
        .client
        .proxy_forward(path, &header_pairs(headers), body.to_vec())
        .await
    {
        Ok((status, content_type, bytes)) => (
            StatusCode::from_u16(status).unwrap_or(StatusCode::OK),
            [(header::CONTENT_TYPE, content_type)],
            bytes,
        )
            .into_response(),
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

async fn notify_sse_local(state: &AppState, run: crate::agent_wire::LocalRun) {
    let mut waiters = state.sse_waiters.lock().await;
    if let Some(waiter) = waiters.get_mut(&run.request_id) {
        if let Some(tx) = waiter.tx.take() {
            let _ = tx.send(SseDecision::Local(run));
        }
    }
}

async fn notify_sse_official(state: &AppState, request_id: &str) {
    let started = {
        let mut waiters = state.sse_waiters.lock().await;
        waiters.get_mut(request_id).and_then(|waiter| {
            waiter.tx.take().map(|tx| {
                let started = waiter.started.clone();
                let _ = tx.send(SseDecision::Official);
                started
            })
        })
    };
    if let Some(started) = started {
        let _ = tokio::time::timeout(Duration::from_millis(800), started.notified()).await;
    }
}

async fn stream_forward_cursor_rpc(
    state: &AppState,
    path: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> axum::response::Response {
    match state
        .client
        .proxy_forward_stream(path, &header_pairs(headers), body.to_vec())
        .await
    {
        Ok(response) => {
            let status = StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::OK);
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("application/connect+proto")
                .to_owned();
            let stream = response
                .bytes_stream()
                .map(|chunk| chunk.map_err(|error| std::io::Error::other(error.to_string())));
            let mut out_headers = HeaderMap::new();
            if let Ok(value) = content_type.parse() {
                out_headers.insert(header::CONTENT_TYPE, value);
            }
            if let Ok(value) = "1".parse() {
                out_headers.insert(HeaderName::from_static("connect-protocol-version"), value);
            }
            (status, out_headers, Body::from_stream(stream)).into_response()
        }
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

fn empty_bidi() -> axum::response::Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/proto")],
        Vec::<u8>::new(),
    )
        .into_response()
}

fn spawn_injected_connect(
    account: Option<crate::sand::Account>,
    providers: Vec<crate::providers::Provider>,
    client: crate::client::SandClient,
    hub: crate::agent_session::ExecHub,
    run: crate::agent_wire::LocalRun,
    local_runs: Arc<Mutex<HashMap<String, crate::agent_wire::LocalRun>>>,
) -> axum::response::Response {
    let (tx, rx) = mpsc::unbounded_channel::<std::result::Result<Bytes, std::io::Error>>();
    tokio::spawn(async move {
        let request_id = run.request_id.clone();
        let jobs = crate::agent_session::spawn_wait_bridge(
            hub.clone(),
            request_id.clone(),
            run.cancel.clone(),
        );
        let emit_tx = tx.clone();
        let jobs_ix = jobs.clone();
        let result = crate::proxy::stream_injected_run(
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
                let _ = emit_tx.send(Ok(Bytes::from(crate::agent_wire::encode_server(&msg))));
            },
        )
        .await;
        match result {
            Ok(()) => {
                let _ = tx.send(Ok(Bytes::from(crate::agent_wire::encode_end_stream_ok())));
            }
            Err(error) => {
                let _ = tx.send(Ok(Bytes::from(crate::agent_wire::encode_end_stream_error(
                    "unavailable",
                    &error,
                ))));
            }
        }
        hub.finish(&request_id).await;
        local_runs.lock().await.remove(&request_id);
    });
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/connect+proto"),
            (HeaderName::from_static("connect-protocol-version"), "1"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        Body::from_stream(UnboundedReceiverStream::new(rx)),
    )
        .into_response()
}

async fn cursor_bidi_append(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let body = Bytes::from(crate::connect::prepare_cursor_body(&body));
    let enabled = state.enabled.lock().await.clone();
    let scanned = crate::agent_wire::scan_gb_model(&body);
    let mut official_pair_id = None;
    if let Some(decoded) = crate::agent_wire::decode_bidi_append(&body) {
        official_pair_id = Some(decoded.request_id.clone());
        if let Some(text) = decoded.queued_user.clone() {
            crate::agent_wire::queue_user(
                &mut *state.queued_users.lock().await,
                &decoded.request_id,
                text,
                decoded.queued_background,
            );
        }
        if let Some(mut run) = decoded.run {
            if crate::agent_wire::is_injected_model(&run.model_id, &enabled)
                || run.model_id.starts_with("gb-")
            {
                let queued = crate::agent_wire::drain_queued(
                    &mut *state.queued_users.lock().await,
                    &decoded.request_id,
                );
                if run.user_text.is_empty() {
                    if let Some(queued) = queued {
                        run.user_text = queued.text;
                        run.is_background_completion |= queued.is_background;
                    }
                }
                log_cursor_rpc(&format!(
                    "BidiAppend local request_id={} model={}",
                    decoded.request_id, run.model_id
                ));
                state
                    .local_runs
                    .lock()
                    .await
                    .insert(decoded.request_id.clone(), run.clone());
                state.exec_hub.put_run(run.clone()).await;
                notify_sse_local(&state, run).await;
                return empty_bidi();
            }
        } else if state.exec_hub.is_local(&decoded.request_id).await
            || state
                .local_runs
                .lock()
                .await
                .contains_key(&decoded.request_id)
        {
            if let Some(exec) = decoded.exec {
                log_cursor_rpc(&format!(
                    "BidiAppend exec local request_id={} id={}",
                    decoded.request_id, exec.id
                ));
                state
                    .exec_hub
                    .push(
                        &decoded.request_id,
                        crate::agent_session::ClientEvt::Exec(exec),
                    )
                    .await;
            } else if let Some((id, error)) = decoded.throw.clone() {
                log_cursor_rpc(&format!(
                    "BidiAppend throw local request_id={} id={id}",
                    decoded.request_id
                ));
                state
                    .exec_hub
                    .push(
                        &decoded.request_id,
                        crate::agent_session::ClientEvt::Throw { id, error },
                    )
                    .await;
            } else if decoded.cancel {
                log_cursor_rpc(&format!(
                    "BidiAppend cancel local request_id={}",
                    decoded.request_id
                ));
                state
                    .exec_hub
                    .push(&decoded.request_id, crate::agent_session::ClientEvt::Cancel)
                    .await;
            } else if decoded.heartbeat {
                state
                    .exec_hub
                    .push(
                        &decoded.request_id,
                        crate::agent_session::ClientEvt::Heartbeat,
                    )
                    .await;
            } else if let Some(id) = decoded.stream_close {
                log_cursor_rpc(&format!(
                    "BidiAppend streamClose local request_id={} id={id}",
                    decoded.request_id
                ));
                state
                    .exec_hub
                    .push(
                        &decoded.request_id,
                        crate::agent_session::ClientEvt::StreamClose(id),
                    )
                    .await;
            } else if let Some(kv) = decoded.kv {
                log_cursor_rpc(&format!(
                    "BidiAppend kv local request_id={} id={}",
                    decoded.request_id, kv.id
                ));
                state
                    .exec_hub
                    .push(
                        &decoded.request_id,
                        crate::agent_session::ClientEvt::Kv(kv),
                    )
                    .await;
            } else if let Some(interaction) = decoded.interaction {
                log_cursor_rpc(&format!(
                    "BidiAppend interaction local request_id={} id={}",
                    decoded.request_id, interaction.id
                ));
                state
                    .exec_hub
                    .push(
                        &decoded.request_id,
                        crate::agent_session::ClientEvt::Interaction(interaction),
                    )
                    .await;
            } else {
                log_cursor_rpc(&format!(
                    "BidiAppend local keep request_id={}",
                    decoded.request_id
                ));
            }
            return empty_bidi();
        }
    } else if let Some(model) = scanned {
        log_cursor_rpc(&format!("BidiAppend decode-fail scan model={model}"));
    } else {
        let _ = std::fs::write(crate::store::data_dir().join("last-bidi.bin"), &body);
        log_cursor_rpc(&format!(
            "BidiAppend miss decode len={} head={:02x?}",
            body.len(),
            body.iter().take(12).copied().collect::<Vec<_>>()
        ));
    }
    if let Some(id) = official_pair_id {
        notify_sse_official(&state, &id).await;
    }
    log_cursor_rpc("BidiAppend forward official");
    forward_cursor_rpc(
        &state,
        "/aiserver.v1.BidiService/BidiAppend",
        &headers,
        body,
    )
    .await
}

async fn local_agent_completion(
    state: &AppState,
    run: crate::agent_wire::LocalRun,
) -> axum::response::Response {
    log_cursor_rpc(&format!(
        "AgentService/Run local request_id={} model={} text_len={}",
        run.request_id,
        run.model_id,
        run.user_text.len()
    ));
    state.exec_hub.put_run(run.clone()).await;
    let account = state.account.lock().await.clone();
    let providers = state.providers.lock().await.clone();
    let client = state.client.clone();
    spawn_injected_connect(
        account,
        providers,
        client,
        state.exec_hub.clone(),
        run,
        state.local_runs.clone(),
    )
}

async fn cursor_agent_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let enabled = state.enabled.lock().await.clone();
    if let Some(run) = crate::agent_wire::decode_connect_agent_run(&body) {
        if crate::agent_wire::is_injected_model(&run.model_id, &enabled)
            || run.model_id.starts_with("gb-")
        {
            return local_agent_completion(&state, run).await;
        }
        log_cursor_rpc(&format!(
            "AgentService/Run forward official model={}",
            run.model_id
        ));
    } else {
        log_cursor_rpc("AgentService/Run forward official (no gb-)");
    }
    stream_forward_cursor_rpc(&state, "/agent.v1.AgentService/Run", &headers, body).await
}

async fn cursor_run_sse(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let body = Bytes::from(crate::connect::prepare_cursor_body(&body));
    if let Some(request_id) = crate::agent_wire::decode_run_sse_id(&body) {
        if let Some(run) = state.local_runs.lock().await.get(&request_id).cloned() {
            log_cursor_rpc(&format!(
                "RunSSE local request_id={} model={}",
                request_id, run.model_id
            ));
            let account = state.account.lock().await.clone();
            let providers = state.providers.lock().await.clone();
            let client = state.client.clone();
            state.exec_hub.put_run(run.clone()).await;
            return spawn_injected_connect(
                account,
                providers,
                client,
                state.exec_hub.clone(),
                run,
                state.local_runs.clone(),
            );
        }
        let (tx, rx) = oneshot::channel();
        let started = Arc::new(Notify::new());
        state.sse_waiters.lock().await.insert(
            request_id.clone(),
            SseWaiter {
                tx: Some(tx),
                started: started.clone(),
            },
        );
        log_cursor_rpc(&format!(
            "RunSSE waiting for BidiAppend request_id={request_id}"
        ));
        let decision = tokio::time::timeout(Duration::from_millis(2000), rx).await;
        state.sse_waiters.lock().await.remove(&request_id);
        match decision {
            Ok(Ok(SseDecision::Local(run))) => {
                log_cursor_rpc(&format!(
                    "RunSSE local request_id={} model={}",
                    request_id, run.model_id
                ));
                let account = state.account.lock().await.clone();
                let providers = state.providers.lock().await.clone();
                let client = state.client.clone();
                state.exec_hub.put_run(run.clone()).await;
                return spawn_injected_connect(
                    account,
                    providers,
                    client,
                    state.exec_hub.clone(),
                    run,
                    state.local_runs.clone(),
                );
            }
            Ok(Ok(SseDecision::Official)) => {
                log_cursor_rpc(&format!(
                    "RunSSE forward official request_id={request_id} (paired)"
                ));
                let response = stream_forward_cursor_rpc(
                    &state,
                    "/agent.v1.AgentService/RunSSE",
                    &headers,
                    body,
                )
                .await;
                started.notify_waiters();
                return response;
            }
            _ => {
                log_cursor_rpc(&format!(
                    "RunSSE forward official request_id={request_id} (no gb- bidi)"
                ));
                let response = stream_forward_cursor_rpc(
                    &state,
                    "/agent.v1.AgentService/RunSSE",
                    &headers,
                    body,
                )
                .await;
                started.notify_waiters();
                return response;
            }
        }
    }
    log_cursor_rpc("RunSSE forward official");
    stream_forward_cursor_rpc(&state, "/agent.v1.AgentService/RunSSE", &headers, body).await
}

async fn cursor_upload_conversation_blobs(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let payload = crate::connect::unwrap_connect(&body).1;
    let req = crate::agent_wire::UploadConversationBlobsRequest::decode(payload)
        .unwrap_or_default();
    if req.conversation_id.is_empty() || !crate::blob::is_local_conversation(&req.conversation_id)
    {
        return forward_cursor_rpc(
            &state,
            "/agent.v1.AgentService/UploadConversationBlobs",
            &headers,
            body,
        )
        .await;
    }
    for blob in req.blobs {
        if blob.id.is_empty() {
            continue;
        }
        let id = String::from_utf8_lossy(&blob.id).into_owned();
        let data = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &blob.value,
        );
        let _ = crate::blob::put_cas(&id, &data);
    }
    let mut framed = crate::connect::encode_connect_frame(
        &crate::agent_wire::UploadConversationBlobsResponse {}.encode_to_vec(),
    );
    framed.extend_from_slice(&crate::connect::encode_end_stream());
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/connect+proto")],
        framed,
    )
        .into_response()
}

async fn cursor_notify_conversation_clone(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let payload = crate::connect::unwrap_connect(&body).1;
    let req = crate::agent_wire::NotifyConversationCloneRequest::decode(payload)
        .unwrap_or_default();
    if !crate::blob::is_local_conversation(&req.source_conversation_id)
        && !crate::blob::is_local_conversation(&req.conversation_id)
    {
        return forward_cursor_rpc(
            &state,
            "/agent.v1.AgentService/NotifyConversationClone",
            &headers,
            body,
        )
        .await;
    }
    if !req.conversation_id.is_empty() {
        crate::blob::register_clone(
            &req.conversation_id,
            &req.source_conversation_id,
            &req.source_request_id,
        );
    }
    let mut framed = crate::connect::encode_connect_frame(
        &crate::agent_wire::NotifyConversationCloneResponse {}.encode_to_vec(),
    );
    framed.extend_from_slice(&crate::connect::encode_end_stream());
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/connect+proto")],
        framed,
    )
        .into_response()
}

async fn cursor_prompt_context_usage(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let payload = crate::connect::unwrap_connect(&body).1;
    let req = crate::agent_wire::GetPromptContextUsageRequest::decode(payload)
        .unwrap_or_default();
    if !crate::blob::is_local_conversation(&req.conversation_id) {
        return forward_cursor_rpc(
            &state,
            "/agent.v1.AgentService/GetPromptContextUsage",
            &headers,
            body,
        )
        .await;
    }
    let snapshot = crate::agent_wire::load_usage_snapshot(&req.snapshot_blob_id).or_else(|| {
        Some(crate::agent_wire::PromptContextUsageSnapshot {
            prompt_context_usage_tree: Some(crate::agent_wire::context_usage_tree(0, 0)),
            root_prompt_messages_json: Vec::new(),
        })
    });
    let resp = crate::agent_wire::GetPromptContextUsageResponse { snapshot };
    let mut framed = crate::connect::encode_connect_frame(&resp.encode_to_vec());
    framed.extend_from_slice(&crate::connect::encode_end_stream());
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/connect+proto")],
        framed,
    )
        .into_response()
}

async fn cursor_name_agent(body: Bytes) -> impl IntoResponse {
    let payload = crate::connect::unwrap_connect(&body).1;
    let req = crate::agent_wire::NameAgentRequest::decode(payload).unwrap_or_default();
    let resp = crate::agent_wire::NameAgentResponse {
        name: crate::agent_wire::name_agent_title(&req.user_message),
    };
    unary_ack(&resp.encode_to_vec())
}

async fn cursor_signed_media_url(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let payload = crate::connect::unwrap_connect(&body).1;
    let req = crate::agent_wire::GetSignedUrlForAttachedMediaRequest::decode(payload)
        .unwrap_or_default();
    let local = if req.conversation_id.is_empty() {
        crate::blob::has_any_local_conversation()
    } else {
        crate::blob::is_local_conversation(&req.conversation_id)
    };
    if !local {
        return forward_cursor_rpc(
            &state,
            "/agent.v1.AgentService/GetSignedUrlForAttachedMedia",
            &headers,
            body,
        )
        .await;
    }
    let key = req.key.clone().filter(|k| !k.is_empty()).unwrap_or_else(|| {
        format!("gba-{}", uuid::Uuid::new_v4())
    });
    let resp = crate::agent_wire::signed_media_response(&key);
    unary_ack(&resp.encode_to_vec())
}

async fn gba_media_put(
    Path(key): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let mime = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();
    match crate::blob::put_media(&key, &mime, body.to_vec()) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::PAYLOAD_TOO_LARGE,
    }
}

async fn gba_media_get(Path(key): Path<String>) -> impl IntoResponse {
    match crate::blob::get_media(&key) {
        Some((mime, bytes)) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime)],
            bytes,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn cursor_default_model_cli() -> impl IntoResponse {
    unary_ack(
        &crate::agent_wire::GetDefaultModelForCliResponse { model: None }.encode_to_vec(),
    )
}

fn unary_ack(payload: &[u8]) -> axum::response::Response {
    let mut framed = crate::connect::encode_connect_frame(payload);
    framed.extend_from_slice(&crate::connect::encode_end_stream());
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/connect+proto")],
        framed,
    )
        .into_response()
}

async fn cursor_nudge(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let enabled = state.enabled.lock().await.clone();
    if crate::catalog::request_uses_injected_model(&body, &enabled) {
        let mut framed = crate::connect::encode_connect_frame(&[]);
        framed.extend_from_slice(&[2, 0, 0, 0, 0]);
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/connect+proto")],
            framed,
        )
            .into_response();
    }
    forward_cursor_rpc(
        &state,
        "/agent.v1.AgentService/GetNewChatNudgeParameterizedModelPicker",
        &headers,
        body,
    )
    .await
}

async fn openai_models(State(state): State<AppState>) -> Json<Value> {
    let models = state.models.lock().await;
    Json(json!({
        "object": "list",
        "data": models.iter().map(|model| json!({
            "id": model.id,
            "object": "model",
            "owned_by": "grok-bot-auth"
        })).collect::<Vec<_>>()
    }))
}

async fn openai_chat(State(state): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    let account = match state.account.lock().await.clone() {
        Some(account) => account,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": { "message": "sign in first" } })),
            )
                .into_response();
        }
    };
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("grok-4.6")
        .to_owned();
    let message = body
        .pointer("/messages/0/content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    let (event_tx, event_rx) = mpsc::channel::<std::result::Result<Event, Infallible>>(64);
    let (delta_tx, mut delta_rx) = mpsc::channel::<ChatDelta>(64);
    let client = state.client.clone();
    tokio::spawn(async move {
        let fail_tx = event_tx.clone();
        let pump = tokio::spawn(async move {
            while let Some(delta) = delta_rx.recv().await {
                if let Some(error) = delta.error {
                    let payload = json!({ "error": { "message": error } });
                    let _ = event_tx
                        .send(Ok(Event::default().data(payload.to_string())))
                        .await;
                    continue;
                }
                if !delta.text.is_empty() {
                    let payload = json!({
                        "choices": [{ "delta": { "content": delta.text }, "index": 0 }]
                    });
                    let _ = event_tx
                        .send(Ok(Event::default().data(payload.to_string())))
                        .await;
                }
            }
            let _ = event_tx.send(Ok(Event::default().data("[DONE]"))).await;
        });
        if let Err(error) = client
            .stream_chat(&account, &model, &message, Some("high"), false, delta_tx)
            .await
        {
            let payload = json!({ "error": { "message": error.to_string() } });
            let _ = fail_tx
                .send(Ok(Event::default().data(payload.to_string())))
                .await;
        }
        let _ = pump.await;
    });
    Sse::new(ReceiverStream::new(event_rx)).into_response()
}
