use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{Error, Result};

pub const SAND_BACKEND: &str = "https://api2.cursor.sh";
pub const LOGIN_ORIGIN: &str = "https://cursor.com";
/// Grok Bot desktop is still 0.47.0, but api2 rejects that as "outdated Cursor".
/// Stamp the installed Cursor version instead (currently 3.19.x).
pub fn sand_client_version() -> String {
    static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    VERSION
        .get_or_init(|| {
            let path = std::env::var_os("LOCALAPPDATA")
                .map(std::path::PathBuf::from)
                .map(|p| p.join("Programs/cursor/resources/app/package.json"));
            if let Some(path) = path {
                if let Ok(raw) = std::fs::read_to_string(path) {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
                        if let Some(version) = value.get("version").and_then(|v| v.as_str()) {
                            if !version.is_empty() && version != "0.47.0" {
                                return version.to_owned();
                            }
                        }
                    }
                }
            }
            "3.20.17".into()
        })
        .clone()
}
pub const OAUTH_CLIENT_ID: &str = "KbZUR41cY7W6zRSdpSUJ7I7mLYBKOCmB";

const CURSOR_ISS: &str = "https://authentication.cursor.sh";
const CURSOR_AUD: &str = "https://cursor.com";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub machine_id: String,
    pub display_name: String,
    pub sand_state: Option<String>,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub exhausted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota: Option<crate::quota::QuotaSnapshot>,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            access_token: String::new(),
            refresh_token: None,
            machine_id: String::new(),
            display_name: String::new(),
            sand_state: None,
            email: String::new(),
            exhausted: false,
            last_error: None,
            quota: None,
        }
    }
}

