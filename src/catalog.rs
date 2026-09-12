use prost::Message;
use serde_json::{json, Value};

use crate::connect::{frame_then_rest, split_first_frame};
use crate::sand::{is_effort_axis_id, parse_variant_pairs, ParamAxis, SandFamily};

#[derive(Clone, PartialEq, Message)]
struct AvailableModelsAddition {
    #[prost(string, repeated, tag = "1")]
    model_names: Vec<String>,
    #[prost(message, repeated, tag = "2")]
    models: Vec<AvailableModel>,
}

#[derive(Clone, PartialEq, Message)]
struct AvailableModel {
    #[prost(string, tag = "1")]
    name: String,
    #[prost(bool, tag = "2")]
    default_on: bool,
    #[prost(bool, optional, tag = "5")]
    supports_agent: Option<bool>,
    #[prost(int32, optional, tag = "6")]
    degradation_status: Option<i32>,
    #[prost(message, optional, tag = "8")]
    tooltip_data: Option<TooltipData>,
    #[prost(bool, optional, tag = "9")]
    supports_thinking: Option<bool>,
    #[prost(bool, optional, tag = "10")]
    supports_images: Option<bool>,
    #[prost(bool, optional, tag = "14")]
    supports_max_mode: Option<bool>,
    #[prost(int32, optional, tag = "15")]
    context_token_limit: Option<i32>,
    #[prost(int32, optional, tag = "16")]
    context_token_limit_for_max_mode: Option<i32>,
    #[prost(string, optional, tag = "17")]
    client_display_name: Option<String>,
    #[prost(string, optional, tag = "18")]
    server_model_name: Option<String>,
    #[prost(bool, optional, tag = "19")]
    supports_non_max_mode: Option<bool>,
    #[prost(message, repeated, tag = "29")]
    parameter_definitions: Vec<ModelParameterDefinition>,
    #[prost(message, repeated, tag = "30")]
    variants: Vec<ModelVariant>,
    #[prost(string, optional, tag = "24")]
    inputbox_short_model_name: Option<String>,
    #[prost(bool, optional, tag = "22")]
    supports_plan_mode: Option<bool>,
    #[prost(string, repeated, tag = "36")]
    legacy_slugs: Vec<String>,
    #[prost(int32, optional, tag = "38")]
    named_model_section_index: Option<i32>,
    #[prost(string, optional, tag = "41")]
    vendor_name: Option<String>,
    #[prost(message, optional, tag = "42")]
    vendor: Option<AvailableModelVendor>,
    #[prost(message, repeated, tag = "48")]
    model_picker_badges: Vec<ModelPickerBadge>,
}

