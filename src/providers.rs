//! Official/BYOK provider records (CC Switch / cursor-byok style).
//! Does not replace the grok_bot Cursor-session plane.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    #[default]
    Openai,
    Xai,
    Anthropic,
    Gemini,
    Zhipu,
    Kimi,
    Deepseek,
    Generic,
}

impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Xai => "xai",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::Zhipu => "zhipu",
            Self::Kimi => "kimi",
            Self::Deepseek => "deepseek",
            Self::Generic => "generic",
        }
    }

    pub fn default_base_url(&self) -> &'static str {
        match self {
            Self::Openai => "https://api.openai.com/v1",
            Self::Xai => "https://api.x.ai/v1",
            Self::Anthropic => "https://api.anthropic.com",
            Self::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai",
            Self::Zhipu => "https://open.bigmodel.cn/api/paas/v4",
            Self::Kimi => "https://api.moonshot.cn/v1",
            Self::Deepseek => "https://api.deepseek.com/v1",
            Self::Generic => "https://api.openai.com/v1",
        }
    }

    pub fn default_wire(&self) -> WireFormat {
        match self {
            Self::Anthropic => WireFormat::AnthropicMessages,
            _ => WireFormat::ChatCompletions,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WireFormat {
    #[default]
    ChatCompletions,
    Responses,
    AnthropicMessages,
}

impl WireFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ChatCompletions => "chat_completions",
            Self::Responses => "responses",
            Self::AnthropicMessages => "anthropic_messages",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::ChatCompletions => "Chat Completions",
            Self::Responses => "Responses",
            Self::AnthropicMessages => "Anthropic Messages",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    #[default]
    ApiKey,
    Oauth,
}

impl AuthMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ApiKey => "api_key",
            Self::Oauth => "oauth",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ModelMap {
    pub alias: String,
    pub upstream: String,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    /// Thinking grades the user enabled. Palette is CCursor-style
    /// none/minimal/low/medium/high/xhigh/max/ultra. Empty = follow official catalog.
    #[serde(default)]
    pub efforts: Vec<String>,
    /// Official catalog has a Fast boolean axis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<bool>,
}

/// CCursor 思考等级划分（截图：none … ultra）。
pub const THINKING_GRADES: &[&str] = &[
    "none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra",
];

pub fn thinking_grades() -> Vec<String> {
    THINKING_GRADES.iter().map(|s| (*s).to_owned()).collect()
}

#[allow(dead_code)]
pub const OFFICIAL_EFFORTS: &[&str] = THINKING_GRADES;

#[allow(dead_code)]
pub fn official_efforts() -> Vec<String> {
    thinking_grades()
}

pub fn effort_display(value: &str) -> String {
    crate::sand::axis_value_label(value)
}

pub fn row_efforts(row: &ModelMap) -> Vec<String> {
    if !row.efforts.is_empty() {
        row.efforts.clone()
    } else if let Some(effort) = row.effort.as_deref().filter(|s| !s.is_empty()) {
        vec![effort.to_owned()]
    } else {
        Vec::new()
    }
}

pub fn preferred_effort(row: &ModelMap) -> Option<String> {
    if let Some(effort) = row
        .effort
        .as_deref()
        .filter(|value| !value.is_empty() && *value != "auto")
    {
        return Some(effort.to_owned());
    }
    let list = row_efforts(row);
    if list.iter().any(|item| item == "high") {
        Some("high".into())
    } else {
        list.into_iter().next()
    }
}

pub fn maps_for_picked(picked: &[String], existing: &[ModelMap]) -> Vec<ModelMap> {
    picked
        .iter()
        .filter(|id| !id.is_empty())
        .map(|id| {
            existing
                .iter()
                .find(|entry| entry.alias == *id)
                .cloned()
                .unwrap_or_else(|| identity_map(id))
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAccount {
    pub email: String,
    #[serde(default)]
    pub display_name: String,
    pub token: String,
    #[serde(default)]
    pub exhausted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota: Option<crate::quota::QuotaSnapshot>,
}

impl crate::accounts::SlotId for ProviderAccount {
    fn slot_id(&self) -> String {
        crate::accounts::normalize_email(&self.email)
    }
    fn is_exhausted(&self) -> bool {
        self.exhausted
    }
    fn mark_exhausted(&mut self, error: Option<String>) {
        self.exhausted = true;
        self.last_error = error;
    }
    fn clear_exhausted(&mut self) {
        self.exhausted = false;
        self.last_error = None;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub kind: ProviderKind,
    pub label: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub auth_mode: AuthMode,
    #[serde(default)]
    pub oauth_token: String,
    #[serde(default)]
    pub oauth_refresh: String,
    #[serde(default)]
    pub wire: WireFormat,
    #[serde(default)]
    pub model_map: Vec<ModelMap>,
    #[serde(default)]
    pub catalog: Vec<String>,
    #[serde(default)]
    pub enabled_models: Vec<String>,
    #[serde(default)]
    pub selected_model: String,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub oauth_accounts: Vec<ProviderAccount>,
    #[serde(default)]
    pub pool_mode: crate::accounts::PoolMode,
    #[serde(default)]
    pub active_email: String,
}

impl Default for Provider {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: ProviderKind::Openai,
            label: String::new(),
            base_url: ProviderKind::Openai.default_base_url().into(),
            api_key: String::new(),
            auth_mode: AuthMode::ApiKey,
            oauth_token: String::new(),
            oauth_refresh: String::new(),
            wire: WireFormat::ChatCompletions,
            model_map: Vec::new(),
            catalog: Vec::new(),
            enabled_models: Vec::new(),
            selected_model: String::new(),
            effort: None,
            context: None,
            oauth_accounts: Vec::new(),
            pool_mode: crate::accounts::PoolMode::Share,
            active_email: String::new(),
        }
    }
}

pub fn provider_account_from_token(token: &str, email_hint: &str) -> ProviderAccount {
    let hint = crate::accounts::normalize_email(email_hint);
    let email = crate::sand::jwt_account_id(token)
        .or_else(|| (!hint.is_empty()).then_some(hint))
        .unwrap_or_else(|| "oauth-account".into());
    ProviderAccount {
        email,
        display_name: crate::sand::jwt_display_name(token).unwrap_or_default(),
        token: token.to_owned(),
        exhausted: false,
        last_error: None,
        quota: None,
    }
}

pub fn upsert_provider_account(provider: &mut Provider, account: ProviderAccount) {
    let id = crate::accounts::normalize_email(&account.email);
    if id.is_empty() {
        return;
    }
    if let Some(existing) = provider
        .oauth_accounts
        .iter_mut()
        .find(|item| crate::accounts::normalize_email(&item.email) == id)
    {
        *existing = account;
    } else {
        provider.oauth_accounts.push(account);
    }
    provider.active_email = id;
    if let Some(current) = provider
        .oauth_accounts
        .iter()
        .find(|item| crate::accounts::normalize_email(&item.email) == provider.active_email)
    {
        provider.oauth_token = current.token.clone();
        if provider.auth_mode == AuthMode::Oauth {
            provider.api_key = current.token.clone();
        }
    }
}

pub fn provider_pool(provider: &Provider) -> crate::accounts::AccountPool<ProviderAccount> {
    let mut pool = crate::accounts::AccountPool {
        mode: provider.pool_mode,
        active_id: crate::accounts::normalize_email(&provider.active_email),
        share_cursor: 0,
        slots: provider.oauth_accounts.clone(),
    };
    if pool.slots.is_empty() {
        let token = bearer_token(provider);
        if !token.is_empty() {
            pool.upsert(provider_account_from_token(&token, &provider.active_email));
        }
    }
    pool
}

pub fn apply_xai_oauth_tokens(
    list: &mut Vec<Provider>,
    access: String,
    refresh: Option<String>,
) -> String {
    if let Some(provider) = list.iter_mut().find(|item| item.kind == ProviderKind::Xai) {
        provider.auth_mode = AuthMode::Oauth;
        provider.oauth_token = access.clone();
        provider.api_key = access.clone();
        if let Some(refresh) = refresh.filter(|s| !s.is_empty()) {
            provider.oauth_refresh = refresh;
        }
        upsert_provider_account(
            provider,
            provider_account_from_token(&access, &provider.label),
        );
        return provider.id.clone();
    }
    let mut provider = Provider {
        id: "xai-xAI".into(),
        kind: ProviderKind::Xai,
        label: "xAI".into(),
        base_url: ProviderKind::Xai.default_base_url().into(),
        api_key: access.clone(),
        auth_mode: AuthMode::Oauth,
        oauth_token: access.clone(),
        oauth_refresh: refresh.unwrap_or_default(),
        wire: WireFormat::ChatCompletions,
        ..Provider::default()
    };
    upsert_provider_account(&mut provider, provider_account_from_token(&access, "xAI"));
    let id = provider.id.clone();
    list.push(provider);
    id
}

pub fn keep_oauth_refresh(incoming: &mut Provider, previous: Option<&Provider>) {
    if incoming.oauth_refresh.is_empty() {
        if let Some(previous) = previous {
            incoming.oauth_refresh = previous.oauth_refresh.clone();
        }
    }
}

pub fn upsert_provider(list: &mut Vec<Provider>, mut incoming: Provider) {
    if let Some(existing) = list.iter_mut().find(|item| item.id == incoming.id) {
        keep_oauth_refresh(&mut incoming, Some(existing));
        *existing = incoming;
        return;
    }
    list.push(incoming);
}

pub fn merge_provider_secrets(prefs: Vec<Provider>, file: Vec<Provider>) -> Vec<Provider> {
    let mut out = prefs;
    for item in file {
        if let Some(slot) = out.iter_mut().find(|row| row.id == item.id) {
            if slot.oauth_refresh.is_empty() {
                slot.oauth_refresh = item.oauth_refresh;
            }
            if slot.oauth_token.is_empty() {
                slot.oauth_token = item.oauth_token.clone();
            }
            if slot.api_key.is_empty() {
                slot.api_key = item.api_key;
            }
        } else {
            out.push(item);
        }
    }
    out
}

pub fn default_model_for(kind: &ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Openai => "gpt-4o",
        ProviderKind::Xai => "grok-3",
        ProviderKind::Anthropic => "claude-sonnet-4-5",
        ProviderKind::Gemini => "gemini-2.0-flash",
        ProviderKind::Zhipu => "glm-4.7",
        ProviderKind::Kimi => "kimi-k2.5",
        ProviderKind::Deepseek => "deepseek-chat",
        ProviderKind::Generic => "gpt-4o",
    }
}

/// Model id used on the outbound request when a 供应商 chip is selected.
pub fn chat_model_for_provider(provider: &Provider) -> String {
    if !provider.selected_model.is_empty() {
        provider.selected_model.clone()
    } else {
        default_model_for(&provider.kind).to_owned()
    }
}

pub fn optional_knob(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "default" {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// Anthropic thinking budget. None = do not send `thinking`.
pub fn anthropic_thinking_budget(effort: Option<&str>) -> Option<u32> {
    match effort? {
        "low" => Some(1024),
        "medium" => Some(4096),
        "high" => Some(8192),
        "xhigh" => Some(16384),
        "max" => Some(32768),
        _ => None,
    }
}

/// `max_tokens` must be strictly greater than thinking budget.
pub fn anthropic_max_tokens(effort: Option<&str>, context: Option<&str>) -> u32 {
    let budget = anthropic_thinking_budget(effort).unwrap_or(0);
    let from_context = context
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|&n| n > 0 && n <= 32_768);
    let floor = if budget == 0 { 4096 } else { budget + 4096 };
    from_context.filter(|&n| n > budget).unwrap_or(floor)
}

fn output_token_cap(context: Option<&str>) -> Option<u32> {
    context
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|&n| n > 0 && n <= 32_768)
}

pub fn row_for_model<'a>(map: &'a [ModelMap], model: &str) -> Option<&'a ModelMap> {
    map.iter()
        .find(|entry| entry.alias == model || entry.upstream == model)
}