impl crate::accounts::SlotId for Account {
    fn slot_id(&self) -> String {
        account_id(self)
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

pub fn jwt_account_id(token: &str) -> Option<String> {
    jwt_email(token)
}

pub fn account_id(account: &Account) -> String {
    if !account.email.is_empty() {
        crate::accounts::normalize_email(&account.email)
    } else if let Some(email) = jwt_email(&account.access_token) {
        email
    } else {
        crate::accounts::normalize_email(&account.display_name)
    }
}

pub fn fill_account_identity(account: &mut Account) {
    if account.email.is_empty() {
        if let Some(email) = jwt_email(&account.access_token) {
            account.email = email;
        }
    } else {
        account.email = crate::accounts::normalize_email(&account.email);
    }
    if let Some(name) = jwt_display_name(&account.access_token) {
        if account.display_name.is_empty() || account.display_name == "Grok Bot" {
            account.display_name = name;
        }
    }
    if account.email.is_empty() {
        account.email = crate::accounts::normalize_email(&account.display_name);
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicAccount {
    pub signed_in: bool,
    #[serde(default)]
    pub exhausted: bool,
    pub display_name: Option<String>,
    pub machine_id_prefix: Option<String>,
    pub sand_state: Option<String>,
    pub exp: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ImportBody {
    #[serde(alias = "accessToken", alias = "cursor-access-token")]
    access_token: String,
    #[serde(alias = "refreshToken", alias = "cursor-refresh-token")]
    refresh_token: Option<String>,
    #[serde(alias = "machineId", alias = "cursor-machine-id")]
    machine_id: String,
    #[serde(alias = "displayName")]
    display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandFamily {
    pub id: String,
    pub display_name: String,
    pub context_token_limit: Option<u64>,
    #[serde(default)]
    pub context_token_limit_for_max_mode: Option<u64>,
    pub thinking: bool,
    #[serde(default)]
    pub supports_max_mode: bool,
    #[serde(default = "default_true")]
    pub supports_non_max_mode: bool,
    #[serde(default)]
    pub supports_images: bool,
    pub axes: Vec<ParamAxis>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_variant: Option<String>,
    /// Stream-usable on the grok_bot allowlist (E-019). Not a reason to drop the row.
    #[serde(default)]
    pub available: bool,
}

fn default_true() -> bool {
    true
}

impl Default for SandFamily {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            context_token_limit: None,
            context_token_limit_for_max_mode: None,
            thinking: false,
            supports_max_mode: false,
            supports_non_max_mode: true,
            supports_images: false,
            axes: Vec::new(),
            default_variant: None,
            available: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamAxis {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#enum: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    pub bool: bool,
}

pub fn is_effort_axis_id(id: &str) -> bool {
    matches!(id, "effort" | "reasoning" | "reasoning_effort")
}

impl SandFamily {
    pub fn effort_axis(&self) -> Option<&ParamAxis> {
        self.axes.iter().find(|axis| is_effort_axis_id(&axis.id))
    }

    pub fn effort_values(&self) -> Vec<String> {
        self.effort_axis()
            .and_then(|axis| axis.r#enum.clone())
            .unwrap_or_default()
    }

    pub fn has_fast_axis(&self) -> bool {
        self.axes.iter().any(|axis| axis.id == "fast" && axis.bool)
    }

    pub fn default_effort(&self) -> Option<String> {
        if let Some(pairs) = self.default_variant.as_deref().and_then(parse_variant_pairs) {
            if let Some((_, value)) = pairs.into_iter().find(|(id, _)| is_effort_axis_id(id)) {
                return Some(value);
            }
        }
        let values = self.effort_values();
        if values.iter().any(|item| item == "high") {
            Some("high".into())
        } else {
            values.into_iter().next()
        }
    }

    pub fn default_fast(&self) -> bool {
        if let Some(pairs) = self.default_variant.as_deref().and_then(parse_variant_pairs) {
            return pairs
                .iter()
                .any(|(id, value)| id == "fast" && value == "true");
        }
        self.has_fast_axis()
    }

    pub fn token_windows(&self) -> Vec<u64> {
        let mut out = Vec::new();
        if let Some(n) = self.context_token_limit {
            out.push(n);
        }
        if let Some(n) = self.context_token_limit_for_max_mode {
            if !out.contains(&n) {
                out.push(n);
            }
        }
        out
    }

    pub fn knob_summary(&self) -> String {
        let mut parts = Vec::new();
        for n in self.token_windows() {
            parts.push(format_tokens(n));
        }
        let max_mode_extra = self.supports_max_mode
            && self
                .context_token_limit
                .zip(self.context_token_limit_for_max_mode)
                .is_some_and(|(a, b)| a != b);
        if max_mode_extra {
            parts.push("Max Mode".into());
        }
        for axis in &self.axes {
            if axis.bool {
                parts.push(axis.name.clone());
            } else if let Some(values) = &axis.r#enum {
                let labels: Vec<String> = values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        axis.labels
                            .as_ref()
                            .and_then(|labels| labels.get(index))
                            .cloned()
                            .filter(|label| !label.is_empty())
                            .unwrap_or_else(|| axis_value_label(value))
                    })
                    .collect();
                parts.push(format!("{} {}", axis.name, labels.join("/")));
            }
        }
        if parts.is_empty() {
            "官方目录无额外旋钮".into()
        } else {
            parts.join(" · ")
        }
    }
}

pub fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 && n % 1_000_000 == 0 {
        format!("{}M", n / 1_000_000)
    } else if n >= 1000 && n % 1000 == 0 {
        format!("{}K", n / 1000)
    } else {
        n.to_string()
    }
}

pub fn axis_value_label(value: &str) -> String {
    match value {
        "none" => "None".into(),
        "minimal" => "Minimal".into(),
        "low" => "Low".into(),
        "medium" => "Medium".into(),
        "high" => "High".into(),
        "xhigh" => "Extra High".into(),
        "max" => "Max".into(),
        "ultra" => "Ultra".into(),
        "auto" => "Auto".into(),
        other => other.to_owned(),
    }
}

pub fn parse_variant_pairs(repr: &str) -> Option<Vec<(String, String)>> {
    let start = repr.find('[')?;
    let end = repr.rfind(']')?;
    if end <= start {
        return None;
    }
    let inner = &repr[start + 1..end];
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    Some(
        inner
            .split(',')
            .filter_map(|part| {
                let (id, value) = part.split_once('=')?;
                Some((id.trim().to_owned(), value.trim().to_owned()))
            })
            .collect(),
    )
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginStart {
    pub id: String,
    pub login_url: String,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone)]
pub struct LoginSession {
    pub uuid: String,
    pub verifier: String,
    pub machine_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelParameter {
    pub id: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDetails {
    pub model_id: String,
    pub parameters: Vec<ModelParameter>,
}

pub fn b64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn decode_jwt_payload(token: &str) -> Option<serde_json::Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn is_cursor_session_jwt(token: &str) -> bool {
    let Some(payload) = decode_jwt_payload(token) else {
        return false;
    };
    if payload.get("iss").and_then(|v| v.as_str()) == Some("https://auth.x.ai") {
        return false;
    }
    payload.get("iss").and_then(|v| v.as_str()) == Some(CURSOR_ISS)
        && payload.get("aud").and_then(|v| v.as_str()) == Some(CURSOR_AUD)
        && payload.get("type").and_then(|v| v.as_str()) == Some("session")
}

pub fn is_grok_bot_jwt(token: &str) -> bool {
    let Some(payload) = decode_jwt_payload(token) else {
        return false;
    };
    payload.get("iss").and_then(|v| v.as_str()) == Some(CURSOR_ISS)
        && payload.get("aud").and_then(|v| v.as_str()) == Some(CURSOR_AUD)
        && payload.get("type").and_then(|v| v.as_str()) == Some("grok_bot")
}

/// Stream Authorization is the short-lived `grokBotToken` (type=grok_bot).
/// Desktop Cursor session JWTs and inference `accessToken` (type=session) both
/// return ERROR_NOT_LOGGED_IN on InferenceService/Stream.
pub fn stream_headers(
    account: &Account,
    grok_bot_token: &str,
    now_ms: u128,
) -> Result<Vec<(String, String)>> {
    if grok_bot_token.is_empty() {
        return Err(Error::Msg(
            "Stream fail-closed: missing grokBotToken".into(),
        ));
    }
    if is_cursor_session_jwt(grok_bot_token) {
        return Err(Error::Msg(
            "Stream refuses Cursor session JWT (use grokBotToken)".into(),
        ));
    }
    if !is_grok_bot_jwt(grok_bot_token) {
        return Err(Error::Msg(
            "Stream fail-closed: bearer is not grok_bot".into(),
        ));
    }
    let mut headers = sand_headers(account, now_ms);
    for (name, value) in &mut headers {
        if name == "authorization" {
            *value = format!("Bearer {grok_bot_token}");
        }
    }
    Ok(headers)
}

pub fn jwt_email(token: &str) -> Option<String> {
    let payload = decode_jwt_payload(token)?;
    payload
        .get("email")
        .and_then(|v| v.as_str())
        .map(crate::accounts::normalize_email)
        .filter(|value| value.contains('@'))
}

pub fn jwt_display_name(token: &str) -> Option<String> {
    let payload = decode_jwt_payload(token)?;
    ["email", "preferred_username", "name", "sub"]
        .into_iter()
        .find_map(|key| payload.get(key).and_then(|v| v.as_str()).map(str::to_owned))
}

pub fn jwt_exp(token: &str) -> Option<u64> {
    decode_jwt_payload(token)?
        .get("exp")
        .and_then(|v| v.as_u64())
}

pub fn cursor_checksum(machine_id: &str, now_ms: u128) -> String {
    let ts = now_ms / 1_000_000;
    let mut buf = [
        ((ts >> 40) & 255) as u8,
        ((ts >> 32) & 255) as u8,
        ((ts >> 24) & 255) as u8,
        ((ts >> 16) & 255) as u8,
        ((ts >> 8) & 255) as u8,
        (ts & 255) as u8,
    ];
    let mut rolling = 165u8;
    for (index, byte) in buf.iter_mut().enumerate() {
        *byte = (*byte ^ rolling).wrapping_add((index % 256) as u8);
        rolling = *byte;
    }
    format!("{}{machine_id}", b64url(&buf))
}

pub fn sand_identity_headers(account: &Account, now_ms: u128) -> Vec<(String, String)> {
    vec![
        (
            "authorization".into(),
            format!("Bearer {}", account.access_token),
        ),
        ("x-cursor-client-type".into(), "sand".into()),
        ("x-cursor-client-version".into(), sand_client_version()),
        ("x-cursor-client-source".into(), "sand-desktop".into()),
        ("x-sand-box-namespace".into(), "prod".into()),
        (
            "x-cursor-checksum".into(),
            cursor_checksum(&account.machine_id, now_ms),
        ),
        ("x-ghost-mode".into(), "true".into()),
    ]
}

pub fn sand_headers(account: &Account, now_ms: u128) -> Vec<(String, String)> {
    let mut headers = vec![
        ("content-type".into(), "application/json".into()),
        ("connect-protocol-version".into(), "1".into()),
    ];
    headers.extend(sand_identity_headers(account, now_ms));
    headers
}

/// Grok Bot inference allowlist from live Stream (E-019): grok/composer/gemini/haiku.
/// Catalog lists many more families; Stream returns ERROR_NOT_HIGH_ENOUGH_PERMISSIONS.
pub fn grok_bot_inference_allowed(model_id: &str) -> bool {
    let stripped = model_id.strip_prefix("gb-").unwrap_or(model_id);
    let family = family_from_model_id(stripped).to_ascii_lowercase();
    family.starts_with("grok")
        || family.starts_with("composer")
        || family.starts_with("gemini")
        || family.starts_with("claude-haiku")
}

pub fn family_from_model_id(model_id: &str) -> String {
    let last = model_id.rsplit('/').next().unwrap_or(model_id);
    let last = last.strip_prefix("gb-").unwrap_or(last);
    let last = last.split('[').next().unwrap_or(last);
    let stripped = last.strip_prefix("cursor-").unwrap_or(last);
    let re = [
        "-low-fast",
        "-medium-fast",
        "-high-fast",
        "-xhigh-fast",
        "-max-fast",
        "-low",
        "-medium",
        "-high",
        "-xhigh",
        "-max",
    ];
    for suffix in re {
        if let Some(prefix) = stripped.strip_suffix(suffix) {
            return prefix.to_owned();
        }
    }
    stripped.to_owned()
}

pub fn model_details_for(model_id: &str, effort: Option<&str>, fast: bool) -> ModelDetails {
    model_details_for_knobs(model_id, effort, fast, None)
}

pub fn model_details_for_knobs(
    model_id: &str,
    effort: Option<&str>,
    fast: bool,
    context: Option<&str>,
) -> ModelDetails {
    model_details_from_axes(model_id, &fallback_axes(model_id), effort, fast, context)
}

pub fn model_details_from_family(
    family: &SandFamily,
    effort: Option<&str>,
    fast: bool,
) -> ModelDetails {
    model_details_from_axes(&family.id, &family.axes, effort, fast, None)
}

/// Wire fallback when the live catalog row is not in hand.
/// Grok / Composer match AvailableModels 047 (`effort`+`fast` / `fast`). Other
/// families send nothing invented — no `context` token-count parameter.
fn fallback_axes(model_id: &str) -> Vec<ParamAxis> {
    let family = family_from_model_id(model_id.strip_prefix("gb-").unwrap_or(model_id))
        .to_ascii_lowercase();
    if family.contains("grok") {
        vec![
            ParamAxis {
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
            ParamAxis {
                id: "fast".into(),
                name: "Fast".into(),
                r#enum: None,
                labels: None,
                bool: true,
            },
        ]
    } else if family.contains("composer") {
        vec![ParamAxis {
            id: "fast".into(),
            name: "Fast".into(),
            r#enum: None,
            labels: None,
            bool: true,
        }]
    } else {
        Vec::new()
    }
}

pub fn model_details_from_axes(
    model_id: &str,
    axes: &[ParamAxis],
    effort: Option<&str>,
    fast: bool,
    context: Option<&str>,
) -> ModelDetails {
    let family = family_from_model_id(model_id.strip_prefix("gb-").unwrap_or(model_id));
    let mut parameters = Vec::new();
    for axis in axes {
        if axis.bool {
            let value = if axis.id == "fast" {
                if fast { "true" } else { "false" }
            } else {
                "false"
            };
            parameters.push(ModelParameter {
                id: axis.id.clone(),
                value: value.into(),
            });
            continue;
        }
        let Some(values) = &axis.r#enum else {
            continue;
        };
        if values.is_empty() {
            continue;
        }
        let chosen = if is_effort_axis_id(&axis.id) {
            effort
                .filter(|value| *value != "none" && values.iter().any(|item| item == value))
                .map(str::to_owned)
                .or_else(|| values.iter().find(|item| *item == "high").cloned())
                .or_else(|| values.first().cloned())
        } else if axis.id == "context" {
            context
                .filter(|value| !value.is_empty() && *value != "none" && values.iter().any(|item| item == value))
                .map(str::to_owned)
        } else {
            None
        };
        if let Some(value) = chosen {
            parameters.push(ModelParameter {
                id: axis.id.clone(),
                value,
            });
        }
    }
    ModelDetails {
        model_id: family,
        parameters,
    }
}

pub fn public_account(account: Option<&Account>) -> PublicAccount {
    match account {
        None => PublicAccount {
            signed_in: false,
            exhausted: false,
            display_name: None,
            machine_id_prefix: None,
            sand_state: None,
            exp: None,
        },
        Some(account) => PublicAccount {
            signed_in: true,
            exhausted: account.exhausted,
            display_name: Some(
                jwt_display_name(&account.access_token)
                    .unwrap_or_else(|| account.display_name.clone()),
            ),
            machine_id_prefix: Some(account.machine_id.chars().take(8).collect()),
            sand_state: account.sand_state.clone(),
            exp: jwt_exp(&account.access_token),
        },
    }
}

pub fn import_account(body: ImportBody) -> Result<Account> {
    if !is_cursor_session_jwt(&body.access_token) {
        return Err(Error::Msg(
            "not a Cursor session JWT (refusing xAI grok-auth files)".into(),
        ));
    }
    if body.machine_id.trim().is_empty() {
        return Err(Error::Msg(
            "missing machineId (needed for x-cursor-checksum)".into(),
        ));
    }
    let mut account = Account {
        display_name: body
            .display_name
            .or_else(|| jwt_display_name(&body.access_token))
            .unwrap_or_else(|| "Grok Bot".into()),
        access_token: body.access_token,
        refresh_token: body.refresh_token.filter(|value| !value.is_empty()),
        machine_id: body.machine_id,
        ..Account::default()
    };
    fill_account_identity(&mut account);
    Ok(account)
}

pub fn begin_login() -> (LoginStart, LoginSession) {
    let verifier_raw: [u8; 32] = rand_bytes();
    let verifier = b64url(&verifier_raw);
    let challenge = b64url(&Sha256::digest(verifier.as_bytes()));
    let uuid = Uuid::new_v4().to_string();
    let machine_id = Uuid::new_v4().to_string();
    let login_url = format!(
        "{LOGIN_ORIGIN}/loginDeepControl?challenge={challenge}&uuid={uuid}&mode=login&redirectTarget=sand&supportsSelectedTeamLogin=true"
    );
    let start = LoginStart {
        id: uuid.clone(),
        login_url,
        expires_at_ms: now_ms() + 5 * 60 * 1000,
    };
    let session = LoginSession {
        uuid,
        verifier,
        machine_id,
    };
    (start, session)
}

pub fn parse_sand_families(body: &serde_json::Value) -> Vec<SandFamily> {
    let Some(models) = body.get("models").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for model in models {
        let id = model
            .get("name")
            .and_then(|v| v.as_str())
            .or_else(|| model.get("serverModelName").and_then(|v| v.as_str()));
        let Some(id) = id else { continue };
        if !seen.insert(id.to_owned()) {
            continue;
        }
        let axes = model
            .get("parameterDefinitions")
            .and_then(|v| v.as_array())
            .map(|defs| {
                defs.iter()
                    .filter_map(|def| {
                        let id = def.get("id")?.as_str()?.to_owned();
                        let name = def
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&id)
                            .to_owned();
                        let enum_pairs = def
                            .pointer("/parameterType/enumParameter/values")
                            .and_then(|v| v.as_array())
                            .map(|values| {
                                values
                                    .iter()
                                    .filter_map(|item| {
                                        let value = item.get("value")?.as_str()?.to_owned();
                                        let label = item
                                            .get("displayName")
                                            .and_then(|v| v.as_str())
                                            .map(str::to_owned)
                                            .filter(|s| !s.is_empty());
                                        Some((value, label))
                                    })
                                    .collect::<Vec<_>>()
                            });
                        let (enum_values, labels) = match enum_pairs {
                            Some(pairs) if !pairs.is_empty() => {
                                let values: Vec<String> =
                                    pairs.iter().map(|(value, _)| value.clone()).collect();
                                let labels: Vec<String> = pairs
                                    .iter()
                                    .map(|(value, label)| {
                                        label.clone().unwrap_or_else(|| axis_value_label(value))
                                    })
                                    .collect();
                                (Some(values), Some(labels))
                            }
                            _ => (None, None),
                        };
                        let bool_axis = def
                            .pointer("/parameterType/booleanParameter/values")
                            .and_then(|v| v.as_array())
                            .is_some();
                        Some(ParamAxis {
                            id,
                            name,
                            r#enum: enum_values,
                            labels,
                            bool: bool_axis,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let default_variant = model
            .get("variants")
            .and_then(|v| v.as_array())
            .and_then(|vars| {
                vars.iter()
                    .find(|item| {
                        item.get("isDefaultNonMaxConfig")
                            .and_then(|v| v.as_bool())
                            == Some(true)
                    })
                    .or_else(|| {
                        vars.iter().find(|item| {
                            item.get("isDefaultMaxConfig").and_then(|v| v.as_bool())
                                == Some(true)
                        })
                    })
                    .or_else(|| vars.first())
                    .and_then(|item| {
                        item.get("variantStringRepresentation")
                            .and_then(|v| v.as_str())
                            .map(str::to_owned)
                    })
            });
        out.push(SandFamily {
            display_name: model
                .get("clientDisplayName")
                .and_then(|v| v.as_str())
                .unwrap_or(id)
                .to_owned(),
            context_token_limit: model.get("contextTokenLimit").and_then(|v| v.as_u64()),
            context_token_limit_for_max_mode: model
                .get("contextTokenLimitForMaxMode")
                .and_then(|v| v.as_u64()),
            thinking: model.get("supportsThinking").and_then(|v| v.as_bool()) == Some(true),
            supports_max_mode: model.get("supportsMaxMode").and_then(|v| v.as_bool())
                == Some(true),
            supports_non_max_mode: model
                .get("supportsNonMaxMode")
                .and_then(|v| v.as_bool())
                != Some(false),
            supports_images: model.get("supportsImages").and_then(|v| v.as_bool()) == Some(true),
            axes,
            default_variant,
            id: id.to_owned(),
            available: false,
        });
    }
    present_catalog(out)
}

/// Sort by display name (then id), tag Stream-usable families. Never drops rows.
pub fn present_catalog(mut families: Vec<SandFamily>) -> Vec<SandFamily> {
    for family in &mut families {
        family.available = grok_bot_inference_allowed(&family.id);
    }
    families.sort_by(|left, right| {
        left.display_name
            .to_ascii_lowercase()
            .cmp(&right.display_name.to_ascii_lowercase())
            .then_with(|| left.id.to_ascii_lowercase().cmp(&right.id.to_ascii_lowercase()))
    });
    families
}

pub fn first_available_id(families: &[SandFamily]) -> Option<String> {
    families
        .iter()
        .find(|family| family.available || grok_bot_inference_allowed(&family.id))
        .map(|family| family.id.clone())
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn rand_bytes() -> [u8; 32] {
    let uuid = Uuid::new_v4();
    let uuid2 = Uuid::new_v4();
    let mut out = [0u8; 32];
    out[..16].copy_from_slice(uuid.as_bytes());
    out[16..].copy_from_slice(uuid2.as_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jwt(payload: serde_json::Value) -> String {
        let body = b64url(payload.to_string().as_bytes());
        format!("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.{body}.sig")
    }

    #[test]
    fn rejects_xai_tokens() {
        let token = jwt(serde_json::json!({
            "iss": "https://auth.x.ai",
            "typ": "at+jwt"
        }));
        assert!(!is_cursor_session_jwt(&token));
    }

    #[test]
    fn accepts_cursor_session() {
        let token = jwt(serde_json::json!({
            "iss": "https://authentication.cursor.sh",
            "aud": "https://cursor.com",
            "type": "session",
            "sub": "google-oauth2|user"
        }));
        assert!(is_cursor_session_jwt(&token));
    }

    #[test]
    fn parse_sand_families_keeps_catalog_models() {
        let body = serde_json::json!({
            "models": [
                {"name": "claude-opus-4-7", "clientDisplayName": "Opus"},
                {"name": "gpt-5.4", "clientDisplayName": "GPT"},
                {"name": "grok-4.6", "clientDisplayName": "Grok"},
                {"name": "gemini-3.1-pro", "clientDisplayName": "Gemini"}
            ]
        });
        let families = parse_sand_families(&body);
        let ids: Vec<_> = families.iter().map(|f| f.id.as_str()).collect();
        assert!(ids.contains(&"claude-opus-4-7"));
        assert!(ids.contains(&"gpt-5.4"));
        assert_eq!(ids, ["gemini-3.1-pro", "gpt-5.4", "grok-4.6", "claude-opus-4-7"]);
        assert_eq!(families.len(), 4);
        let tagged: Vec<_> = families
            .iter()
            .filter(|family| family.available)
            .map(|family| family.id.as_str())
            .collect();
        assert_eq!(tagged, ["gemini-3.1-pro", "grok-4.6"]);
        assert!(!families.iter().any(|family| family.id.contains("opus") && family.available));
        assert!(!families.iter().any(|family| family.id.starts_with("gpt") && family.available));
    }

    #[test]
    fn present_catalog_sorts_by_name_and_keeps_unavailable() {
        let families = present_catalog(vec![
            SandFamily {
                id: "claude-opus-4-6".into(),
                display_name: "Claude Opus 4.6".into(),
                ..SandFamily::default()
            },
            SandFamily {
                id: "gpt-5.4".into(),
                display_name: "GPT-5.4".into(),
                ..SandFamily::default()
            },
            SandFamily {
                id: "claude-haiku-4-5".into(),
                display_name: "Claude Haiku 4.5".into(),
                ..SandFamily::default()
            },
            SandFamily {
                id: "composer-2.5".into(),
                display_name: "Composer 2.5".into(),
                ..SandFamily::default()
            },
        ]);
        let names: Vec<_> = families.iter().map(|f| f.display_name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Claude Haiku 4.5",
                "Claude Opus 4.6",
                "Composer 2.5",
                "GPT-5.4"
            ]
        );
        assert!(families.iter().any(|f| f.id == "claude-opus-4-6" && !f.available));
        assert!(families.iter().any(|f| f.id == "gpt-5.4" && !f.available));
        assert!(families.iter().any(|f| f.id == "claude-haiku-4-5" && f.available));
        assert!(families.iter().any(|f| f.id == "composer-2.5" && f.available));
        assert_eq!(first_available_id(&families).as_deref(), Some("claude-haiku-4-5"));
    }

    #[test]
    fn grok_bot_inference_allowlist_matches_live_stream() {
        assert!(grok_bot_inference_allowed("grok-4.6"));
        assert!(grok_bot_inference_allowed("gb-gemini-3.1-pro"));
        assert!(grok_bot_inference_allowed("claude-haiku-4-5"));
        assert!(grok_bot_inference_allowed("composer-2.5"));
        assert!(!grok_bot_inference_allowed("claude-opus-4-7"));
        assert!(!grok_bot_inference_allowed("gb-gpt-5.4"));
        assert!(!grok_bot_inference_allowed("kimi-k3"));
        assert!(!grok_bot_inference_allowed("default"));
    }

    #[test]
    fn maps_grok_effort_fast() {
        let details = model_details_for("plugin:x/grok-bot/grok-4.6", Some("xhigh"), true);
        assert_eq!(details.model_id, "grok-4.6");
        assert_eq!(details.parameters[0].id, "effort");
        assert_eq!(details.parameters[0].value, "xhigh");
        assert_eq!(details.parameters[1].value, "true");
    }

    #[test]
    fn grok_stream_knobs_are_effort_and_fast_only() {
        let details = model_details_for_knobs("gb-grok-4.6", Some("medium"), false, Some("128000"));
        let map: std::collections::BTreeMap<_, _> = details
            .parameters
            .iter()
            .map(|p| (p.id.as_str(), p.value.as_str()))
            .collect();
        assert_eq!(details.model_id, "grok-4.6");
        assert_eq!(map.get("effort"), Some(&"medium"));
        assert_eq!(map.get("fast"), Some(&"false"));
        assert!(!map.contains_key("context"));
        assert!(!map.contains_key("reasoning"));
        let haiku = model_details_for_knobs("claude-haiku-4-5", Some("high"), false, Some("200000"));
        assert!(haiku.parameters.is_empty());
    }

    #[test]
    fn parse_sand_families_keeps_official_grok_axes() {
        let body = serde_json::json!({
            "models": [{
                "name": "grok-4.6",
                "clientDisplayName": "Cursor Grok 4.6",
                "contextTokenLimit": 256000,
                "contextTokenLimitForMaxMode": 256000,
                "supportsThinking": true,
                "supportsMaxMode": true,
                "supportsNonMaxMode": true,
                "parameterDefinitions": [
                    {
                        "id": "effort",
                        "name": "Effort",
                        "parameterType": {
                            "enumParameter": {
                                "values": [
                                    {"value": "low"},
                                    {"value": "medium"},
                                    {"value": "high"},
                                    {"value": "xhigh"}
                                ]
                            }
                        }
                    },
                    {
                        "id": "fast",
                        "name": "Fast",
                        "parameterType": {
                            "booleanParameter": {
                                "values": [{"value": "false"}, {"value": "true"}]
                            }
                        }
                    }
                ],
                "variants": [{
                    "isDefaultNonMaxConfig": true,
                    "variantStringRepresentation": "grok-4.6[effort=high,fast=true]"
                }]
            }]
        });
        let families = parse_sand_families(&body);
        assert_eq!(families.len(), 1);
        let grok = &families[0];
        assert_eq!(grok.id, "grok-4.6");
        assert_eq!(grok.context_token_limit, Some(256000));
        assert_eq!(grok.context_token_limit_for_max_mode, Some(256000));
        assert!(grok.supports_max_mode);
        assert_eq!(grok.axes.len(), 2);
        assert_eq!(grok.axes[0].id, "effort");
        assert_eq!(
            grok.axes[0].r#enum.as_ref().unwrap(),
            &vec![
                "low".to_string(),
                "medium".into(),
                "high".into(),
                "xhigh".into()
            ]
        );
        assert!(grok.has_fast_axis());
        assert_eq!(grok.default_effort().as_deref(), Some("high"));
        assert!(grok.default_fast());
        assert!(!grok.effort_values().iter().any(|item| item == "max"));
        let summary = grok.knob_summary();
        assert!(summary.contains("256K"));
        assert!(!summary.contains("500"));
        assert!(!summary.contains("Max Mode"));
    }

    #[test]
    fn client_version_is_not_grok_bot_desktop() {
        assert_ne!(sand_client_version(), "0.47.0");
    }

    #[test]
    fn checksum_is_stable_for_fixed_time() {
        let one = cursor_checksum("64ddef80-09a0-4000-8000-000000000001", 1_788_000_000_000);
        let two = cursor_checksum("64ddef80-09a0-4000-8000-000000000001", 1_788_000_000_000);
        assert_eq!(one, two);
        assert!(one.ends_with("64ddef80-09a0-4000-8000-000000000001"));
    }

    fn sample_account(token: &str) -> Account {
        Account {
            access_token: token.to_owned(),
            machine_id: "64ddef80-09a0-4000-8000-000000000001".into(),
            display_name: "t".into(),
            ..Account::default()
        }
    }

    #[test]
    fn stream_headers_use_grok_bot_not_session() {
        let session = jwt(serde_json::json!({
            "iss": "https://authentication.cursor.sh",
            "aud": "https://cursor.com",
            "type": "session",
            "sandBoxPod": "pod"
        }));
        let grok = jwt(serde_json::json!({
            "iss": "https://authentication.cursor.sh",
            "aud": "https://cursor.com",
            "type": "grok_bot",
            "sandBoxPod": "pod"
        }));
        let account = sample_account(&session);
        assert!(stream_headers(&account, &session, 1_788_000_000_000).is_err());
        let headers = stream_headers(&account, &grok, 1_788_000_000_000).expect("grok_bot");
        let auth = headers
            .iter()
            .find(|(name, _)| name == "authorization")
            .map(|(_, value)| value.as_str())
            .expect("authorization");
        assert!(auth.ends_with(&grok), "stream bearer must be grokBotToken");
        assert!(
            !auth.contains(&session),
            "stream must not fall back to session JWT"
        );
        assert_eq!(
            headers
                .iter()
                .find(|(name, _)| name == "x-cursor-client-type")
                .map(|(_, value)| value.as_str()),
            Some("sand")
        );
    }
}