#[derive(Clone, PartialEq, Message)]
struct TooltipData {
    #[prost(string, optional, tag = "7")]
    markdown_content: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
struct AvailableModelVendor {
    #[prost(int32, tag = "1")]
    id: i32,
    #[prost(string, tag = "2")]
    display_name: String,
}

#[derive(Clone, PartialEq, Message)]
struct ModelPickerBadge {
    #[prost(string, tag = "1")]
    label: String,
    #[prost(int32, tag = "2")]
    variant: i32,
    #[prost(bool, tag = "3")]
    dismiss_on_selection: bool,
}

#[derive(Clone, PartialEq, Message)]
struct ModelVariant {
    #[prost(message, repeated, tag = "1")]
    parameter_values: Vec<ModelParameterValue>,
    #[prost(string, tag = "2")]
    display_name: String,
    #[prost(bool, tag = "3")]
    is_max_mode: bool,
    #[prost(bool, optional, tag = "4")]
    is_default_max_config: Option<bool>,
    #[prost(bool, optional, tag = "5")]
    is_default_non_max_config: Option<bool>,
    #[prost(string, optional, tag = "8")]
    display_name_outside_picker: Option<String>,
    #[prost(string, optional, tag = "9")]
    variant_string_representation: Option<String>,
    #[prost(string, optional, tag = "11")]
    legacy_slug: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
struct ModelParameterValue {
    #[prost(string, tag = "1")]
    id: String,
    #[prost(string, tag = "2")]
    value: String,
}

#[derive(Clone, PartialEq, Message)]
struct UsableModelsAddition {
    #[prost(message, repeated, tag = "1")]
    models: Vec<UsableModel>,
}

#[derive(Clone, PartialEq, Message)]
struct ThinkingDetails {}

#[derive(Clone, PartialEq, Message)]
struct UsableModel {
    #[prost(string, tag = "1")]
    model_id: String,
    #[prost(message, optional, tag = "2")]
    thinking_details: Option<ThinkingDetails>,
    #[prost(string, tag = "3")]
    display_model_id: String,
    #[prost(string, tag = "4")]
    display_name: String,
    #[prost(string, tag = "5")]
    display_name_short: String,
    #[prost(bool, optional, tag = "7")]
    max_mode: Option<bool>,
    #[prost(message, optional, tag = "8")]
    api_key_credentials: Option<ApiKeyCredentials>,
}

#[derive(Clone, PartialEq, Message)]
struct ApiKeyCredentials {
    #[prost(string, tag = "1")]
    api_key: String,
}

#[derive(Clone, PartialEq, Message)]
struct ModelParameterDefinition {
    #[prost(string, tag = "1")]
    id: String,
    #[prost(string, tag = "2")]
    name: String,
    #[prost(message, optional, tag = "4")]
    parameter_type: Option<ModelParameterType>,
    #[prost(bool, optional, tag = "5")]
    is_cycleable_by_hotkey: Option<bool>,
}

#[derive(Clone, PartialEq, Message)]
struct ModelParameterType {
    #[prost(message, optional, tag = "1")]
    boolean_parameter: Option<BooleanParameter>,
    #[prost(message, optional, tag = "2")]
    enum_parameter: Option<EnumParameter>,
}

#[derive(Clone, PartialEq, Message)]
struct BooleanParameter {
    #[prost(message, repeated, tag = "1")]
    values: Vec<BooleanParameterValue>,
}

#[derive(Clone, PartialEq, Message)]
struct BooleanParameterValue {
    #[prost(string, tag = "1")]
    value: String,
    #[prost(string, optional, tag = "2")]
    display_name: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
struct EnumParameter {
    #[prost(message, repeated, tag = "1")]
    values: Vec<EnumParameterValue>,
}

#[derive(Clone, PartialEq, Message)]
struct EnumParameterValue {
    #[prost(string, tag = "1")]
    value: String,
    #[prost(string, optional, tag = "2")]
    display_name: Option<String>,
}

fn defs_from_axes(axes: &[ParamAxis]) -> Vec<ModelParameterDefinition> {
    axes.iter()
        .map(|axis| {
            if axis.bool {
                bool_param(&axis.id, &axis.name)
            } else {
                let values = axis.r#enum.clone().unwrap_or_default();
                let pairs: Vec<(String, String)> = values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        let label = axis
                            .labels
                            .as_ref()
                            .and_then(|labels| labels.get(index))
                            .cloned()
                            .filter(|label| !label.is_empty())
                            .unwrap_or_else(|| crate::sand::axis_value_label(value));
                        (value.clone(), label)
                    })
                    .collect();
                enum_param_owned(&axis.id, &axis.name, &pairs)
            }
        })
        .collect()
}

fn enum_param_owned(id: &str, name: &str, values: &[(String, String)]) -> ModelParameterDefinition {
    ModelParameterDefinition {
        id: id.into(),
        name: name.into(),
        is_cycleable_by_hotkey: Some(true),
        parameter_type: Some(ModelParameterType {
            boolean_parameter: None,
            enum_parameter: Some(EnumParameter {
                values: values
                    .iter()
                    .map(|(value, label)| EnumParameterValue {
                        value: value.clone(),
                        display_name: Some(label.clone()),
                    })
                    .collect(),
            }),
        }),
    }
}

fn bool_param(id: &str, name: &str) -> ModelParameterDefinition {
    ModelParameterDefinition {
        id: id.into(),
        name: name.into(),
        is_cycleable_by_hotkey: Some(true),
        parameter_type: Some(ModelParameterType {
            enum_parameter: None,
            boolean_parameter: Some(BooleanParameter {
                values: vec![
                    BooleanParameterValue {
                        value: "false".into(),
                        display_name: None,
                    },
                    BooleanParameterValue {
                        value: "true".into(),
                        display_name: Some(name.into()),
                    },
                ],
            }),
        }),
    }
}

fn provider_suffix(family_id: &str) -> Option<String> {
    let (provider_id, _) = parse_provider_enable_id(family_id)?;
    let suffix = provider_id
        .rsplit(['-', '/'])
        .next()
        .filter(|part| !part.is_empty() && part.chars().any(|ch| ch.is_alphabetic()))
        .unwrap_or(provider_id);
    Some(suffix.to_owned())
}

fn source_tag(family: &SandFamily) -> String {
    provider_suffix(&family.id).unwrap_or_else(|| "Grok Bot".into())
}

fn unsuffixed_name(name: &str) -> &str {
    name.split(" · ").next().unwrap_or(name)
}

fn injected_display(family: &SandFamily) -> String {
    let tag = source_tag(family);
    if family.display_name.contains(&format!(" · {tag}"))
        || (is_provider_enable_id(&family.id) && family.display_name.contains(" · "))
    {
        return family.display_name.clone();
    }
    let base = unsuffixed_name(&family.display_name).trim();
    format!("{base} · {tag}")
}

fn inject_tooltip(family: &SandFamily) -> String {
    if let Some(suffix) = provider_suffix(&family.id) {
        format!("{} via {}.", family.display_name, suffix)
    } else {
        format!("{} via Grok Bot sand session.", family.display_name)
    }
}

fn proto_model_from_family(family: &SandFamily) -> AvailableModel {
    let id = injected_id(&family.id);
    let display = injected_display(family);
    let thinking = family.thinking || family.effort_axis().is_some();
    let parameter_definitions = defs_from_axes(&family.axes);
    let variants = build_variants_from_family(&id, &display, family);
    let legacy_slugs = variants
        .iter()
        .filter_map(|variant| variant.legacy_slug.clone())
        .collect();
    let tooltip = TooltipData {
        markdown_content: Some(inject_tooltip(family)),
    };
    let ctx = family
        .context_token_limit
        .and_then(|n| i32::try_from(n).ok());
    let ctx_max = family
        .context_token_limit_for_max_mode
        .and_then(|n| i32::try_from(n).ok());
    AvailableModel {
        name: id.clone(),
        default_on: true,
        supports_agent: Some(true),
        degradation_status: Some(0),
        tooltip_data: Some(tooltip),
        supports_thinking: Some(thinking),
        supports_images: Some(family.supports_images),
        supports_max_mode: Some(family.supports_max_mode),
        context_token_limit: ctx,
        context_token_limit_for_max_mode: ctx_max,
        client_display_name: Some(display.clone()),
        server_model_name: Some(id),
        supports_non_max_mode: Some(family.supports_non_max_mode),
        parameter_definitions,
        variants,
        inputbox_short_model_name: Some(display),
        supports_plan_mode: Some(true),
        vendor_name: None,
        legacy_slugs,
        named_model_section_index: Some(1),
        vendor: None,
        model_picker_badges: Vec::new(),
    }
}

fn clone_as_injected(official: &AvailableModel, family: &SandFamily) -> AvailableModel {
    let id = injected_id(&family.id);
    let display = injected_display(family);
    let mut cloned = official.clone();
    cloned.name = id.clone();
    cloned.server_model_name = Some(id.clone());
    cloned.client_display_name = Some(display.clone());
    cloned.inputbox_short_model_name = Some(display.clone());
    cloned.vendor_name = None;
    cloned.vendor = None;
    cloned.default_on = true;
    cloned.degradation_status = Some(0);
    cloned.model_picker_badges = Vec::new();
    if let Some(ctx) = family
        .context_token_limit
        .and_then(|n| i32::try_from(n).ok())
    {
        cloned.context_token_limit = Some(ctx);
    }
    if let Some(ctx) = family
        .context_token_limit_for_max_mode
        .and_then(|n| i32::try_from(n).ok())
    {
        cloned.context_token_limit_for_max_mode = Some(ctx);
    }
    cloned.tooltip_data = Some(TooltipData {
        markdown_content: Some(inject_tooltip(family)),
    });
    cloned.legacy_slugs = cloned
        .legacy_slugs
        .iter()
        .map(|slug| rewrite_legacy_slug(slug, &family.id))
        .collect();
    for variant in &mut cloned.variants {
        if let Some(repr) = &variant.variant_string_representation {
            variant.variant_string_representation =
                Some(rewrite_variant_repr(repr, &family.id, &id));
        }
        if let Some(slug) = &variant.legacy_slug {
            variant.legacy_slug = Some(rewrite_legacy_slug(slug, &family.id));
        }
        variant.display_name =
            retitle_display(&variant.display_name, &family.display_name, &display);
        if let Some(outside) = &variant.display_name_outside_picker {
            variant.display_name_outside_picker =
                Some(retitle_display(outside, &family.display_name, &display));
        }
    }
    cloned
}

fn retitle_display(text: &str, official: &str, injected: &str) -> String {
    if injected.is_empty() || text.contains(injected) {
        return text.to_owned();
    }
    let tag = injected.rsplit(" · ").next().unwrap_or("");
    if !tag.is_empty() && text.contains(&format!(" · {tag}")) {
        return text.to_owned();
    }
    let official_base = unsuffixed_name(official);
    if !official_base.is_empty() && text.contains(official_base) {
        return text.replacen(official_base, injected, 1);
    }
    text.to_owned()
}

fn rewrite_variant_repr(repr: &str, family_id: &str, injected: &str) -> String {
    let mut aliases = vec![family_id.to_owned()];
    if let Some((_, model)) = parse_provider_enable_id(family_id) {
        aliases.push(model.to_owned());
    }
    if let Some(bracket) = repr.find('[') {
        let name = &repr[..bracket];
        if aliases.iter().any(|id| name == id) || name == injected {
            return format!("{injected}{}", &repr[bracket..]);
        }
    }
    for alias in &aliases {
        if repr.starts_with(alias) {
            return format!("{injected}{}", &repr[alias.len()..]);
        }
    }
    repr.replace(family_id, injected)
}

fn rewrite_legacy_slug(slug: &str, family_id: &str) -> String {
    if slug.starts_with(INJECT_PREFIX) {
        slug.to_owned()
    } else if let Some(rest) = slug.strip_prefix("cursor-") {
        format!("{INJECT_PREFIX}{rest}")
    } else if let Some(rest) = slug.strip_prefix(family_id) {
        format!("{}{rest}", injected_id(family_id))
    } else {
        format!("{}-{slug}", injected_id(family_id))
    }
}

fn official_id_aliases(family: &SandFamily) -> Vec<String> {
    let mut ids = vec![family.id.clone()];
    if let Some((_, model)) = parse_provider_enable_id(&family.id) {
        ids.push(model.to_owned());
    }
    ids
}

fn find_official<'a>(
    models: &'a [AvailableModel],
    family: &SandFamily,
) -> Option<&'a AvailableModel> {
    let ids = official_id_aliases(family);
    models.iter().find(|model| {
        ids.iter().any(|id| {
            model.name == *id
                || model.server_model_name.as_deref() == Some(id.as_str())
                || model
                    .legacy_slugs
                    .iter()
                    .any(|slug| slug == id || slug.strip_prefix("cursor-") == Some(id.as_str()))
        })
    })
}