pub fn row_knobs(provider: &Provider, model: &str) -> (Option<String>, Option<String>) {
    if let Some(row) = row_for_model(&provider.model_map, model) {
        (
            preferred_effort(row).or_else(|| provider.effort.clone()),
            row.context.clone().or_else(|| provider.context.clone()),
        )
    } else {
        (provider.effort.clone(), provider.context.clone())
    }
}

pub fn apply_model_map(map: &[ModelMap], model: &str) -> String {
    map.iter()
        .find(|entry| entry.alias == model)
        .map(|entry| entry.upstream.clone())
        .unwrap_or_else(|| model.to_owned())
}

pub fn parse_model_map(text: &str) -> Vec<ModelMap> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (alias, upstream) = line.split_once('=')?;
            let alias = alias.trim();
            let upstream = upstream.trim();
            if alias.is_empty() || upstream.is_empty() {
                return None;
            }
            Some(ModelMap {
                alias: alias.into(),
                upstream: upstream.into(),
                context: None,
                effort: None,
                efforts: Vec::new(),
                fast: None,
            })
        })
        .collect()
}

pub fn identity_map(id: &str) -> ModelMap {
    ModelMap {
        alias: id.to_owned(),
        upstream: id.to_owned(),
        context: None,
        effort: None,
        efforts: Vec::new(),
        fast: None,
    }
}

pub fn apply_official_knobs(row: &mut ModelMap, catalog: &[crate::sand::SandFamily]) {
    let Some(family) = catalog
        .iter()
        .find(|family| family.id == row.alias || family.id == row.upstream)
    else {
        return;
    };
    row.context = family.context_token_limit.map(|n| n.to_string());
    row.efforts = family.effort_values();
    row.effort = family.default_effort();
    row.fast = family.has_fast_axis().then_some(true);
}

pub fn map_from_catalog(id: &str, catalog: &[crate::sand::SandFamily]) -> ModelMap {
    let mut row = identity_map(id);
    apply_official_knobs(&mut row, catalog);
    row
}

pub fn official_knob_line(id: &str, catalog: &[crate::sand::SandFamily]) -> String {
    catalog
        .iter()
        .find(|family| family.id == id)
        .map(|family| family.knob_summary())
        .unwrap_or_else(|| "官方目录无此模型，不编 Fast / 思考 / 上下文".into())
}

pub fn supports_oauth(kind: ProviderKind) -> bool {
    matches!(
        kind,
        ProviderKind::Openai | ProviderKind::Xai | ProviderKind::Anthropic
    )
}

/// Keep custom alias→upstream for ids still in the catalog; identity-map the rest.
/// Drops leftover maps (e.g. gpt-4o after switching to xAI).
pub fn merge_catalog_maps(catalog: &[String], existing: &[ModelMap]) -> Vec<ModelMap> {
    catalog
        .iter()
        .filter(|id| !id.is_empty())
        .map(|id| {
            existing
                .iter()
                .find(|entry| entry.alias == *id)
                .cloned()
                .unwrap_or_else(|| identity_map(id))
        })
        .collect()
}

pub fn oauth_slots_for_display(provider: &Provider) -> Vec<ProviderAccount> {
    if !provider.oauth_accounts.is_empty() {
        return provider.oauth_accounts.clone();
    }
    if provider.auth_mode != AuthMode::Oauth {
        return Vec::new();
    }
    let token = bearer_token(provider);
    if token.is_empty() {
        return Vec::new();
    }
    vec![provider_account_from_token(&token, &provider.label)]
}

pub fn visible_provider(provider: &Provider) -> bool {
    provider.id != "default"
        && !provider.label.is_empty()
        && !provider.label.eq_ignore_ascii_case("default")
}

pub fn format_model_map(map: &[ModelMap]) -> String {
    map.iter()
        .map(|entry| format!("{}={}", entry.alias, entry.upstream))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn parse_model_catalog(json: &str) -> Vec<String> {
    let value: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
    let mut ids = Vec::new();
    if let Some(array) = value.get("data").and_then(|v| v.as_array()) {
        for item in array {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                if !id.is_empty() {
                    ids.push(id.to_owned());
                }
            }
        }
    } else if let Some(array) = value.as_array() {
        for item in array {
            if let Some(id) = item
                .as_str()
                .or_else(|| item.get("id").and_then(|v| v.as_str()))
            {
                if !id.is_empty() {
                    ids.push(id.to_owned());
                }
            }
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

pub fn models_list_url(base: &str) -> String {
    let base = base.trim_end_matches('/');
    if base.ends_with("/models") {
        base.to_owned()
    } else if base.ends_with("/v1") {
        format!("{base}/models")
    } else {
        format!("{base}/v1/models")
    }
}

pub fn oauth_portal_url(kind: ProviderKind) -> Result<&'static str, &'static str> {
    match kind {
        ProviderKind::Openai => Ok("https://platform.openai.com/login"),
        ProviderKind::Xai => Ok("https://accounts.x.ai/sign-in"),
        ProviderKind::Anthropic => Ok("https://console.anthropic.com/login"),
        ProviderKind::Gemini => Ok("https://aistudio.google.com/apikey"),
        ProviderKind::Zhipu => Ok("https://open.bigmodel.cn/usercenter/apikeys"),
        ProviderKind::Kimi => Ok("https://platform.moonshot.cn/console/api-keys"),
        ProviderKind::Deepseek => Ok("https://platform.deepseek.com/api_keys"),
        ProviderKind::Generic => Err("generic 供应商没有官方 OAuth，请用 API Key"),
    }
}

pub fn bearer_expired(token: &str) -> bool {
    let Some(exp) = crate::sand::jwt_exp(token) else {
        return false;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    now + 120 >= exp
}

pub fn xai_refresh_body(refresh: &str) -> String {
    format!(
        "grant_type=refresh_token&client_id={}&refresh_token={}",
        XAI_OAUTH_CLIENT_ID,
        urlencoding_lite(refresh)
    )
}

fn json_str(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = value
            .get(*key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            return Some(text.to_owned());
        }
    }
    None
}

pub fn parse_xai_token_json(json: &str) -> Result<(String, Option<String>), String> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let root = if value.get("access_token").is_some() || value.get("accessToken").is_some() {
        &value
    } else {
        value.get("token").unwrap_or(&value)
    };
    let access = json_str(root, &["access_token", "accessToken"])
        .ok_or_else(|| "missing access_token".to_owned())?;
    let refresh = json_str(root, &["refresh_token", "refreshToken"]);
    Ok((access, refresh))
}

pub fn bearer_token(provider: &Provider) -> String {
    let active = crate::accounts::normalize_email(&provider.active_email);
    if !active.is_empty() {
        if let Some(slot) = provider.oauth_accounts.iter().find(|item| {
            crate::accounts::normalize_email(&item.email) == active && !item.token.is_empty()
        }) {
            return slot.token.clone();
        }
    }
    if provider.auth_mode == AuthMode::Oauth && !provider.oauth_token.is_empty() {
        provider.oauth_token.clone()
    } else {
        provider.api_key.clone()
    }
}

pub fn provider_endpoint(provider: &Provider) -> String {
    let base = provider.base_url.trim_end_matches('/');
    match provider.wire {
        WireFormat::Responses => {
            if base.ends_with("/responses") {
                base.to_owned()
            } else if base.ends_with("/v1") {
                format!("{base}/responses")
            } else {
                format!("{base}/v1/responses")
            }
        }
        WireFormat::AnthropicMessages => {
            if base.ends_with("/v1/messages") {
                base.to_owned()
            } else if base.ends_with("/v1") {
                format!("{base}/messages")
            } else {
                format!("{base}/v1/messages")
            }
        }
        WireFormat::ChatCompletions => {
            if base.ends_with("/chat/completions") {
                base.to_owned()
            } else if base.ends_with("/v1") {
                format!("{base}/chat/completions")
            } else {
                format!("{base}/v1/chat/completions")
            }
        }
    }
}

/// OpenAI-compatible, Responses, or Anthropic Messages request. Does not touch grok_bot.
pub fn provider_chat_request(
    provider: &Provider,
    model: &str,
    message: &str,
) -> (String, Vec<(String, String)>, serde_json::Value) {
    provider_chat_request_messages(
        provider,
        model,
        &[crate::compact::ChatTurn::new("user", message)],
    )
}

pub fn provider_chat_request_messages(
    provider: &Provider,
    model: &str,
    turns: &[crate::compact::ChatTurn],
) -> (String, Vec<(String, String)>, serde_json::Value) {
    provider_chat_request_messages_mode(provider, model, turns, 1, false, &[], ToolFlags::default())
}

pub fn provider_chat_request_messages_mode(
    provider: &Provider,
    model: &str,
    turns: &[crate::compact::ChatTurn],
    mode: i32,
    is_subagent: bool,
    images: &[(String, String)],
    flags: ToolFlags,
) -> (String, Vec<(String, String)>, serde_json::Value) {
    let bare = model.split('[').next().unwrap_or(model);
    let mapped = apply_model_map(&provider.model_map, bare);
    let (row_effort, row_context) = row_knobs(provider, bare);
    let id_effort = model.find('[').and_then(|open| {
        model[open + 1..].find(']').and_then(|rel| {
            model[open + 1..open + 1 + rel].split(',').find_map(|part| {
                let mut kv = part.splitn(2, '=');
                let key = kv.next()?.trim();
                let value = kv.next()?.trim();
                matches!(key, "effort" | "reasoning").then(|| value.to_owned())
            })
        })
    });
    let effort = id_effort.as_deref().or(row_effort.as_deref());
    let context = row_context.as_deref();
    let url = provider_endpoint(provider);
    let token = bearer_token(provider);
    let openai_tools = filter_host_tools(cursor_host_tools_openai(), mode, is_subagent, flags);
    let anthropic_tools = filter_host_tools(cursor_host_tools_anthropic(), mode, is_subagent, flags);
    let style = match provider.wire {
        WireFormat::AnthropicMessages => ImageStyle::Anthropic,
        WireFormat::Responses => ImageStyle::Responses,
        WireFormat::ChatCompletions => ImageStyle::Chat,
    };
    let mut messages: Vec<serde_json::Value> = turns
        .iter()
        .map(|turn| {
            let mut msg = serde_json::json!({
                "role": turn.role,
                "content": turn.text,
            });
            if !turn.images.is_empty() {
                attach_images_on(&mut msg, &turn.images, style);
            }
            msg
        })
        .collect();
    messages = attach_user_images_styled(messages, images, style);
    match provider.wire {
        WireFormat::AnthropicMessages => {
            let max_tokens = anthropic_max_tokens(effort, context);
            let mut body = serde_json::json!({
                "model": mapped,
                "max_tokens": max_tokens,
                "stream": true,
                "messages": messages,
                "tools": anthropic_tools,
            });
            if let Some(budget) = anthropic_thinking_budget(effort) {
                debug_assert!(max_tokens > budget);
                body["thinking"] = serde_json::json!({
                    "type": "enabled",
                    "budget_tokens": budget,
                });
            }
            (
                url,
                vec![
                    ("x-api-key".into(), token),
                    ("anthropic-version".into(), "2023-06-01".into()),
                    ("content-type".into(), "application/json".into()),
                ],
                body,
            )
        }
        WireFormat::Responses => {
            let mut body = serde_json::json!({
                "model": mapped,
                "stream": true,
                "input": messages,
            });
            if let Some(effort) = effort.filter(|s| !s.is_empty()) {
                body["reasoning"] = serde_json::json!({ "effort": effort });
            }
            if let Some(cap) = output_token_cap(context) {
                body["max_output_tokens"] = cap.into();
            }
            (
                url,
                vec![
                    ("authorization".into(), format!("Bearer {token}")),
                    ("content-type".into(), "application/json".into()),
                ],
                body,
            )
        }
        WireFormat::ChatCompletions => {
            let mut body = serde_json::json!({
                "model": mapped,
                "stream": true,
                "messages": messages,
                "tools": openai_tools,
                "tool_choice": "auto",
            });
            if provider.kind == ProviderKind::Xai {
                if let Some(effort) = effort.filter(|s| !s.is_empty()) {
                    body["reasoning_effort"] = serde_json::Value::String(effort.to_owned());
                    body["reasoning"] = serde_json::json!({ "effort": effort });
                }
            }
            if let Some(cap) = output_token_cap(context) {
                body["max_completion_tokens"] = cap.into();
            }
            (
                url,
                vec![
                    ("authorization".into(), format!("Bearer {token}")),
                    ("content-type".into(), "application/json".into()),
                ],
                body,
            )
        }
    }
}

const ASK_MODE_EXCLUDED: &[&str] = &[
    "Edit",
    "Write",
    "Delete",
    "Task",
    "Subagent",
    "EditNotebook",
    "GenerateImage",
    "SwitchMode",
    "CreatePlan",
];
const SUBAGENT_ONLY: &[&str] = &["updateCurrentStep"];

#[derive(Clone, Copy)]
pub struct ToolFlags {
    pub web_search: bool,
    pub web_fetch: bool,
    pub read_lints: bool,
}

impl Default for ToolFlags {
    fn default() -> Self {
        Self {
            web_search: true,
            web_fetch: true,
            read_lints: true,
        }
    }
}

#[allow(dead_code)]
pub(crate) fn filter_host_tools_for_mode(
    tools: serde_json::Value,
    mode: i32,
    is_subagent: bool,
) -> serde_json::Value {
    filter_host_tools(tools, mode, is_subagent, ToolFlags::default())
}

pub(crate) fn filter_host_tools(
    tools: serde_json::Value,
    mode: i32,
    is_subagent: bool,
    flags: ToolFlags,
) -> serde_json::Value {
    let Some(array) = tools.as_array() else {
        return tools;
    };
    let filtered: Vec<serde_json::Value> = array
        .iter()
        .filter(|tool| {
            let name = tool
                .pointer("/function/name")
                .and_then(|v| v.as_str())
                .or_else(|| tool.get("name").and_then(|v| v.as_str()))
                .unwrap_or("");
            if !is_subagent && SUBAGENT_ONLY.contains(&name) {
                return false;
            }
            if !flags.web_search && name == "WebSearch" {
                return false;
            }
            if !flags.web_fetch && name == "WebFetch" {
                return false;
            }
            if !flags.read_lints && name == "ReadLints" {
                return false;
            }
            match mode {
                2 => !ASK_MODE_EXCLUDED.contains(&name),
                3 => true,
                4 => name != "SwitchMode" && name != "CreatePlan",
                _ => name != "CreatePlan",
            }
        })
        .cloned()
        .collect();
    serde_json::Value::Array(filtered)
}

#[derive(Clone, Copy)]
enum ImageStyle {
    Chat,
    Anthropic,
    Responses,
}

fn attach_user_images_styled(
    mut messages: Vec<serde_json::Value>,
    images: &[(String, String)],
    style: ImageStyle,
) -> Vec<serde_json::Value> {
    if images.is_empty() {
        return messages;
    }
    let Some(target) = messages.iter_mut().rev().find(|msg| {
        msg["role"] == "user"
            && match msg.get("content") {
                Some(serde_json::Value::String(text)) => !text.contains("<tool_result"),
                Some(serde_json::Value::Array(_)) => true,
                _ => false,
            }
    }) else {
        return messages;
    };
    attach_images_on(target, images, style);
    messages
}

fn attach_images_on(target: &mut serde_json::Value, images: &[(String, String)], style: ImageStyle) {
    if images.is_empty() {
        return;
    }
    let text = match &target["content"] {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(parts) => parts
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .or_else(|| part.get("input_text"))
                    .and_then(|v| v.as_str())
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    };
    let mut existing_images: Vec<(String, String)> = Vec::new();
    if let serde_json::Value::Array(parts) = &target["content"] {
        for part in parts {
            match style {
                ImageStyle::Chat => {
                    if let Some(url) = part
                        .get("image_url")
                        .and_then(|v| v.get("url"))
                        .and_then(|v| v.as_str())
                    {
                        if let Some((mime, data)) = parse_data_url(url) {
                            existing_images.push((mime, data));
                        }
                    }
                }
                ImageStyle::Anthropic => {
                    if part.get("type").and_then(|v| v.as_str()) == Some("image") {
                        let mime = part["source"]["media_type"]
                            .as_str()
                            .unwrap_or("image/png")
                            .to_owned();
                        let data = part["source"]["data"].as_str().unwrap_or("").to_owned();
                        if !data.is_empty() {
                            existing_images.push((mime, data));
                        }
                    }
                }
                ImageStyle::Responses => {
                    if let Some(url) = part.get("image_url").and_then(|v| v.as_str()) {
                        if let Some((mime, data)) = parse_data_url(url) {
                            existing_images.push((mime, data));
                        }
                    }
                }
            }
        }
    }
    let mut all = existing_images;
    for image in images {
        if !all.iter().any(|existing| existing == image) {
            all.push(image.clone());
        }
    }
    target["content"] = image_content_parts(&text, &all, style);
}

fn parse_data_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("data:")?;
    let (mime, data) = rest.split_once(";base64,")?;
    Some((mime.to_owned(), data.to_owned()))
}

fn image_content_parts(
    text: &str,
    images: &[(String, String)],
    style: ImageStyle,
) -> serde_json::Value {
    let mut parts = Vec::new();
    match style {
        ImageStyle::Chat => {
            parts.push(serde_json::json!({"type": "text", "text": text}));
            for (mime, data) in images {
                parts.push(serde_json::json!({
                    "type": "image_url",
                    "image_url": { "url": format!("data:{mime};base64,{data}") }
                }));
            }
        }
        ImageStyle::Anthropic => {
            for (mime, data) in images {
                parts.push(serde_json::json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": mime,
                        "data": data
                    }
                }));
            }
            parts.push(serde_json::json!({"type": "text", "text": text}));
        }
        ImageStyle::Responses => {
            parts.push(serde_json::json!({"type": "input_text", "text": text}));
            for (mime, data) in images {
                parts.push(serde_json::json!({
                    "type": "input_image",
                    "image_url": format!("data:{mime};base64,{data}")
                }));
            }
        }
    }
    serde_json::Value::Array(parts)
}