#[derive(Clone)]
struct AxisChoice {
    id: String,
    value: String,
    label: String,
}

fn axis_choices(axis: &ParamAxis) -> Vec<AxisChoice> {
    if axis.bool {
        return vec![
            AxisChoice {
                id: axis.id.clone(),
                value: "false".into(),
                label: String::new(),
            },
            AxisChoice {
                id: axis.id.clone(),
                value: "true".into(),
                label: axis.name.clone(),
            },
        ];
    }
    let Some(values) = &axis.r#enum else {
        return Vec::new();
    };
    values
        .iter()
        .enumerate()
        .map(|(index, value)| AxisChoice {
            id: axis.id.clone(),
            value: value.clone(),
            label: axis
                .labels
                .as_ref()
                .and_then(|labels| labels.get(index))
                .cloned()
                .filter(|label| !label.is_empty())
                .unwrap_or_else(|| crate::sand::axis_value_label(value)),
        })
        .collect()
}

fn cartesian_choices(axes: &[ParamAxis]) -> Vec<Vec<AxisChoice>> {
    let mut sets: Vec<Vec<AxisChoice>> = vec![Vec::new()];
    for axis in axes {
        let choices = axis_choices(axis);
        if choices.is_empty() {
            continue;
        }
        let mut next = Vec::new();
        for prefix in &sets {
            for choice in &choices {
                let mut row = prefix.clone();
                row.push(AxisChoice {
                    id: choice.id.clone(),
                    value: choice.value.clone(),
                    label: choice.label.clone(),
                });
                next.push(row);
            }
        }
        sets = next;
    }
    sets
}

fn combo_matches_default(combo: &[AxisChoice], family: &SandFamily) -> bool {
    if let Some(pairs) = family
        .default_variant
        .as_deref()
        .and_then(parse_variant_pairs)
    {
        if pairs.is_empty() {
            return combo.is_empty();
        }
        return pairs.iter().all(|(id, value)| {
            combo
                .iter()
                .any(|choice| choice.id == *id && choice.value == *value)
        });
    }
    let effort_ok = match family.effort_axis() {
        Some(axis) => combo.iter().any(|choice| {
            choice.id == axis.id
                && (choice.value == "high"
                    || family
                        .default_effort()
                        .as_deref()
                        .is_some_and(|want| want == choice.value))
        }),
        None => true,
    };
    let fast_ok = if family.has_fast_axis() {
        combo
            .iter()
            .any(|choice| choice.id == "fast" && choice.value == "true")
    } else {
        true
    };
    effort_ok && fast_ok
}