fn cursor_host_tools_anthropic() -> serde_json::Value {
    let openai = cursor_host_tools_openai();
    let Some(array) = openai.as_array() else {
        return serde_json::json!([]);
    };
    serde_json::Value::Array(
        array
            .iter()
            .filter_map(|tool| {
                let function = tool.get("function")?;
                Some(serde_json::json!({
                    "name": function.get("name")?,
                    "description": function.get("description")?,
                    "input_schema": function.get("parameters")?,
                }))
            })
            .collect(),
    )
}

pub(crate) fn cursor_host_tools_openai() -> serde_json::Value {
    let tools = [
        (
            "Read",
            "Read a file. path is absolute.",
            serde_json::json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": {"type": "string"},
                    "offset": {"type": "integer"},
                    "limit": {"type": "integer"}
                }
            }),
        ),
        (
            "Grep",
            "Search file contents with a regex.",
            serde_json::json!({
                "type": "object",
                "required": ["pattern"],
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": "string"},
                    "glob": {"type": "string"}
                }
            }),
        ),
        (
            "Glob",
            "Find files by glob pattern.",
            serde_json::json!({
                "type": "object",
                "required": ["glob_pattern"],
                "properties": {
                    "glob_pattern": {"type": "string"},
                    "target_directory": {"type": "string"}
                }
            }),
        ),
        (
            "Ls",
            "List a directory.",
            serde_json::json!({
                "type": "object",
                "required": ["path"],
                "properties": {"path": {"type": "string"}}
            }),
        ),
        (
            "Shell",
            "Run a shell command.",
            serde_json::json!({
                "type": "object",
                "required": ["command"],
                "properties": {
                    "command": {"type": "string"},
                    "working_directory": {"type": "string"}
                }
            }),
        ),
        (
            "Edit",
            "Replace text in a file.",
            serde_json::json!({
                "type": "object",
                "required": ["path", "old_string", "new_string"],
                "properties": {
                    "path": {"type": "string"},
                    "old_string": {"type": "string"},
                    "new_string": {"type": "string"},
                    "replace_all": {"type": "boolean"}
                }
            }),
        ),
        (
            "Write",
            "Write a file.",
            serde_json::json!({
                "type": "object",
                "required": ["path", "contents"],
                "properties": {
                    "path": {"type": "string"},
                    "contents": {"type": "string"}
                }
            }),
        ),
        (
            "Delete",
            "Delete a file. path is absolute.",
            serde_json::json!({
                "type": "object",
                "required": ["path"],
                "properties": {"path": {"type": "string"}}
            }),
        ),
        (
            "EditNotebook",
            "Edit a jupyter notebook cell.",
            serde_json::json!({
                "type": "object",
                "required": ["target_notebook", "cell_idx", "is_new_cell", "cell_language", "new_string"],
                "properties": {
                    "target_notebook": {"type": "string"},
                    "cell_idx": {"type": "integer"},
                    "is_new_cell": {"type": "boolean"},
                    "cell_language": {"type": "string"},
                    "old_string": {"type": "string"},
                    "new_string": {"type": "string"}
                }
            }),
        ),
        (
            "ApplyPatch",
            "Apply a *** Begin Patch *** hunk to a file.",
            serde_json::json!({
                "type": "object",
                "required": ["patch"],
                "properties": {"patch": {"type": "string"}}
            }),
        ),
        (
            "TodoWrite",
            "Update the session todo list.",
            serde_json::json!({
                "type": "object",
                "required": ["todos"],
                "properties": {
                    "todos": {"type": "array"},
                    "merge": {"type": "boolean"}
                }
            }),
        ),
        (
            "ReadLints",
            "Read linter diagnostics.",
            serde_json::json!({
                "type": "object",
                "properties": {"paths": {"type": "array", "items": {"type": "string"}}}
            }),
        ),
        (
            "WebSearch",
            "Search the web.",
            serde_json::json!({
                "type": "object",
                "required": ["search_term"],
                "properties": {
                    "search_term": {"type": "string"},
                    "explanation": {"type": "string"}
                }
            }),
        ),
        (
            "WebFetch",
            "Fetch a URL as markdown.",
            serde_json::json!({
                "type": "object",
                "required": ["url"],
                "properties": {"url": {"type": "string"}}
            }),
        ),
        (
            "GenerateImage",
            "Generate an image file from a description. Only when the user asks for an image.",
            serde_json::json!({
                "type": "object",
                "required": ["description"],
                "properties": {
                    "description": {"type": "string"},
                    "filename": {"type": "string"},
                    "reference_image_paths": {"type": "array", "items": {"type": "string"}}
                }
            }),
        ),
        (
            "AskQuestion",
            "Ask the user a question with options.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "questions": {"type": "array"}
                }
            }),
        ),
        (
            "Task",
            "Launch a subagent.",
            serde_json::json!({
                "type": "object",
                "required": ["description", "prompt"],
                "properties": {
                    "description": {"type": "string"},
                    "prompt": {"type": "string"},
                    "subagent_type": {"type": "string"}
                }
            }),
        ),
        (
            "ListMcpResources",
            "List MCP resources. Optional server filter.",
            serde_json::json!({
                "type": "object",
                "properties": {"server": {"type": "string"}}
            }),
        ),
        (
            "FetchMcpResource",
            "Read an MCP resource by server and uri.",
            serde_json::json!({
                "type": "object",
                "required": ["server", "uri"],
                "properties": {
                    "server": {"type": "string"},
                    "uri": {"type": "string"},
                    "downloadPath": {"type": "string"}
                }
            }),
        ),
        (
            "SwitchMode",
            "Switch agent mode (plan|agent).",
            serde_json::json!({
                "type": "object",
                "required": ["target_mode_id"],
                "properties": {
                    "target_mode_id": {"type": "string"},
                    "explanation": {"type": "string"}
                }
            }),
        ),
        (
            "CallDynamicTool",
            "Call an MCP or cursor-namespace tool discovered via GetDynamicTools.",
            serde_json::json!({
                "type": "object",
                "required": ["toolName"],
                "properties": {
                    "namespace": {"type": "string"},
                    "toolName": {"type": "string"},
                    "arguments": {"type": "object"}
                }
            }),
        ),
        (
            "GetDynamicTools",
            "List MCP tool schemas.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "namespace": {"type": "string"},
                    "toolName": {"type": "string"},
                    "pattern": {"type": "string"}
                }
            }),
        ),
        (
            "CreatePlan",
            "Create a markdown plan file.",
            serde_json::json!({
                "type": "object",
                "required": ["plan"],
                "properties": {
                    "name": {"type": "string"},
                    "overview": {"type": "string"},
                    "plan": {"type": "string"},
                    "todos": {"type": "array"}
                }
            }),
        ),
        (
            "Await",
            "Wait for a background task.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "task_id": {"type": "string"},
                    "block_until_ms": {"type": "integer"}
                }
            }),
        ),
        (
            "updateCurrentStep",
            "Report subagent progress.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "current_step": {"type": "string"},
                    "final_summary": {"type": "string"},
                    "completed_subtitle": {"type": "string"}
                }
            }),
        ),
    ];
    serde_json::Value::Array(
        tools
            .into_iter()
            .map(|(name, description, parameters)| {
                serde_json::json!({
                    "type": "function",
                    "function": { "name": name, "description": description, "parameters": parameters }
                })
            })
            .collect(),
    )
}

pub fn tool_call_xml(name: &str, id: &str, arguments: &str) -> String {
    let args: serde_json::Value =
        serde_json::from_str(arguments).unwrap_or_else(|_| serde_json::json!({ "raw": arguments }));
    format!(
        "<tool_call>{}</tool_call>",
        serde_json::json!({ "name": name, "id": id, "arguments": args })
    )
}

/// OpenAI-compatible streamed tool_calls (xAI / generic ChatCompletions).
pub fn parse_openai_tool_deltas(data: &str) -> Vec<(usize, String, String, String)> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(data.trim()) else {
        return Vec::new();
    };
    let Some(calls) = value
        .pointer("/choices/0/delta/tool_calls")
        .and_then(|v| v.as_array())
    else {
        return Vec::new();
    };
    calls
        .iter()
        .enumerate()
        .filter_map(|(fallback, call)| {
            let index = call
                .get("index")
                .and_then(|v| v.as_u64())
                .unwrap_or(fallback as u64) as usize;
            let id = call
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let name = call
                .pointer("/function/name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let arguments = call
                .pointer("/function/arguments")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            if id.is_empty() && name.is_empty() && arguments.is_empty() {
                return None;
            }
            Some((index, id, name, arguments))
        })
        .collect()
}

pub fn parse_openai_tool_delta(data: &str) -> Option<(usize, String, String, String)> {
    parse_openai_tool_deltas(data).into_iter().next()
}