fn build_variants_from_family(id: &str, display: &str, family: &SandFamily) -> Vec<ModelVariant> {
    let combos = cartesian_choices(&family.axes);
    let max_opts: &[bool] = match (family.supports_max_mode, family.supports_non_max_mode) {
        (true, true) => &[false, true],
        (true, false) => &[true],
        _ => &[false],
    };
    let mut out = Vec::new();
    for combo in &combos {
        for is_max in max_opts {
            let suffix: Vec<String> = combo
                .iter()
                .filter(|choice| !choice.label.is_empty())
                .map(|choice| choice.label.clone())
                .collect();
            let display_name = if suffix.is_empty() {
                display.to_owned()
            } else {
                format!(
                    "{display} <span style=\"color: var(--cursor-text-tertiary);\">{}</span>",
                    suffix.join(" ")
                )
            };
            let parameter_values = combo
                .iter()
                .map(|choice| ModelParameterValue {
                    id: choice.id.clone(),
                    value: choice.value.clone(),
                })
                .collect();
            let inner = combo
                .iter()
                .map(|choice| format!("{}={}", choice.id, choice.value))
                .collect::<Vec<_>>()
                .join(",");
            let repr = if inner.is_empty() {
                format!("{id}[]")
            } else {
                format!("{id}[{inner}]")
            };
            let slug_tail = combo
                .iter()
                .filter(|choice| {
                    is_effort_axis_id(&choice.id) || (choice.id == "fast" && choice.value == "true")
                })
                .map(|choice| {
                    if choice.id == "fast" {
                        "fast".into()
                    } else {
                        choice.value.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join("-");
            let slug = if slug_tail.is_empty() {
                id.to_owned()
            } else {
                format!("{id}-{slug_tail}")
            };
            let is_default = combo_matches_default(combo, family);
            out.push(ModelVariant {
                parameter_values,
                display_name: display_name.clone(),
                is_max_mode: *is_max,
                is_default_max_config: (*is_max && is_default).then_some(true),
                is_default_non_max_config: (!*is_max && is_default).then_some(true),
                display_name_outside_picker: Some(display_name),
                variant_string_representation: Some(repr),
                legacy_slug: Some(slug),
            });
        }
    }
    out
}

pub const INJECT_PREFIX: &str = "gb-";
pub const PROVIDER_ENABLE_PREFIX: &str = "p/";

pub fn injected_id(family_id: &str) -> String {
    format!("{INJECT_PREFIX}{family_id}")
}

pub fn provider_enable_id(provider_id: &str, model: &str) -> String {
    format!("{PROVIDER_ENABLE_PREFIX}{provider_id}/{model}")
}

pub fn parse_provider_enable_id(id: &str) -> Option<(&str, &str)> {
    id.strip_prefix(PROVIDER_ENABLE_PREFIX)
        .and_then(|rest| rest.split_once('/'))
        .filter(|(provider, model)| !provider.is_empty() && !model.is_empty())
}

pub fn is_provider_enable_id(id: &str) -> bool {
    parse_provider_enable_id(id).is_some()
}

/// Injected `gb-p/...` / `p/...` enable ids must not fall through to Grok Bot sand.
pub fn is_provider_run_id(model_id: &str) -> bool {
    is_provider_enable_id(model_id.strip_prefix(INJECT_PREFIX).unwrap_or(model_id))
}

pub fn resolve_provider_run<'a>(
    providers: &'a [crate::providers::Provider],
    model_id: &str,
) -> Option<(&'a crate::providers::Provider, String)> {
    let raw = model_id.strip_prefix(INJECT_PREFIX).unwrap_or(model_id);
    let (provider_id, model) = parse_provider_enable_id(raw)?;
    let provider = providers.iter().find(|item| item.id == provider_id)?;
    Some((provider, model.to_owned()))
}

/// Official / imported window for this injected id. Provider row wins over
/// inbound Cursor `token_details`; grok-4.6 Stream fallback is 256000.
pub fn context_tokens_for_model(
    model_id: &str,
    providers: &[crate::providers::Provider],
    inbound_max: u32,
) -> u32 {
    if let Some((provider, model)) = resolve_provider_run(providers, model_id) {
        let (_, ctx) = crate::providers::row_knobs(provider, &model);
        if let Some(n) = ctx
            .as_deref()
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|n| *n > 0)
        {
            return n;
        }
    }
    if inbound_max > 0 {
        return inbound_max;
    }
    crate::agent_wire::DEFAULT_CONTEXT_TOKENS
}

pub fn started_provider_families(
    providers: &[crate::providers::Provider],
    enabled: &[String],
    catalog: &[SandFamily],
) -> Vec<SandFamily> {
    enabled
        .iter()
        .filter_map(|key| {
            let (provider_id, model) = parse_provider_enable_id(key)?;
            let provider = providers.iter().find(|item| item.id == provider_id)?;
            let official = catalog.iter().find(|family| family.id == model);
            let row = provider
                .model_map
                .iter()
                .find(|item| item.alias == model || item.upstream == model);
            let display = official
                .map(|family| family.display_name.as_str())
                .unwrap_or(model)
                .to_owned();
            let ctx = official
                .and_then(|family| family.context_token_limit)
                .or_else(|| {
                    row.and_then(|item| item.context.as_ref())
                        .and_then(|value| value.parse().ok())
                });
            let mut axes = official
                .map(|family| {
                    family
                        .axes
                        .iter()
                        .filter(|axis| axis.id != "context")
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if let Some(row) = row {
                if !row.efforts.is_empty() {
                    axes.retain(|axis| !crate::sand::is_effort_axis_id(&axis.id));
                    axes.insert(
                        0,
                        crate::sand::ParamAxis {
                            id: "effort".into(),
                            name: "Effort".into(),
                            r#enum: Some(row.efforts.clone()),
                            labels: Some(
                                row.efforts
                                    .iter()
                                    .map(|grade| crate::sand::axis_value_label(grade))
                                    .collect(),
                            ),
                            bool: false,
                        },
                    );
                }
                match row.fast {
                    Some(true) => {
                        if !axes.iter().any(|axis| axis.id == "fast") {
                            axes.push(crate::sand::ParamAxis {
                                id: "fast".into(),
                                name: "Fast".into(),
                                r#enum: None,
                                labels: None,
                                bool: true,
                            });
                        }
                    }
                    Some(false) => axes.retain(|axis| axis.id != "fast"),
                    None => {}
                }
            }
            Some(SandFamily {
                id: key.clone(),
                display_name: format!("{} · {}", display, provider.label),
                context_token_limit: ctx,
                context_token_limit_for_max_mode: ctx,
                thinking: official
                    .map(|family| family.thinking)
                    .unwrap_or(!axes.is_empty()),
                supports_images: official
                    .map(|family| family.supports_images)
                    .unwrap_or(true),
                supports_max_mode: false,
                supports_non_max_mode: true,
                axes,
                default_variant: official.and_then(|family| family.default_variant.clone()),
                available: true,
                ..SandFamily::default()
            })
        })
        .collect()
}

const PICKER_KEY: &str =
    "src.vs.platform.reactivestorage.browser.reactiveStorageServiceImpl.persistentStorage.applicationUser";

fn picker_db() -> Option<std::path::PathBuf> {
    let appdata = std::env::var_os("APPDATA").map(std::path::PathBuf::from)?;
    let path = appdata.join("Cursor/User/globalStorage/state.vscdb");
    path.is_file().then_some(path)
}

fn is_injected_picker_row(item: &Value) -> bool {
    let name = item.get("name").and_then(Value::as_str).unwrap_or("");
    let display = item
        .get("clientDisplayName")
        .and_then(Value::as_str)
        .unwrap_or("");
    let vendor = item.get("vendorName").and_then(Value::as_str).unwrap_or("");
    name.starts_with("gb-")
        || vendor.starts_with("gba-")
        || display.contains(" · Grok Bot")
        || display.contains(" · xAI")
        || display.contains(" · 通用")
        || display.contains('路')
        || name.contains('路')
}

fn load_picker_root(db: &rusqlite::Connection) -> crate::error::Result<Value> {
    let text: String = db
        .query_row(
            "SELECT value FROM ItemTable WHERE key = ?1",
            [PICKER_KEY],
            |row| row.get(0),
        )
        .map_err(|e| crate::error::Error::Msg(e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| crate::error::Error::Msg(e.to_string()))
}

fn save_picker_root(db: &rusqlite::Connection, root: &Value) -> crate::error::Result<()> {
    let text = serde_json::to_string(root)?;
    db.execute(
        "UPDATE ItemTable SET value = ?1 WHERE key = ?2",
        rusqlite::params![text, PICKER_KEY],
    )
    .map_err(|e| crate::error::Error::Msg(e.to_string()))?;
    Ok(())
}

fn strip_injected_picker(root: &mut Value) {
    if let Some(models) = root
        .get_mut("availableDefaultModels2")
        .and_then(Value::as_array_mut)
    {
        models.retain(|item| !is_injected_picker_row(item));
    }
    if let Some(enabled) = root
        .pointer_mut("/aiSettings/modelOverrideEnabled")
        .and_then(Value::as_array_mut)
    {
        enabled.retain(|item| {
            item.as_str()
                .map(|id| !id.starts_with("gb-") && !id.contains('路'))
                .unwrap_or(true)
        });
    }
}

pub fn publish_injected_to_cursor_picker(families: &[SandFamily]) -> crate::error::Result<String> {
    let Some(path) = picker_db() else {
        return Ok("no Cursor picker db".into());
    };
    let db = rusqlite::Connection::open(&path)
        .map_err(|e| crate::error::Error::Msg(e.to_string()))?;
    db.busy_timeout(std::time::Duration::from_secs(8))
        .map_err(|e| crate::error::Error::Msg(e.to_string()))?;
    let mut root = load_picker_root(&db)?;
    strip_injected_picker(&mut root);
    let officials = root
        .get("availableDefaultModels2")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut added = 0;
    if let Some(models) = root
        .get_mut("availableDefaultModels2")
        .and_then(Value::as_array_mut)
    {
        for family in families.iter().filter(|family| family.id != "default") {
            let item = json_injected_model(&officials, family);
            models.push(item);
            added += 1;
        }
    }
    if let Some(enabled) = root
        .pointer_mut("/aiSettings/modelOverrideEnabled")
        .and_then(Value::as_array_mut)
    {
        for family in families.iter().filter(|family| family.id != "default") {
            let id = injected_id(&family.id);
            if !enabled.iter().any(|item| item.as_str() == Some(id.as_str())) {
                enabled.push(Value::String(id));
            }
        }
    }
    save_picker_root(&db, &root)?;
    Ok(format!("picker +{added}"))
}

pub fn unpublish_injected_from_cursor_picker() -> crate::error::Result<String> {
    let Some(path) = picker_db() else {
        return Ok("no Cursor picker db".into());
    };
    let db = rusqlite::Connection::open(&path)
        .map_err(|e| crate::error::Error::Msg(e.to_string()))?;
    db.busy_timeout(std::time::Duration::from_secs(8))
        .map_err(|e| crate::error::Error::Msg(e.to_string()))?;
    let mut root = load_picker_root(&db)?;
    strip_injected_picker(&mut root);
    save_picker_root(&db, &root)?;
    Ok("picker gb-* removed".into())
}

/// Cursor only sees models the user started in Grok-Bot-Auth.
pub fn started_families(catalog: &[SandFamily], enabled: &[String]) -> Vec<SandFamily> {
    catalog
        .iter()
        .filter(|family| family.id != "default" && enabled.iter().any(|id| id == &family.id))
        .cloned()
        .collect()
}

/// Old builds wrote every catalog id into enabled. Treat that as "none started".
pub fn normalize_enabled(catalog: &[SandFamily], enabled: Vec<String>) -> Vec<String> {
    let ids: Vec<String> = catalog
        .iter()
        .filter(|family| family.id != "default")
        .map(|family| family.id.clone())
        .collect();
    let provider_keys: Vec<String> = enabled
        .iter()
        .filter(|id| is_provider_enable_id(id))
        .cloned()
        .collect();
    if ids.is_empty() {
        return enabled
            .into_iter()
            .filter(|id| id != "default" && !id.is_empty())
            .collect();
    }
    if ids.iter().all(|id| enabled.iter().any(|item| item == id)) {
        return provider_keys;
    }
    enabled
        .into_iter()
        .filter(|id| ids.iter().any(|item| item == id) || is_provider_enable_id(id))
        .collect()
}

#[derive(Clone, PartialEq, Message)]
struct DefaultModelResponse {
    #[prost(string, tag = "1")]
    model: String,
    #[prost(string, tag = "2")]
    thinking_model: String,
    #[prost(bool, tag = "3")]
    max_mode: bool,
    #[prost(string, tag = "4")]
    next_default_set_date: String,
}

#[derive(Clone, PartialEq, Message)]
struct DefaultModelNudgeDataResponse {
    #[prost(string, tag = "1")]
    nudge_date: String,
    #[prost(bool, tag = "2")]
    should_default_switch_on_new_chat: bool,
    #[prost(string, repeated, tag = "3")]
    models_with_no_default_switch: Vec<String>,
    #[prost(string, tag = "4")]
    conversion_model_override: String,
}

pub fn encode_default_model(model_id: &str) -> Vec<u8> {
    DefaultModelResponse {
        model: model_id.into(),
        thinking_model: model_id.into(),
        max_mode: false,
        next_default_set_date: String::new(),
    }
    .encode_to_vec()
}

pub fn encode_default_nudge(enabled: &[String]) -> Vec<u8> {
    DefaultModelNudgeDataResponse {
        nudge_date: "0".into(),
        should_default_switch_on_new_chat: false,
        models_with_no_default_switch: enabled.iter().map(|id| injected_id(id)).collect(),
        conversion_model_override: String::new(),
    }
    .encode_to_vec()
}

fn json_clone_injected(official: &Value, family: &SandFamily) -> Value {
    let mut value = official.clone();
    let id = injected_id(&family.id);
    let display = injected_display(family);
    let official_title = official
        .get("clientDisplayName")
        .and_then(Value::as_str)
        .unwrap_or("");
    if let Some(obj) = value.as_object_mut() {
        obj.insert("name".into(), json!(id));
        obj.insert("serverModelName".into(), json!(id));
        obj.insert("clientDisplayName".into(), json!(display));
        obj.insert("inputboxShortModelName".into(), json!(display));
        let tag = source_tag(family);
        obj.insert("vendorName".into(), json!(tag));
        obj.insert("vendor".into(), json!(format!("gba-{tag}")));
        obj.insert("namedModelSectionIndex".into(), json!(2));
        obj.insert("defaultOn".into(), json!(true));
        obj.insert("modelPickerBadges".into(), json!([]));
        if let Some(limit) = family.context_token_limit {
            obj.insert("contextTokenLimit".into(), json!(limit));
        }
        if let Some(limit) = family.context_token_limit_for_max_mode {
            obj.insert("contextTokenLimitForMaxMode".into(), json!(limit));
        }
        if let Some(slugs) = obj.get("legacySlugs").and_then(|v| v.as_array()).cloned() {
            let rewritten: Vec<Value> = slugs
                .iter()
                .filter_map(|item| item.as_str())
                .map(|slug| Value::String(rewrite_legacy_slug(slug, &family.id)))
                .collect();
            obj.insert("legacySlugs".into(), Value::Array(rewritten));
        }
        obj.insert(
            "tooltipData".into(),
            json!({"markdownContent": inject_tooltip(family)}),
        );
        if let Some(variants) = obj.get_mut("variants").and_then(|v| v.as_array_mut()) {
            for variant in variants {
                if let Some(repr) = variant
                    .get("variantStringRepresentation")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
                {
                    variant["variantStringRepresentation"] =
                        json!(rewrite_variant_repr(&repr, &family.id, &id));
                }
                if let Some(slug) = variant
                    .get("legacySlug")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
                {
                    variant["legacySlug"] = json!(rewrite_legacy_slug(&slug, &family.id));
                }
                if let Some(name) = variant
                    .get("displayName")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
                {
                    variant["displayName"] =
                        json!(retitle_display(&name, official_title, &display));
                }
                if let Some(outside) = variant
                    .get("displayNameOutsidePicker")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
                {
                    variant["displayNameOutsidePicker"] =
                        json!(retitle_display(&outside, official_title, &display));
                }
            }
        }
    }
    value
}

fn json_model(family: &SandFamily) -> Value {
    let id = injected_id(&family.id);
    let display = injected_display(family);
    let variants: Vec<Value> = build_variants_from_family(&id, &display, family)
        .into_iter()
        .map(|variant| {
            json!({
                "displayName": variant.display_name,
                "isMaxMode": variant.is_max_mode,
                "isDefaultNonMaxConfig": variant.is_default_non_max_config.unwrap_or(false),
                "isDefaultMaxConfig": variant.is_default_max_config.unwrap_or(false),
                "parameterValues": variant
                    .parameter_values
                    .iter()
                    .map(|item| json!({"id": item.id, "value": item.value}))
                    .collect::<Vec<_>>(),
                "variantStringRepresentation": variant.variant_string_representation,
                "legacySlug": variant.legacy_slug
            })
        })
        .collect();
    let parameter_definitions: Vec<Value> = family
        .axes
        .iter()
        .map(|axis| {
            if axis.bool {
                json!({"id": axis.id, "name": axis.name, "parameterType": {"booleanParameter": {"values": [{"value": "false"}, {"value": "true"}]}}})
            } else {
                json!({
                    "id": axis.id,
                    "name": axis.name,
                    "parameterType": {
                        "enumParameter": {
                            "values": axis.r#enum.clone().unwrap_or_default().into_iter().map(|value| json!({"value": value})).collect::<Vec<_>>()
                        }
                    }
                })
            }
        })
        .collect();
    json!({
        "name": id,
        "serverModelName": id,
        "clientDisplayName": display,
        "inputboxShortModelName": display,
        "vendorName": source_tag(family),
        "vendor": format!("gba-{}", source_tag(family)),
        "namedModelSectionIndex": 2,
        "defaultOn": true,
        "supportsAgent": true,
        "supportsThinking": family.thinking || family.effort_axis().is_some(),
        "supportsImages": family.supports_images,
        "supportsMaxMode": family.supports_max_mode,
        "supportsNonMaxMode": family.supports_non_max_mode,
        "contextTokenLimit": family.context_token_limit,
        "contextTokenLimitForMaxMode": family.context_token_limit_for_max_mode,
        "modelPickerBadges": [],
        "parameterDefinitions": parameter_definitions,
        "variants": variants
    })
}

fn json_injected_model(officials: &[Value], family: &SandFamily) -> Value {
    let ids = official_id_aliases(family);
    officials
        .iter()
        .find(|model| {
            let name = model.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let server = model
                .get("serverModelName")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            ids.iter().any(|id| name == id || server == id)
        })
        .map(|model| json_clone_injected(model, family))
        .unwrap_or_else(|| json_model(family))
}

pub fn merge_usable_models(body: &[u8], content_type: &str, families: &[SandFamily]) -> Vec<u8> {
    if families.is_empty() {
        return body.to_vec();
    }
    let extra = UsableModelsAddition {
        models: families
            .iter()
            .map(|family| UsableModel {
                model_id: injected_id(&family.id),
                thinking_details: (family.thinking || family.effort_axis().is_some())
                    .then_some(ThinkingDetails {}),
                display_model_id: injected_id(&family.id),
                display_name: injected_display(family),
                display_name_short: injected_display(family),
                max_mode: Some(false),
                api_key_credentials: Some(ApiKeyCredentials {
                    api_key: "grok-bot-local".into(),
                }),
            })
            .collect(),
    }
    .encode_to_vec();
    if let Some((_flags, payload, rest)) = split_first_frame(body) {
        if content_type.contains("json") || payload.first() == Some(&b'{') {
            if let Ok(mut value) = serde_json::from_slice::<Value>(payload) {
                if let Some(models) = value.get_mut("models").and_then(Value::as_array_mut) {
                    for family in families {
                        models.push(json!({
                            "modelId": injected_id(&family.id),
                            "displayName": injected_display(family),
                            "displayNameShort": injected_display(family),
                            "contextTokenLimit": family.context_token_limit,
                            "contextTokenLimitForMaxMode": family.context_token_limit_for_max_mode,
                            "supportsThinking": family.thinking || family.effort_axis().is_some(),
                        }));
                    }
                }
                if let Ok(merged) = serde_json::to_vec(&value) {
                    return frame_then_rest(&merged, rest);
                }
            }
            return body.to_vec();
        }
        let mut payload = payload.to_vec();
        payload.extend_from_slice(&extra);
        return frame_then_rest(&payload, rest);
    }
    let mut out = body.to_vec();
    out.extend_from_slice(&extra);
    out
}

pub fn merge_available_models(body: &[u8], content_type: &str, families: &[SandFamily]) -> Vec<u8> {
    if families.is_empty() {
        return body.to_vec();
    }
    try_merge(body, content_type, families).unwrap_or_else(|| body.to_vec())
}

fn injected_models(official: &[AvailableModel], families: &[SandFamily]) -> Vec<AvailableModel> {
    families
        .iter()
        .map(|family| {
            find_official(official, family)
                .map(|model| clone_as_injected(model, family))
                .unwrap_or_else(|| proto_model_from_family(family))
        })
        .collect()
}

fn try_merge(body: &[u8], content_type: &str, families: &[SandFamily]) -> Option<Vec<u8>> {
    if let Some((_flags, payload, rest)) = split_first_frame(body) {
        let merged = if content_type.contains("json") || payload.first() == Some(&b'{') {
            merge_json(payload, families)?
        } else {
            merge_proto(payload, families)
        };
        return Some(frame_then_rest(&merged, rest));
    }
    if content_type.contains("json") || body.first() == Some(&b'{') {
        return merge_json(body, families);
    }
    Some(merge_proto(body, families))
}

fn merge_proto(payload: &[u8], families: &[SandFamily]) -> Vec<u8> {
    let official = AvailableModelsAddition::decode(payload)
        .map(|parsed| parsed.models)
        .unwrap_or_default();
    let extra = AvailableModelsAddition {
        model_names: families.iter().map(|f| injected_id(&f.id)).collect(),
        models: injected_models(&official, families),
    }
    .encode_to_vec();
    let mut out = payload.to_vec();
    out.extend_from_slice(&extra);
    out
}

fn merge_json(payload: &[u8], families: &[SandFamily]) -> Option<Vec<u8>> {
    let mut value = serde_json::from_slice::<Value>(payload).ok()?;
    let officials = value
        .get("models")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(models) = value.get_mut("models").and_then(Value::as_array_mut) {
        for family in families {
            models.push(json_injected_model(&officials, family));
        }
    }
    if let Some(names) = value.get_mut("modelNames").and_then(Value::as_array_mut) {
        for family in families {
            names.push(Value::String(injected_id(&family.id)));
        }
    }
    serde_json::to_vec(&value).ok()
}

pub fn request_uses_injected_model(body: &[u8], enabled: &[String]) -> bool {
    crate::agent_wire::body_mentions_injected(body, enabled)
}

pub fn rewrite_injected_model_ids(body: &[u8], enabled: &[String]) -> Vec<u8> {
    if let Some((_flags, payload, rest)) = split_first_frame(body) {
        return frame_then_rest(&rewrite_plain(payload, enabled), rest);
    }
    rewrite_plain(body, enabled)
}

fn rewrite_plain(payload: &[u8], enabled: &[String]) -> Vec<u8> {
    let mut current = payload.to_vec();
    for id in enabled {
        if id.is_empty() || id == "default" {
            continue;
        }
        let from = injected_id(id);
        if let Some(next) = replace_len_prefixed(&current, from.as_bytes(), id.as_bytes()) {
            current = next;
        }
        current = replace_raw(&current, from.as_bytes(), id.as_bytes());
    }
    current
}

fn replace_raw(buf: &[u8], old: &[u8], new: &[u8]) -> Vec<u8> {
    if old.is_empty() || old == new {
        return buf.to_vec();
    }
    let mut out = Vec::with_capacity(buf.len());
    let mut index = 0;
    while index < buf.len() {
        if index + old.len() <= buf.len() && &buf[index..index + old.len()] == old {
            out.extend_from_slice(new);
            index += old.len();
        } else {
            out.push(buf[index]);
            index += 1;
        }
    }
    out
}

fn encode_varint(mut value: usize) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
    out
}

#[allow(dead_code)]
fn contains_len_prefixed(buf: &[u8], text: &[u8]) -> bool {
    let mut needle = encode_varint(text.len());
    needle.extend_from_slice(text);
    buf.windows(needle.len())
        .any(|window| window == needle.as_slice())
}

fn replace_len_prefixed(buf: &[u8], old: &[u8], new: &[u8]) -> Option<Vec<u8>> {
    let mut needle = encode_varint(old.len());
    needle.extend_from_slice(old);
    let pos = buf
        .windows(needle.len())
        .position(|window| window == needle.as_slice())?;
    let mut replacement = encode_varint(new.len());
    replacement.extend_from_slice(new);
    let mut out = Vec::with_capacity(buf.len() + replacement.len() - needle.len());
    out.extend_from_slice(&buf[..pos]);
    out.extend_from_slice(&replacement);
    out.extend_from_slice(&buf[pos + needle.len()..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grok_family() -> SandFamily {
        SandFamily {
            id: "grok-4.6".into(),
            display_name: "Cursor Grok 4.6".into(),
            context_token_limit: Some(256000),
            context_token_limit_for_max_mode: Some(256000),
            thinking: true,
            supports_max_mode: true,
            supports_non_max_mode: true,
            default_variant: Some("grok-4.6[effort=high,fast=true]".into()),
            axes: vec![
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
            ],
            available: true,
            ..SandFamily::default()
        }
    }

    #[test]
    fn json_merge_keeps_official_claude() {
        let body = br#"{"models":[{"name":"claude-opus-4-6","clientDisplayName":"Claude 4.6 Opus"},{"name":"gpt-5.6-sol"}]}"#;
        let merged = merge_available_models(body, "application/json", &[grok_family()]);
        let text = String::from_utf8(merged).unwrap();
        assert!(text.contains("claude-opus-4-6"), "{text}");
        assert!(text.contains("gpt-5.6-sol"), "{text}");
        assert!(text.contains("gb-grok-4.6"), "{text}");
    }

    #[test]
    fn json_merge_appends_family() {
        let body = br#"{"models":[{"name":"gpt-5"}]}"#;
        let families = vec![grok_family()];
        let merged = merge_available_models(body, "application/json", &families);
        let text = String::from_utf8(merged).unwrap();
        assert!(text.contains("gb-grok-4.6"));
        assert!(text.contains("Grok Bot"));
        assert!(text.contains("gpt-5"));
        assert!(text.contains("variants"));
        assert!(text.contains("effort=low"));
        assert!(text.contains("effort=high"));
        assert!(text.contains("effort=xhigh"));
        assert!(text.contains("fast=true"));
        assert!(text.contains("fast=false"));
        assert!(!text.contains("reasoning="));
        assert!(!text.contains("context=200k"));
        assert!(!text.contains("effort=max"));
        assert!(text.contains("Extra High"));
        assert!(!text.contains("\"name\":\"grok-4.6\""));
        assert!(text.contains("gb-grok-4.6[effort=high,fast=true]"));
        assert!(!text.contains("可用"));
    }

    #[test]
    fn usable_json_short_name_keeps_source_suffix() {
        let body = crate::connect::encode_connect_frame(br#"{"models":[]}"#);
        let merged = merge_usable_models(&body, "application/json", &[grok_family()]);
        let text = String::from_utf8_lossy(&merged).into_owned();
        assert!(text.contains("gb-grok-4.6"));
        assert!(text.contains("· Grok Bot"));
        assert!(text.contains("\"displayNameShort\":\"Cursor Grok 4.6 · Grok Bot\""));
        assert!(text.contains("\"contextTokenLimit\":256000"));
        assert!(text.contains("\"contextTokenLimitForMaxMode\":256000"));
    }

    #[test]
    fn json_merge_clones_any_official_family_axes() {
        let body = br#"{
            "models":[{
                "name":"claude-opus-4-6",
                "clientDisplayName":"Claude 4.6 Opus",
                "supportsThinking":true,
                "contextTokenLimit":200000,
                "contextTokenLimitForMaxMode":1000000,
                "parameterDefinitions":[{"id":"thinking","name":"Thinking"}],
                "variants":[{
                    "variantStringRepresentation":"claude-opus-4-6[thinking=high]",
                    "displayName":"Claude 4.6 Opus High"
                }]
            }]
        }"#;
        let family = SandFamily {
            id: "claude-opus-4-6".into(),
            display_name: "Claude 4.6 Opus".into(),
            context_token_limit: Some(200000),
            context_token_limit_for_max_mode: Some(1000000),
            thinking: true,
            ..SandFamily::default()
        };
        let merged = merge_available_models(body, "application/json", &[family]);
        let value: serde_json::Value = serde_json::from_slice(&merged).unwrap();
        let gb = value["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|model| model["name"] == "gb-claude-opus-4-6")
            .expect("injected claude");
        assert_eq!(gb["contextTokenLimit"], 200000);
        assert_eq!(gb["contextTokenLimitForMaxMode"], 1000000);
        assert!(gb["parameterDefinitions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "thinking"));
        assert!(gb["variants"][0]["variantStringRepresentation"]
            .as_str()
            .unwrap()
            .starts_with("gb-claude-opus-4-6[thinking=high]"));
        assert!(!serde_json::to_string(&gb).unwrap().contains("effort=xhigh"));
        let official = value["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|model| model["name"] == "claude-opus-4-6")
            .expect("official kept");
        assert_eq!(official["name"], "claude-opus-4-6");
    }

    #[test]
    fn provider_enable_clones_matching_official_family() {
        let body = br#"{"models":[{"name":"grok-4.6","parameterDefinitions":[{"id":"effort"}],"variants":[{"variantStringRepresentation":"grok-4.6[effort=high,fast=true]"}]}]}"#;
        let family = SandFamily {
            id: "p/xai-X/grok-4.6".into(),
            display_name: "Cursor Grok 4.6 · xAI".into(),
            context_token_limit: Some(256000),
            context_token_limit_for_max_mode: Some(256000),
            ..SandFamily::default()
        };
        let merged = merge_available_models(body, "application/json", &[family]);
        let text = String::from_utf8(merged).unwrap();
        assert!(text.contains("gb-p/xai-X/grok-4.6"));
        assert!(text.contains("gb-p/xai-X/grok-4.6[effort=high,fast=true]"));
        assert!(text.contains("\"name\":\"grok-4.6\""));
    }

    #[test]
    fn json_merge_clones_official_variant_ids() {
        let body = br#"{
            "models":[{
                "name":"grok-4.6",
                "clientDisplayName":"Cursor Grok 4.6",
                "contextTokenLimit":200000,
                "contextTokenLimitForMaxMode":500000,
                "parameterDefinitions":[{"id":"effort","name":"Effort"}],
                "variants":[{
                    "variantStringRepresentation":"grok-4.6[effort=xhigh,fast=true]",
                    "legacySlug":"cursor-grok-4.6-xhigh-fast"
                }]
            }]
        }"#;
        let merged = merge_available_models(body, "application/json", &[grok_family()]);
        let text = String::from_utf8(merged.clone()).unwrap();
        assert!(text.contains("\"name\":\"grok-4.6\""));
        assert!(text.contains("gb-grok-4.6[effort=xhigh,fast=true]"));
        assert!(text.contains("gb-grok-4.6-xhigh-fast"));
        assert!(!text.contains("reasoning="));
        assert!(!text.contains("可用"));
        let value: serde_json::Value = serde_json::from_slice(&merged).unwrap();
        let gb = value["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|model| model["name"] == "gb-grok-4.6")
            .expect("injected grok");
        assert_eq!(gb["contextTokenLimit"], 256000);
        assert_eq!(gb["contextTokenLimitForMaxMode"], 256000);
        assert!(gb["modelPickerBadges"]
            .as_array()
            .map(|badges| badges.is_empty())
            .unwrap_or(true));
    }

    #[test]
    fn started_families_only_keeps_enabled_ids() {
        let catalog = vec![
            SandFamily {
                id: "grok-4.6".into(),
                display_name: "Grok 4.6".into(),
                ..SandFamily::default()
            },
            SandFamily {
                id: "claude-opus-4-6".into(),
                display_name: "Opus".into(),
                ..SandFamily::default()
            },
        ];
        let started = started_families(&catalog, &["grok-4.6".into()]);
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].id, "grok-4.6");
        assert!(started_families(&catalog, &[]).is_empty());
        assert!(
            normalize_enabled(&catalog, vec!["grok-4.6".into(), "claude-opus-4-6".into()])
                .is_empty()
        );
        assert_eq!(
            normalize_enabled(&catalog, vec!["grok-4.6".into()]),
            vec!["grok-4.6".to_string()]
        );
        let mixed = normalize_enabled(
            &catalog,
            vec![
                "grok-4.6".into(),
                "p/xai-X/grok-4.6".into(),
                "claude-opus-4-6".into(),
            ],
        );
        assert_eq!(mixed, vec!["p/xai-X/grok-4.6".to_string()]);
        assert_eq!(
            parse_provider_enable_id("p/xai-X/grok-4.6"),
            Some(("xai-X", "grok-4.6"))
        );
        assert_ne!(injected_id("grok-4.6"), injected_id("p/xai-X/grok-4.6"));
        let provider = crate::providers::Provider {
            id: "xai-X".into(),
            label: "xAI".into(),
            kind: crate::providers::ProviderKind::Xai,
            ..crate::providers::Provider::default()
        };
        let families =
            started_provider_families(&[provider], &["p/xai-X/grok-4.6".into()], &[grok_family()]);
        assert_eq!(families.len(), 1);
        let injected = &families[0];
        assert_eq!(injected.display_name, "Cursor Grok 4.6 · xAI");
        let shown = injected_display(injected);
        assert_eq!(shown, "Cursor Grok 4.6 · xAI");
        assert_eq!(
            retitle_display("Cursor Grok 4.6 High Fast", "Cursor Grok 4.6", &shown),
            "Cursor Grok 4.6 · xAI High Fast"
        );
        assert_eq!(retitle_display(&shown, "Cursor Grok 4.6", &shown), shown);
        assert!(injected.has_fast_axis());
        assert_eq!(
            injected.effort_values(),
            vec![
                "low".to_string(),
                "medium".into(),
                "high".into(),
                "xhigh".into()
            ]
        );
        assert!(!injected.effort_values().iter().any(|item| item == "max"));
        assert_eq!(injected.context_token_limit, Some(256000));
        assert_eq!(injected.context_token_limit_for_max_mode, Some(256000));
        assert!(!injected.axes.iter().any(|axis| axis.id == "context"));
        let merged = merge_available_models(
            br#"{"models":[{"name":"gpt-5"}]}"#,
            "application/json",
            families.as_slice(),
        );
        let text = String::from_utf8(merged).unwrap();
        assert!(text.contains("effort=xhigh"));
        assert!(text.contains("fast=true"));
        assert!(text.contains("Extra High"));
        assert!(text.contains("xAI"));
        assert!(!text.contains("via Grok Bot sand session"));
        assert!(!text.contains("可用"));
    }

    #[test]
    fn context_tokens_prefers_provider_row_over_inbound() {
        let provider = crate::providers::Provider {
            id: "xai-X".into(),
            model_map: vec![crate::providers::ModelMap {
                alias: "grok-4.6".into(),
                upstream: "grok-4.6".into(),
                context: Some("131072".into()),
                ..crate::providers::ModelMap::default()
            }],
            ..crate::providers::Provider::default()
        };
        assert_eq!(
            context_tokens_for_model("gb-p/xai-X/grok-4.6", &[provider], 256000),
            131072,
            "imported/official row window wins over inbound 256k"
        );
        assert_eq!(
            context_tokens_for_model("gb-grok-4.6", &[], 0),
            256000,
            "Grok Bot grok-4.6 Stream fallback"
        );
        assert_eq!(
            context_tokens_for_model("gb-grok-4.6", &[], 128000),
            128000,
            "Cursor token_details used when no provider row"
        );
    }

    #[test]
    fn injected_id_keeps_gb_prefix_for_tagged_available_family() {
        assert_eq!(injected_id("gemini-3.1-pro"), "gb-gemini-3.1-pro");
        assert_eq!(injected_id("claude-haiku-4-5"), "gb-claude-haiku-4-5");
        assert_eq!(injected_id("composer-2.5"), "gb-composer-2.5");
        assert!(!injected_id("grok-4.6").starts_with("gb-gb-"));
        let families = vec![
            SandFamily {
                id: "gemini-3.1-pro".into(),
                display_name: "Gemini 3.1 Pro".into(),
                available: true,
                ..SandFamily::default()
            },
            SandFamily {
                id: "claude-opus-4-6".into(),
                display_name: "Claude Opus 4.6".into(),
                ..SandFamily::default()
            },
        ];
        let merged = merge_available_models(
            br#"{"models":[{"name":"gpt-5.4"}]}"#,
            "application/json",
            &families,
        );
        let text = String::from_utf8(merged).unwrap();
        assert!(text.contains("gb-gemini-3.1-pro"));
        assert!(text.contains("gb-claude-opus-4-6"));
        assert!(text.contains("gpt-5.4"));
        assert!(!text.contains("\"name\":\"gemini-3.1-pro\""));
    }

    #[test]
    fn detects_injected_len_prefixed_id() {
        let enabled = vec!["grok-4.6".into()];
        let mut body = encode_varint("gb-grok-4.6".len());
        body.extend_from_slice(b"gb-grok-4.6");
        assert!(request_uses_injected_model(&body, &enabled));
        assert!(!request_uses_injected_model(
            b"please use grok-4.6 in chat",
            &enabled
        ));
        assert!(request_uses_injected_model(
            br#"{"modelId":"gb-grok-4.6"}"#,
            &enabled
        ));
        let hex = crate::agent_wire::ascii_hex(b"gb-grok-4.6");
        assert!(request_uses_injected_model(hex.as_bytes(), &enabled));
    }

    #[test]
    fn rewrite_maps_injected_id_to_sand_id() {
        let mut body = encode_varint("gb-claude-opus-4-6".len());
        body.extend_from_slice(b"gb-claude-opus-4-6");
        let out = rewrite_injected_model_ids(&body, &["claude-opus-4-6".into()]);
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("claude-opus-4-6"));
        assert!(!text.contains("gb-claude-opus-4-6"));
    }

    #[test]
    fn rewrite_json_model_id() {
        let out = rewrite_injected_model_ids(
            br#"{"modelId":"gb-claude-opus-4-6"}"#,
            &["claude-opus-4-6".into()],
        );
        let text = String::from_utf8_lossy(&out);
        assert_eq!(text, r#"{"modelId":"claude-opus-4-6"}"#);
    }

    #[test]
    fn deleted_provider_enable_is_not_sand_fallback() {
        assert!(is_provider_run_id("gb-p/gone/model"));
        assert!(is_provider_run_id("p/gone/model"));
        assert!(!is_provider_run_id("gb-grok-4.6"));
        assert!(resolve_provider_run(&[], "gb-p/gone/model").is_none());
    }
}