pub fn parse_provider_sse_usage(data: &str) -> Option<(i32, i32, i32)> {
    let data = data.trim();
    if data.is_empty() || data == "[DONE]" {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(data).ok()?;
    let usage = value.get("usage")?;
    let prompt = usage
        .get("prompt_tokens")
        .or_else(|| usage.get("input_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;
    let completion = usage
        .get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;
    let reasoning = usage
        .pointer("/completion_tokens_details/reasoning_tokens")
        .or_else(|| usage.get("reasoning_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;
    if prompt == 0 && completion == 0 && reasoning == 0 {
        return None;
    }
    Some((prompt, completion, reasoning))
}

pub fn parse_provider_sse_thinking(
    kind: &ProviderKind,
    wire: WireFormat,
    data: &str,
) -> Option<String> {
    let data = data.trim();
    if data.is_empty() || data == "[DONE]" {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(data).ok()?;
    match wire {
        WireFormat::Responses => value
            .pointer("/delta/reasoning")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .or_else(|| {
                value
                    .pointer("/delta/reasoning_content")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
            }),
        _ => match kind {
            ProviderKind::Anthropic if wire == WireFormat::AnthropicMessages => {
                if value.get("type").and_then(|v| v.as_str()) == Some("content_block_delta") {
                    value
                        .pointer("/delta/thinking")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned)
                } else {
                    None
                }
            }
            _ => value
                .pointer("/choices/0/delta/reasoning_content")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    value
                        .pointer("/choices/0/delta/reasoning")
                        .and_then(|v| v.as_str())
                })
                .or_else(|| {
                    value
                        .pointer("/choices/0/delta/thinking")
                        .and_then(|v| v.as_str())
                })
                .map(str::to_owned),
        },
    }
}

pub fn parse_provider_sse_data(
    kind: &ProviderKind,
    wire: WireFormat,
    data: &str,
) -> Option<String> {
    let data = data.trim();
    if data.is_empty() || data == "[DONE]" {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(data).ok()?;
    match wire {
        WireFormat::Responses => value
            .get("delta")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .or_else(|| {
                value
                    .pointer("/delta/text")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
            })
            .or_else(|| {
                value
                    .pointer("/response/output_text")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
            }),
        WireFormat::AnthropicMessages | WireFormat::ChatCompletions => match kind {
            ProviderKind::Anthropic if wire == WireFormat::AnthropicMessages => {
                if value.get("type").and_then(|v| v.as_str()) == Some("content_block_delta") {
                    value
                        .pointer("/delta/text")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned)
                } else {
                    None
                }
            }
            _ => value
                .pointer("/choices/0/delta/content")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .or_else(|| {
                    value
                        .pointer("/choices/0/message/content")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned)
                }),
        },
    }
}

/// xAI device-code OAuth, lifted from cursor-byok grok-auth (client_id public).
/// Access JWT (`at+jwt`) is typically 6 hours (`exp - iat`). `offline_access`
/// is requested so the token endpoint also returns `refresh_token`.
pub const XAI_OAUTH_CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
pub const XAI_DEVICE_CODE_URL: &str = "https://auth.x.ai/oauth2/device/code";
pub const XAI_TOKEN_URL: &str = "https://auth.x.ai/oauth2/token";

pub fn xai_device_begin_body() -> String {
    format!(
        "client_id={}&scope=openid+profile+email+offline_access+grok-cli%3Aaccess+api%3Aaccess",
        XAI_OAUTH_CLIENT_ID
    )
}

pub fn xai_device_poll_body(device_code: &str) -> String {
    format!(
        "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code&client_id={}&device_code={}&scope=openid+profile+email+offline_access+grok-cli%3Aaccess+api%3Aaccess",
        XAI_OAUTH_CLIENT_ID,
        urlencoding_lite(device_code)
    )
}

fn urlencoding_lite(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn parse_xai_device_begin(json: &str) -> Result<(String, String, String), String> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let device = value
        .get("device_code")
        .and_then(|v| v.as_str())
        .ok_or("missing device_code")?;
    let user = value
        .get("user_code")
        .and_then(|v| v.as_str())
        .ok_or("missing user_code")?;
    let url = value
        .get("verification_uri_complete")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("verification_uri").and_then(|v| v.as_str()))
        .ok_or("missing verification_uri")?;
    Ok((device.to_owned(), user.to_owned(), url.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(kind: ProviderKind, key: &str, base: &str) -> Provider {
        Provider {
            id: format!("{}-1", kind.as_str()),
            kind,
            label: "t".into(),
            base_url: base.into(),
            api_key: key.into(),
            wire: kind.default_wire(),
            ..Provider::default()
        }
    }

    #[test]
    fn identity_map_does_not_invent_max_knobs() {
        let row = identity_map("grok-4.6");
        assert!(row.context.is_none());
        assert!(row.effort.is_none());
        assert!(row.efforts.is_empty());
        let mapped = maps_for_picked(&["grok-4.6".into()], &[]);
        assert_eq!(mapped.len(), 1);
        assert!(mapped[0].efforts.is_empty());
        assert!(mapped[0].context.is_none());
        assert!(mapped[0].fast.is_none());
    }

    #[test]
    fn thinking_grades_match_ccursor_palette() {
        assert_eq!(
            THINKING_GRADES,
            &["none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra"]
        );
        assert_eq!(effort_display("none"), "None");
        assert_eq!(effort_display("ultra"), "Ultra");
        assert_eq!(effort_display("xhigh"), "Extra High");
    }

    #[test]
    fn official_grok_does_not_auto_enable_max_or_ultra() {
        let family = crate::sand::SandFamily {
            id: "grok-4.6".into(),
            context_token_limit: Some(256000),
            axes: vec![
                crate::sand::ParamAxis {
                    id: "effort".into(),
                    name: "Effort".into(),
                    r#enum: Some(vec![
                        "low".into(),
                        "medium".into(),
                        "high".into(),
                        "xhigh".into(),
                    ]),
                    labels: None,
                    bool: false,
                },
                crate::sand::ParamAxis {
                    id: "fast".into(),
                    name: "Fast".into(),
                    r#enum: None,
                    labels: None,
                    bool: true,
                },
            ],
            ..crate::sand::SandFamily::default()
        };
        let mut row = identity_map("grok-4.6");
        apply_official_knobs(&mut row, &[family]);
        assert_eq!(
            row.efforts,
            vec![
                "low".to_string(),
                "medium".into(),
                "high".into(),
                "xhigh".into()
            ]
        );
        assert!(!row
            .efforts
            .iter()
            .any(|item| item == "max" || item == "ultra" || item == "none"));
        assert_eq!(row.fast, Some(true));
        assert_eq!(row.context.as_deref(), Some("256000"));
    }

    #[test]
    fn records_openai_and_xai_without_dropping_others() {
        let mut list = Vec::new();
        upsert_provider(
            &mut list,
            sample(
                ProviderKind::Openai,
                "sk-test",
                ProviderKind::Openai.default_base_url(),
            ),
        );
        upsert_provider(
            &mut list,
            sample(
                ProviderKind::Xai,
                "xai-test",
                ProviderKind::Xai.default_base_url(),
            ),
        );
        upsert_provider(
            &mut list,
            Provider {
                id: "generic-1".into(),
                kind: ProviderKind::Generic,
                label: "BYOK".into(),
                base_url: "https://example.invalid/v1".into(),
                api_key: "k".into(),
                ..Provider::default()
            },
        );
        assert_eq!(list.len(), 3);
        upsert_provider(
            &mut list,
            Provider {
                id: "openai-1".into(),
                kind: ProviderKind::Openai,
                label: "OpenAI".into(),
                base_url: ProviderKind::Openai.default_base_url().into(),
                api_key: "sk-rotated".into(),
                ..Provider::default()
            },
        );
        assert_eq!(list.len(), 3);
        assert_eq!(
            list.iter().find(|p| p.id == "openai-1").unwrap().api_key,
            "sk-rotated"
        );
    }

    #[test]
    fn wire_formats_change_url_and_body() {
        let mut chat = sample(ProviderKind::Openai, "sk-test", "https://api.openai.com/v1");
        chat.effort = Some("high".into());
        chat.context = Some("128000".into());
        chat.model_map = vec![ModelMap {
            alias: "gpt-4o".into(),
            upstream: "gpt-4o-2024-11-20".into(),
            ..ModelMap::default()
        }];
        let (url, _, body) = provider_chat_request(&chat, "gpt-4o", "ping");
        assert!(url.ends_with("/v1/chat/completions"));
        assert_eq!(body["model"], "gpt-4o-2024-11-20");
        assert!(body.get("reasoning_effort").is_none());
        assert!(body.get("context").is_none());
        assert_eq!(body["messages"][0]["content"], "ping");

        chat.wire = WireFormat::Responses;
        let (url, _, body) = provider_chat_request(&chat, "gpt-4o", "ping");
        assert!(url.ends_with("/v1/responses"));
        assert_eq!(body["model"], "gpt-4o-2024-11-20");
        assert_eq!(body["reasoning"]["effort"], "high");
        assert!(body.get("context").is_none());
        assert!(body.get("max_output_tokens").is_none());
        assert_eq!(body["input"][0]["content"], "ping");
        assert!(body.get("messages").is_none());

        chat.context = Some("8192".into());
        let (_, _, body) = provider_chat_request(&chat, "gpt-4o", "ping");
        assert_eq!(body["max_output_tokens"], 8192);

        let mut anth = sample(
            ProviderKind::Anthropic,
            "anth-test",
            ProviderKind::Anthropic.default_base_url(),
        );
        anth.wire = WireFormat::AnthropicMessages;
        anth.effort = Some("high".into());
        let (url, headers, body) = provider_chat_request(&anth, "claude-sonnet-4-5", "ping");
        assert!(url.ends_with("/v1/messages"));
        assert!(headers
            .iter()
            .any(|(n, v)| n == "x-api-key" && v == "anth-test"));
        let budget = body["thinking"]["budget_tokens"].as_u64().unwrap();
        let max = body["max_tokens"].as_u64().unwrap();
        assert_eq!(budget, 8192);
        assert!(max > budget, "max_tokens {max} must exceed budget {budget}");
        assert!(body.get("context").is_none());
    }

    #[test]
    fn anthropic_and_responses_attach_user_images() {
        let turns = [crate::compact::ChatTurn::new("user", "see this")];
        let images = vec![("image/png".into(), "abc".into())];
        let mut anth = sample(
            ProviderKind::Anthropic,
            "anth-test",
            ProviderKind::Anthropic.default_base_url(),
        );
        anth.wire = WireFormat::AnthropicMessages;
        let (_, _, body) = provider_chat_request_messages_mode(
            &anth,
            "claude-sonnet-4-5",
            &turns,
            1,
            false,
            &images,
            ToolFlags::default(),
        );
        let content = body["messages"][0]["content"].as_array().expect("blocks");
        assert!(
            content.iter().any(|part| part["type"] == "image"
                && part["source"]["media_type"] == "image/png"),
            "{content:?}"
        );

        let mut resp = sample(ProviderKind::Openai, "sk-test", "https://api.openai.com/v1");
        resp.wire = WireFormat::Responses;
        let (_, _, body) = provider_chat_request_messages_mode(
            &resp,
            "gpt-4o",
            &turns,
            1,
            false,
            &images,
            ToolFlags::default(),
        );
        let content = body["input"][0]["content"].as_array().expect("blocks");
        assert!(
            content
                .iter()
                .any(|part| part["type"] == "input_image"),
            "{content:?}"
        );
    }

    #[test]
    fn history_turn_images_stay_on_that_turn() {
        let mut prior = crate::compact::ChatTurn::new("user", "old [image image/png]");
        prior.images = vec![("image/png".into(), "hist".into())];
        let current = crate::compact::ChatTurn::new("user", "<user_query>\nnow\n</user_query>");
        let turns = [prior, current];
        let mut p = sample(ProviderKind::Openai, "sk-test", "https://api.openai.com/v1");
        p.wire = WireFormat::ChatCompletions;
        let (_, _, body) = provider_chat_request_messages_mode(
            &p,
            "gpt-4o",
            &turns,
            1,
            false,
            &[("image/jpeg".into(), "now".into())],
            ToolFlags::default(),
        );
        let hist = body["messages"][0]["content"].as_array().expect("hist");
        assert!(
            hist.iter().any(|part| part["type"] == "image_url"
                && part["image_url"]["url"]
                    .as_str()
                    .is_some_and(|url| url.contains("hist"))),
            "{hist:?}"
        );
        let cur = body["messages"][1]["content"].as_array().expect("cur");
        assert!(
            cur.iter().any(|part| part["type"] == "image_url"
                && part["image_url"]["url"]
                    .as_str()
                    .is_some_and(|url| url.contains("now"))),
            "{cur:?}"
        );
    }

    #[test]
    fn chat_model_for_provider_prefers_selected() {
        let mut p = sample(ProviderKind::Openai, "k", "https://api.openai.com/v1");
        assert_eq!(chat_model_for_provider(&p), "gpt-4o");
        p.selected_model = "gpt-4.1-mini".into();
        assert_eq!(chat_model_for_provider(&p), "gpt-4.1-mini");
    }

    #[test]
    fn anthropic_budget_never_meets_or_exceeds_max_tokens() {
        for effort in [
            None,
            Some("low"),
            Some("medium"),
            Some("high"),
            Some("xhigh"),
            Some("max"),
        ] {
            let budget = anthropic_thinking_budget(effort).unwrap_or(0);
            let max = anthropic_max_tokens(effort, Some("128000"));
            assert!(max > budget, "effort={effort:?} max={max} budget={budget}");
            let unset = sample(ProviderKind::Openai, "k", "https://api.openai.com/v1");
            let (_, _, body) = provider_chat_request(&unset, "gpt-4o", "x");
            assert!(body.get("reasoning_effort").is_none());
            assert!(body.get("max_completion_tokens").is_none());
        }
    }

    #[test]
    fn row_knobs_prefer_per_model_over_provider() {
        let mut p = sample(ProviderKind::Openai, "k", "https://api.openai.com/v1");
        p.effort = Some("low".into());
        p.context = Some("8192".into());
        p.model_map = vec![ModelMap {
            alias: "kimi-k3".into(),
            upstream: "kimi-k3".into(),
            effort: Some("xhigh".into()),
            context: Some("1000000".into()),
            ..ModelMap::default()
        }];
        let (effort, ctx) = row_knobs(&p, "kimi-k3");
        assert_eq!(effort.as_deref(), Some("xhigh"));
        assert_eq!(ctx.as_deref(), Some("1000000"));
        let (effort, ctx) = row_knobs(&p, "other");
        assert_eq!(effort.as_deref(), Some("low"));
        assert_eq!(ctx.as_deref(), Some("8192"));
    }

    #[test]
    fn optional_knob_omits_blank() {
        assert_eq!(optional_knob(""), None);
        assert_eq!(optional_knob("  "), None);
        assert_eq!(optional_knob("high"), Some("high".into()));
    }

    #[test]
    fn parse_xai_token_json_reads_refresh() {
        let json = r#"{"access_token":"at-1","refresh_token":"rt-1"}"#;
        let (access, refresh) = parse_xai_token_json(json).unwrap();
        assert_eq!(access, "at-1");
        assert_eq!(refresh.as_deref(), Some("rt-1"));
        let camel = r#"{"accessToken":"at-2","refreshToken":"rt-2"}"#;
        let (access, refresh) = parse_xai_token_json(camel).unwrap();
        assert_eq!(access, "at-2");
        assert_eq!(refresh.as_deref(), Some("rt-2"));
    }

    #[test]
    fn upsert_provider_does_not_wipe_refresh() {
        let mut list = vec![sample(ProviderKind::Xai, "sk-old", "https://api.x.ai/v1")];
        list[0].id = "xai-xAI".into();
        list[0].oauth_refresh = "keep-me".into();
        let mut incoming = list[0].clone();
        incoming.oauth_token = "new-at".into();
        incoming.oauth_refresh.clear();
        upsert_provider(&mut list, incoming);
        assert_eq!(list[0].oauth_refresh, "keep-me");
        assert_eq!(list[0].oauth_token, "new-at");
    }

    #[test]
    fn merge_provider_secrets_fills_empty_refresh_from_file() {
        let mut prefs = sample(ProviderKind::Xai, "sk", "https://api.x.ai/v1");
        prefs.id = "xai-xAI".into();
        let mut file = prefs.clone();
        file.oauth_refresh = "from-file".into();
        let merged = merge_provider_secrets(vec![prefs], vec![file]);
        assert_eq!(merged[0].oauth_refresh, "from-file");
    }

    #[test]
    fn xai_oauth_creates_provider_account_without_email_claim() {
        let mut list = Vec::new();
        apply_xai_oauth_tokens(
            &mut list,
            "eyJhbGciOiJub25lIn0.eyJzdWIiOiJ4YWktc3ViLTEifQ.".into(),
            Some("rt".into()),
        );
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].oauth_accounts.len(), 1);
        assert!(!list[0].oauth_accounts[0].email.is_empty());
    }

    #[test]
    fn bearer_expired_is_false_without_exp() {
        assert!(!bearer_expired("not-a-jwt"));
    }

    #[test]
    fn oauth_bearer_prefers_oauth_token() {
        let mut p = sample(ProviderKind::Xai, "sk-old", "https://api.x.ai/v1");
        p.auth_mode = AuthMode::Oauth;
        p.oauth_token = "oauth-live".into();
        let (_, headers, _) = provider_chat_request(&p, "grok-3", "hi");
        assert!(headers
            .iter()
            .any(|(n, v)| n == "authorization" && v == "Bearer oauth-live"));
    }

    #[test]
    fn parse_catalog_and_map() {
        let ids = parse_model_catalog(r#"{"data":[{"id":"gpt-4o"},{"id":"gpt-4o-mini"}]}"#);
        assert_eq!(ids, vec!["gpt-4o".to_string(), "gpt-4o-mini".to_string()]);
        let map = parse_model_map("gpt-4o=gpt-4o-2024-11-20\n# skip\nbadline\n");
        assert_eq!(map.len(), 1);
        assert_eq!(apply_model_map(&map, "gpt-4o"), "gpt-4o-2024-11-20");
        assert_eq!(apply_model_map(&map, "other"), "other");
        assert_eq!(
            models_list_url("https://api.openai.com/v1"),
            "https://api.openai.com/v1/models"
        );
        let xai = vec!["grok-4.6".into(), "grok-3".into()];
        let leftover = parse_model_map("gpt-4o=gpt-4o-2024-11-20\ngrok-4.6=grok-4.6");
        let merged = merge_catalog_maps(&xai, &leftover);
        assert_eq!(merged.len(), 2);
        assert!(merged.iter().all(|m| m.alias != "gpt-4o"));
        assert_eq!(
            merged
                .iter()
                .find(|m| m.alias == "grok-4.6")
                .unwrap()
                .upstream,
            "grok-4.6"
        );
        assert!(!visible_provider(&Provider {
            id: "default".into(),
            label: "default".into(),
            ..Provider::default()
        }));
    }

    #[test]
    fn oauth_portal_fails_closed_for_generic() {
        assert!(oauth_portal_url(ProviderKind::Openai).is_ok());
        assert!(oauth_portal_url(ProviderKind::Xai).is_ok());
        assert!(oauth_portal_url(ProviderKind::Generic).is_err());
        assert!(supports_oauth(ProviderKind::Openai));
        assert!(!supports_oauth(ProviderKind::Generic));
    }

    #[test]
    fn merge_fills_knobs_for_new_catalog_ids() {
        let catalog = vec!["grok-4.6".into(), "grok-3".into()];
        let leftover = parse_model_map("gpt-4o=gpt-4o-2024-11-20");
        let merged = merge_catalog_maps(&catalog, &leftover);
        assert_eq!(merged.len(), 2);
        assert!(merged.iter().all(|m| m.alias != "gpt-4o"));
        let grok = merged.iter().find(|m| m.alias == "grok-4.6").unwrap();
        assert_eq!(grok.upstream, "grok-4.6");
        assert!(grok.context.is_none());
        assert!(row_efforts(grok).is_empty());
    }

    #[test]
    fn maps_for_picked_skips_unchecked_catalog_ids() {
        let catalog = vec!["grok-4.6".into(), "grok-3".into(), "grok-4.5".into()];
        let picked = vec!["grok-4.6".into()];
        let maps = maps_for_picked(&picked, &merge_catalog_maps(&catalog, &[]));
        assert_eq!(maps.len(), 1);
        assert_eq!(maps[0].alias, "grok-4.6");
        assert!(row_efforts(&maps[0]).is_empty());
    }

    #[test]
    fn merge_matches_alias_only_so_remap_does_not_steal_other_ids() {
        let catalog = vec!["grok-3".into(), "grok-4.6".into()];
        let existing = vec![ModelMap {
            alias: "grok-3".into(),
            upstream: "grok-4.6".into(),
            ..ModelMap::default()
        }];
        let merged = merge_catalog_maps(&catalog, &existing);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].alias, "grok-3");
        assert_eq!(merged[0].upstream, "grok-4.6");
        assert_eq!(merged[1].alias, "grok-4.6");
        assert_eq!(merged[1].upstream, "grok-4.6");
    }

    #[test]
    fn xai_device_bodies_are_form_encoded() {
        let begin = xai_device_begin_body();
        assert!(begin.contains("client_id="));
        assert!(begin.contains("scope="));
        let poll = xai_device_poll_body("abc+def");
        assert!(poll.contains("device_code="));
        assert!(poll.contains("grant_type="));
        let parsed = parse_xai_device_begin(
            r#"{"device_code":"dc","user_code":"WD4K","verification_uri":"https://auth.x.ai/device"}"#,
        )
        .expect("begin");
        assert_eq!(parsed.0, "dc");
        assert_eq!(parsed.1, "WD4K");
    }

    #[test]
    fn parse_openai_and_anthropic_sse() {
        let openai = parse_provider_sse_data(
            &ProviderKind::Openai,
            WireFormat::ChatCompletions,
            r#"{"choices":[{"delta":{"content":"pong"}}]}"#,
        );
        assert_eq!(openai.as_deref(), Some("pong"));
        assert!(parse_provider_sse_data(
            &ProviderKind::Openai,
            WireFormat::ChatCompletions,
            "[DONE]"
        )
        .is_none());
        let anth = parse_provider_sse_data(
            &ProviderKind::Anthropic,
            WireFormat::AnthropicMessages,
            r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"hi"}}"#,
        );
        assert_eq!(anth.as_deref(), Some("hi"));
        let resp = parse_provider_sse_data(
            &ProviderKind::Openai,
            WireFormat::Responses,
            r#"{"type":"response.output_text.delta","delta":"ok"}"#,
        );
        assert_eq!(resp.as_deref(), Some("ok"));
        let thinking = parse_provider_sse_thinking(
            &ProviderKind::Xai,
            WireFormat::ChatCompletions,
            r#"{"choices":[{"delta":{"reasoning_content":"plan"}}]}"#,
        );
        assert_eq!(thinking.as_deref(), Some("plan"));
        assert_eq!(
            parse_provider_sse_usage(
                r#"{"usage":{"prompt_tokens":40500,"completion_tokens":120,"completion_tokens_details":{"reasoning_tokens":80}}}"#,
            ),
            Some((40500, 120, 80))
        );
        let tool = parse_openai_tool_delta(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"Read","arguments":"{\"path\":\"README.md\"}"}}]}}]}"#,
        );
        assert_eq!(tool.as_ref().map(|t| t.2.as_str()), Some("Read"));
        assert_eq!(
            tool_call_xml("Read", "call_1", r#"{"path":"README.md"}"#).contains("README.md"),
            true
        );
    }

    #[test]
    fn openai_and_xai_chat_requests_do_not_touch_grok_bot_shape() {
        let openai = sample(
            ProviderKind::Openai,
            "sk-test",
            ProviderKind::Openai.default_base_url(),
        );
        let (url, headers, body) = provider_chat_request(&openai, "gpt-4o", "ping");
        assert!(url.ends_with("/v1/chat/completions"));
        assert_eq!(
            headers
                .iter()
                .find(|(n, _)| n == "authorization")
                .map(|(_, v)| v.as_str()),
            Some("Bearer sk-test")
        );
        assert_eq!(body["messages"][0]["content"], "ping");
        assert!(
            body["tools"].as_array().is_some_and(|tools| {
                tools
                    .iter()
                    .any(|tool| tool.pointer("/function/name") == Some(&serde_json::json!("Read")))
            }),
            "ChatCompletions must advertise Cursor host tools"
        );
        let xai = sample(
            ProviderKind::Xai,
            "xai-test",
            ProviderKind::Xai.default_base_url(),
        );
        let (url, _, xai_body) = provider_chat_request(&xai, "grok-3", "hi");
        assert!(url.contains("api.x.ai"));
        assert!(xai_body["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty()));
    }

    #[test]
    fn anthropic_chat_request_uses_messages_api() {
        let anth = sample(
            ProviderKind::Anthropic,
            "anth-test",
            ProviderKind::Anthropic.default_base_url(),
        );
        let (url, headers, body) = provider_chat_request(&anth, "claude-sonnet-4-5", "ping");
        assert!(url.ends_with("/v1/messages"));
        assert!(headers
            .iter()
            .any(|(n, v)| n == "x-api-key" && v == "anth-test"));
        assert_eq!(body["messages"][0]["content"], "ping");
    }

    #[test]
    fn cursor_host_tools_cover_ccursor_registry() {
        let catalog = cursor_host_tools_openai();
        let names: Vec<_> = catalog
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool.pointer("/function/name")?.as_str())
            .collect();
        assert!(names.contains(&"CallDynamicTool"));
        assert!(!names.contains(&"CallMcpTool"));
        let no_search = filter_host_tools(
            catalog.clone(),
            1,
            false,
            ToolFlags {
                web_search: false,
                web_fetch: true,
                read_lints: true,
            },
        );
        let no_search_names: Vec<_> = no_search
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool.pointer("/function/name")?.as_str())
            .collect();
        assert!(!no_search_names.contains(&"WebSearch"));
        for need in [
            "Read",
            "Grep",
            "Glob",
            "Ls",
            "Shell",
            "Edit",
            "Write",
            "Delete",
            "EditNotebook",
            "ApplyPatch",
            "TodoWrite",
            "ReadLints",
            "WebSearch",
            "WebFetch",
            "GenerateImage",
            "AskQuestion",
            "Task",
            "ListMcpResources",
            "FetchMcpResource",
            "SwitchMode",
            "CallDynamicTool",
            "GetDynamicTools",
            "CreatePlan",
            "Await",
            "updateCurrentStep",
        ] {
            assert!(names.contains(&need), "missing native tool {need} in {names:?}");
        }
        assert!(names.len() >= 24, "{}", names.len());
    }

    #[test]
    fn ask_mode_drops_write_tools() {
        let catalog = filter_host_tools_for_mode(cursor_host_tools_openai(), 2, false);
        let names: Vec<_> = catalog
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool.pointer("/function/name")?.as_str())
            .collect();
        for banned in ["Edit", "Write", "Delete", "Task", "CreatePlan", "SwitchMode"] {
            assert!(!names.contains(&banned), "{banned} still in ask tools {names:?}");
        }
        assert!(names.contains(&"Read"));
        assert!(!names.contains(&"updateCurrentStep"));
    }

    #[test]
    fn plan_mode_keeps_create_plan() {
        let catalog = filter_host_tools_for_mode(cursor_host_tools_openai(), 3, false);
        let names: Vec<_> = catalog
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool.pointer("/function/name")?.as_str())
            .collect();
        assert!(names.contains(&"CreatePlan"));
        assert!(names.contains(&"Edit"));
    }

    #[test]
    fn subagent_keeps_update_current_step() {
        let catalog = filter_host_tools_for_mode(cursor_host_tools_openai(), 1, true);
        let names: Vec<_> = catalog
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool.pointer("/function/name")?.as_str())
            .collect();
        assert!(names.contains(&"updateCurrentStep"));
        assert!(!names.contains(&"CreatePlan"));
    }
}
