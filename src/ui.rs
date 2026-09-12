use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use eframe::egui::{self, Align, Color32, Layout, RichText, Vec2};
use tokio::runtime::Handle;

use crate::api::{park_cursor_mitm, start_coexist, stop_coexist, AppState};
use crate::motion::{self, Busy};
use crate::sand::{begin_login, import_account, parse_sand_families, public_account, ImportBody};
use crate::store;
use crate::theme::{self, Glyph};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pane {
    Overview,
    Models,
    Providers,
    Accounts,
    Ext,
    Settings,
    Chat,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExtTab {
    Pool,
    Skills,
    Mcp,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Network,
    Data,
    Version,
}

const KIND_OPENAI: usize = 0;
const KIND_XAI: usize = 1;
const KIND_ANTHROPIC: usize = 2;
const KIND_GEMINI: usize = 3;
const KIND_ZHIPU: usize = 4;
const KIND_KIMI: usize = 5;
const KIND_DEEPSEEK: usize = 6;
const KIND_GENERIC: usize = 7;
const KIND_GROK_BOT: usize = 8;

pub struct GrokBotApp {
    rt: Handle,
    state: AppState,
    import_json: String,
    notice: Arc<StdMutex<String>>,
    checked: HashSet<String>,
    test_prompt: String,
    test_think: Arc<StdMutex<String>>,
    test_out: Arc<StdMutex<String>>,
    test_model: String,
    pane: Pane,
    effort: String,
    context: String,
    pool_draft: String,
    provider_label: String,
    provider_kind: usize,
    provider_key: String,
    provider_base: String,
    provider_auth: usize,
    provider_wire: usize,
    provider_rows: Vec<crate::providers::ModelMap>,
    provider_effort: String,
    provider_context: String,
    provider_catalog: Vec<String>,
    provider_picked: HashSet<String>,
    oauth_fill: Arc<StdMutex<Option<String>>>,
    catalog_fill: Arc<StdMutex<Option<Vec<String>>>>,
    ping_fill: Arc<StdMutex<Option<String>>>,
    ping_label: String,
    editing_id: String,
    #[allow(dead_code)]
    compact_budget: usize,
    source: String,
    provider_model: String,
    history: Arc<StdMutex<Vec<crate::compact::ChatTurn>>>,
    model_filter: String,
    show_json: bool,
    busy: Arc<StdMutex<Busy>>,
    snap: Option<ViewSnap>,
    last_quota_ms: u64,
    enabled_pending: HashMap<String, bool>,
    ext_tab: ExtTab,
    settings_tab: SettingsTab,
    mcp_servers: Vec<crate::mcp::McpServer>,
    skills: Vec<crate::skills::SkillEntry>,
    version_notice: String,
    overview_span_ms: u64,
    scroll_to_form: bool,
    mcp_custom_id: String,
    mcp_custom_cmd: String,
    mcp_apply_confirm: bool,
    #[allow(dead_code)]
    grok_rows: Vec<crate::providers::ModelMap>,
    grok_filter: String,
    test_fast: bool,
    oauth_email: String,
    outbound_proxy: String,
    allow_exit: Arc<AtomicBool>,
    show_window: Arc<AtomicBool>,
    tray_hidden: bool,
    tray: Option<crate::tray::TrayHold>,
}

#[derive(Clone)]
struct ViewSnap {
    account: crate::sand::PublicAccount,
    catalog: Vec<crate::sand::SandFamily>,
    imported: Vec<String>,
    enabled: Vec<String>,
    coexist: bool,
    providers: Vec<crate::providers::Provider>,
    pool: Vec<String>,
    grok_slots: Vec<crate::sand::Account>,
    grok_mode: crate::accounts::PoolMode,
    grok_active: String,
    pending_oauth: Option<crate::accounts::PendingOauth>,
}

impl GrokBotApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rt: Handle, state: AppState) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx);
        let prefs = rt.block_on(store::load_prefs()).unwrap_or_default();
        let (effort, context, compact_budget) = rt.block_on(async {
            (
                state.effort.lock().await.clone(),
                state.context.lock().await.clone(),
                *state.compact_budget.lock().await,
            )
        });
        let mcp_servers = if prefs.mcp.is_empty() {
            crate::mcp::presets()
        } else {
            prefs.mcp
        };
        let mut app = Self {
            rt,
            state,
            import_json: String::new(),
            notice: Arc::new(StdMutex::new(String::new())),
            checked: HashSet::new(),
            test_prompt: "hi".into(),
            test_think: Arc::new(StdMutex::new(String::new())),
            test_out: Arc::new(StdMutex::new(String::new())),
            test_model: String::new(),
            pane: Pane::Overview,
            effort,
            context,
            compact_budget,
            pool_draft: String::new(),
            provider_label: "OpenAI".into(),
            provider_kind: KIND_GROK_BOT,
            provider_key: String::new(),
            provider_base: crate::providers::ProviderKind::Openai
                .default_base_url()
                .into(),
            provider_auth: 0,
            provider_wire: 0,
            provider_rows: Vec::new(),
            provider_effort: String::new(),
            provider_context: String::new(),
            provider_catalog: Vec::new(),
            provider_picked: HashSet::new(),
            oauth_fill: Arc::new(StdMutex::new(None)),
            catalog_fill: Arc::new(StdMutex::new(None)),
            ping_fill: Arc::new(StdMutex::new(None)),
            ping_label: String::new(),
            editing_id: String::new(),
            source: "grok".into(),
            provider_model: crate::providers::default_model_for(
                &crate::providers::ProviderKind::Openai,
            )
            .into(),
            history: Arc::new(StdMutex::new(Vec::new())),
            model_filter: String::new(),
            show_json: false,
            busy: Arc::new(StdMutex::new(Busy::default())),
            snap: None,
            last_quota_ms: 0,
            enabled_pending: HashMap::new(),
            ext_tab: ExtTab::Pool,
            settings_tab: SettingsTab::Network,
            mcp_servers,
            skills: crate::skills::scan(),
            version_notice: format!("当前 {}", crate::update::app_version()),
            overview_span_ms: 7 * 86_400_000,
            scroll_to_form: false,
            mcp_custom_id: String::new(),
            mcp_custom_cmd: "npx".into(),
            mcp_apply_confirm: false,
            grok_rows: Vec::new(),
            grok_filter: String::new(),
            test_fast: true,
            oauth_email: String::new(),
            outbound_proxy: prefs.outbound_proxy,
            allow_exit: Arc::new(AtomicBool::new(false)),
            show_window: Arc::new(AtomicBool::new(false)),
            tray_hidden: false,
            tray: None,
        };
        app.tray = crate::tray::install(
            app.allow_exit.clone(),
            app.show_window.clone(),
            cc.egui_ctx.clone(),
            app.rt.clone(),
            app.state.clone(),
        );
        if app.tray.is_none() {
            app.set_notice("托盘图标没建起来，关窗口仍会退出。托盘「退出」才会停反代。");
        }
        app
    }

    fn pull_snap(&mut self) {
        let Ok(account) = self.state.account.try_lock() else {
            return;
        };
        let Ok(catalog) = self.state.models.try_lock() else {
            return;
        };
        let Ok(imported) = self.state.imported.try_lock() else {
            return;
        };
        let Ok(enabled) = self.state.enabled.try_lock() else {
            return;
        };
        let Ok(coexist_flag) = self.state.coexist.try_lock() else {
            return;
        };
        let Ok(providers) = self.state.providers.try_lock() else {
            return;
        };
        let Ok(pool) = self.state.prompt_pool.try_lock() else {
            return;
        };
        let Ok(grok_pool) = self.state.grok_pool.try_lock() else {
            return;
        };
        let Ok(pending) = self.state.pending_oauth.try_lock() else {
            return;
        };
        let mut enabled = enabled.clone();
        self.enabled_pending
            .retain(|id, on| enabled.iter().any(|item| item == id) != *on);
        for (id, on) in &self.enabled_pending {
            if *on {
                if !enabled.iter().any(|item| item == id) {
                    enabled.push(id.clone());
                }
            } else {
                enabled.retain(|item| item != id);
            }
        }
        self.snap = Some(ViewSnap {
            account: public_account(account.as_ref()),
            catalog: catalog.clone(),
            imported: imported.clone(),
            enabled,
            coexist: *coexist_flag,
            providers: providers.clone(),
            pool: pool.clone(),
            grok_slots: grok_pool.slots.clone(),
            grok_mode: grok_pool.mode,
            grok_active: grok_pool.active_id.clone(),
            pending_oauth: pending.clone(),
        });
    }

    fn busy_now(&self) -> Busy {
        self.busy.lock().map(|b| b.clone()).unwrap_or_default()
    }

    fn patch_busy(&self, patch: impl FnOnce(&mut Busy)) {
        if let Ok(mut busy) = self.busy.lock() {
            patch(&mut busy);
        }
    }

    fn set_notice(&self, text: impl Into<String>) {
        if let Ok(mut notice) = self.notice.lock() {
            *notice = text.into();
        }
    }

    fn notice(&self) -> String {
        self.notice.lock().map(|n| n.clone()).unwrap_or_default()
    }
}

impl eframe::App for GrokBotApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Err(error) = self.rt.block_on(park_cursor_mitm(&self.state)) {
            eprintln!("park Cursor 反代 failed: {error}");
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = crate::sand::now_ms();
        if self.last_quota_ms == 0 || now.saturating_sub(self.last_quota_ms) >= 60_000 {
            self.last_quota_ms = now;
            self.refresh_quotas_silent();
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(15));
        if self.show_window.swap(false, Ordering::SeqCst) {
            self.tray_hidden = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            crate::tray::show_native_window();
        }
        if crate::tray::quit_requested(self.allow_exit.load(Ordering::SeqCst)) {
            if let Err(error) = self.rt.block_on(park_cursor_mitm(&self.state)) {
                eprintln!("park Cursor 反代 failed: {error}");
            }
            std::process::exit(0);
        }
        if ctx.input(|i| i.viewport().close_requested())
            && crate::tray::close_hides_to_tray(self.allow_exit.load(Ordering::SeqCst))
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if crate::tray::should_apply_hide(
                self.allow_exit.load(Ordering::SeqCst),
                self.tray_hidden,
            ) {
                self.tray_hidden = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                self.set_notice("已放到托盘。托盘点「打开」回到窗口，「退出」才停。");
            }
        }
        let busy = self.busy_now();
        if busy.any() {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        self.pull_snap();
        if self.snap.is_none() {
            let snapshot = self.rt.block_on(async {
                let account = public_account(self.state.account.lock().await.as_ref());
                let catalog = self.state.models.lock().await.clone();
                let imported = self.state.imported.lock().await.clone();
                let enabled = self.state.enabled.lock().await.clone();
                let coexist = *self.state.coexist.lock().await;
                let providers = self.state.providers.lock().await.clone();
                let pool = self.state.prompt_pool.lock().await.clone();
                let grok_pool = self.state.grok_pool.lock().await.clone();
                let pending_oauth = self.state.pending_oauth.lock().await.clone();
                ViewSnap {
                    account,
                    catalog,
                    imported,
                    enabled,
                    coexist,
                    providers,
                    pool,
                    grok_slots: grok_pool.slots,
                    grok_mode: grok_pool.mode,
                    grok_active: grok_pool.active_id,
                    pending_oauth,
                }
            });
            self.snap = Some(snapshot);
        }
        let ViewSnap {
            account,
            catalog,
            imported,
            enabled,
            coexist,
            providers,
            pool,
            grok_slots,
            grok_mode,
            grok_active,
            pending_oauth,
        } = self.snap.clone().unwrap();
        if self.effort.is_empty() {
            self.effort = "high".into();
        }
        if let Ok(mut slot) = self.oauth_fill.lock() {
            if let Some(token) = slot.take() {
                self.provider_key = token;
                self.provider_auth = 1;
            }
        }
        if let Ok(mut slot) = self.catalog_fill.lock() {
            if let Some(ids) = slot.take() {
                self.provider_catalog = ids;
                // User picks which models enter the mapping table.
                self.provider_picked.clear();
                self.provider_rows.clear();
            }
        }
        if let Ok(mut slot) = self.ping_fill.lock() {
            if let Some(text) = slot.take() {
                self.ping_label = text.clone();
                self.set_notice(text);
            }
        }

        egui::TopBottomPanel::top("header")
            .exact_height(58.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(24, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (mark, _) = ui.allocate_exact_size(Vec2::splat(26.0), egui::Sense::hover());
                    theme::paint_glyph(ui, mark, Glyph::Spark, theme::ACCENT);
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("Grok-Bot-Auth")
                            .size(17.0)
                            .strong()
                            .color(theme::TEXT),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        theme::pill(
                            ui,
                            if coexist {
                                "Cursor 已接"
                            } else {
                                "Cursor 未接"
                            },
                            coexist,
                        );
                        ui.add_space(6.0);
                        theme::pill(
                            ui,
                            if account.exhausted {
                                "额度用尽"
                            } else if account.signed_in {
                                "已登录"
                            } else {
                                "未登录"
                            },
                            account.signed_in && !account.exhausted,
                        );
                    });
                });
            });

        egui::TopBottomPanel::top("tabs")
            .exact_height(52.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(24, 8)),
            )
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .corner_radius(10.0)
                    .inner_margin(egui::Margin::same(4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            for (pane, glyph, label) in [
                                (Pane::Overview, Glyph::Overview, "概览"),
                                (Pane::Chat, Glyph::Chat, "测试对话"),
                                (Pane::Models, Glyph::Models, "模型"),
                                (Pane::Providers, Glyph::Providers, "供应商"),
                                (Pane::Accounts, Glyph::User, "账号"),
                                (Pane::Ext, Glyph::Pool, "扩展"),
                                (Pane::Settings, Glyph::Settings, "设置"),
                            ] {
                                if theme::top_tab(ui, self.pane == pane, glyph, label).clicked() {
                                    self.pane = pane;
                                }
                            }
                        });
                    });
            });

        egui::TopBottomPanel::bottom("status")
            .exact_height(28.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(20, 6)),
            )
            .show(ctx, |ui| {
                let notice = self.notice();
                ui.horizontal(|ui| {
                    if busy.any() {
                        motion::status_spinner(ui);
                    }
                    ui.label(
                        RichText::new(if notice.is_empty() {
                            "上方切换页面"
                        } else {
                            notice.as_str()
                        })
                        .size(12.0)
                        .color(theme::MUTED),
                    );
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(28, 16)),
            )
            .show(ctx, |ui| {
                match self.pane {
                    Pane::Overview => {
                        self.draw_overview(ui, &account, coexist, &catalog, &enabled, &providers)
                    }
                    Pane::Chat => {
                        self.draw_chat(ui, &catalog, &enabled, &imported, &providers, &busy)
                    }
                    Pane::Models => self.draw_models(
                        ui, &catalog, &imported, &enabled, &providers, coexist, &busy,
                    ),
                    Pane::Providers => self.draw_providers(
                        ui,
                        &providers,
                        &catalog,
                        &imported,
                        &busy,
                        account.signed_in,
                    ),
                    Pane::Accounts => self.draw_accounts(
                        ui,
                        &grok_slots,
                        grok_mode,
                        &grok_active,
                        &providers,
                        pending_oauth.as_ref(),
                        &busy,
                    ),
                    Pane::Ext => self.draw_ext(ui, &pool),
                    Pane::Settings => self.draw_settings(ui, coexist, &busy),
                }
                if busy.login && pending_oauth.is_none() {
                    motion::login_veil(ui, ui.input(|i| i.time));
                }
            });
    }
}

impl GrokBotApp {
    fn draw_chat(
        &mut self,
        ui: &mut egui::Ui,
        catalog: &[crate::sand::SandFamily],
        enabled: &[String],
        imported: &[String],
        providers: &[crate::providers::Provider],
        busy: &Busy,
    ) {
        if self.test_model.is_empty() || catalog.iter().all(|family| family.id != self.test_model) {
            if let Some(id) = pick_test_model(catalog, enabled, imported) {
                self.test_model = id;
            }
        }
        theme::page_head(
            ui,
            "测试对话",
            "用来试供应商和 Grok Bot 是否通。绿色「可用」才能走 Grok Bot。",
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new("来源").size(12.0).color(theme::MUTED));
            if theme::brand_chip(ui, Glyph::Spark, "Grok Bot", self.source == "grok").clicked() {
                self.source = "grok".into();
            }
            for provider in providers
                .iter()
                .filter(|p| crate::providers::visible_provider(p))
            {
                if theme::brand_chip(
                    ui,
                    theme::brand_glyph(provider.kind),
                    &provider.label,
                    self.source == provider.id,
                )
                .clicked()
                {
                    self.source = provider.id.clone();
                    self.provider_model = crate::providers::chat_model_for_provider(provider);
                    self.provider_catalog = provider.catalog.clone();
                    self.provider_effort = provider.effort.clone().unwrap_or_default();
                    self.provider_context = provider.context.clone().unwrap_or_default();
                    self.provider_rows = provider.model_map.clone();
                    self.provider_picked = provider.enabled_models.iter().cloned().collect();
                }
            }
        });
        ui.add_space(8.0);
        if self.source == "grok" {
            let selected = catalog.iter().find(|family| family.id == self.test_model);
            ui.horizontal(|ui| {
                ui.label(RichText::new("模型").size(12.0).color(theme::MUTED));
                let label = selected
                    .map(|family| family.display_name.clone())
                    .unwrap_or_else(|| "选择可用模型".into());
                egui::ComboBox::from_id_salt("test-grok-model")
                    .width(280.0)
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        for family in catalog.iter().filter(|family| {
                            family.available && crate::sand::grok_bot_inference_allowed(&family.id)
                        }) {
                            ui.selectable_value(
                                &mut self.test_model,
                                family.id.clone(),
                                &family.display_name,
                            );
                        }
                    });
                if let Some(family) = selected {
                    theme::pill(
                        ui,
                        if family.available {
                            "可用"
                        } else {
                            "仅目录"
                        },
                        family.available,
                    );
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if let Some(family) = selected {
                    let efforts = family.effort_values();
                    if !efforts.is_empty() {
                        if !efforts.iter().any(|item| item == &self.effort) {
                            self.effort = family
                                .default_effort()
                                .unwrap_or_else(|| efforts[0].clone());
                        }
                        ui.label(RichText::new("思考").size(12.0).color(theme::MUTED));
                        let effort_label = crate::providers::effort_display(&self.effort);
                        egui::ComboBox::from_id_salt("effort")
                            .width(140.0)
                            .selected_text(effort_label)
                            .show_ui(ui, |ui| {
                                for value in &efforts {
                                    let label = crate::providers::effort_display(value);
                                    ui.selectable_value(&mut self.effort, value.clone(), label);
                                }
                            });
                    }
                    if family.has_fast_axis() {
                        ui.checkbox(&mut self.test_fast, "Fast");
                    }
                    let windows = family.token_windows();
                    if !windows.is_empty() {
                        ui.label(RichText::new("上下文").size(12.0).color(theme::MUTED));
                        let current = self.context.parse::<u64>().ok();
                        if current.is_none() || !windows.iter().any(|n| Some(*n) == current) {
                            self.context = windows[0].to_string();
                        }
                        let ctx_label =
                            crate::sand::format_tokens(self.context.parse().unwrap_or(windows[0]));
                        egui::ComboBox::from_id_salt("context")
                            .selected_text(ctx_label)
                            .show_ui(ui, |ui| {
                                for n in windows {
                                    ui.selectable_value(
                                        &mut self.context,
                                        n.to_string(),
                                        crate::sand::format_tokens(n),
                                    );
                                }
                            });
                    }
                }
            });
        } else {
            let catalog: Vec<String> = providers
                .iter()
                .find(|p| p.id == self.source)
                .map(|p| {
                    if !p.enabled_models.is_empty() {
                        p.enabled_models.clone()
                    } else {
                        p.catalog.clone()
                    }
                })
                .filter(|ids| !ids.is_empty())
                .unwrap_or_else(|| self.provider_catalog.clone());
            ui.horizontal(|ui| {
                ui.label(RichText::new("模型").size(12.0).color(theme::MUTED));
                egui::ComboBox::from_id_salt("chat-provider-model")
                    .width(280.0)
                    .selected_text(if self.provider_model.is_empty() {
                        "选择已导入的模型".into()
                    } else {
                        self.provider_model.clone()
                    })
                    .show_ui(ui, |ui| {
                        for id in &catalog {
                            ui.selectable_value(&mut self.provider_model, id.clone(), id);
                        }
                    });
                ui.add(egui::TextEdit::singleline(&mut self.provider_model).desired_width(160.0));
            });
            if catalog.is_empty() {
                ui.label(
                    RichText::new("还没有清单。去「供应商」点「导入模型清单」，勾选后再保存。")
                        .size(12.0)
                        .color(theme::MUTED),
                );
            }
        }
        ui.add_space(10.0);
        let think = self
            .test_think
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default();
        let out = self.test_out.lock().map(|s| s.clone()).unwrap_or_default();
        let turns = self.history.lock().map(|h| h.clone()).unwrap_or_default();
        let input_h = 88.0;
        let scroll_h = (ui.available_height() - input_h - 8.0).max(120.0);
        egui::ScrollArea::vertical()
            .max_height(scroll_h)
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                if busy.chat && think.is_empty() && out.is_empty() {
                    ui.add_space(24.0);
                    motion::think_spinner(ui);
                    return;
                }
                if turns.is_empty() && out.is_empty() && think.is_empty() {
                    theme::empty_hint(ui, Glyph::Chat, "还没开始聊", "底下输入框打字，再点发送。");
                    return;
                }
                for turn in &turns {
                    draw_bubble(ui, turn.role == "user", &turn.text, "");
                }
                if let Some(last) = turns.last() {
                    if last.role == "user" && (!out.is_empty() || !think.is_empty()) {
                        draw_bubble(ui, false, &out, &think);
                    }
                } else if !out.is_empty() || !think.is_empty() {
                    draw_bubble(ui, false, &out, &think);
                }
            });
        ui.add_space(8.0);
        egui::Frame::new()
            .fill(theme::PANEL)
            .corner_radius(10.0)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.test_prompt)
                            .desired_rows(2)
                            .desired_width((ui.available_width() - 96.0).max(120.0))
                            .hint_text("说点什么… 点发送"),
                    );
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.add_space(6.0);
                        if motion::action_button(ui, Glyph::Send, "发送", true, busy.chat).clicked()
                        {
                            self.test_model_chat();
                        }
                    });
                });
            });
    }

    fn draw_models(
        &mut self,
        ui: &mut egui::Ui,
        catalog: &[crate::sand::SandFamily],
        imported: &[String],
        enabled: &[String],
        providers: &[crate::providers::Provider],
        coexist: bool,
        busy: &Busy,
    ) {
        theme::page_head(
            ui,
            "模型调度",
            "这里只列出你在「供应商 → Grok Bot」勾选添加的模型。点卡片选中，右侧开关才注入 Cursor。启用后必须完全退出 Cursor，不要 Reload。",
        );
        ui.horizontal(|ui| {
            if coexist {
                if motion::action_button(ui, Glyph::Plug, "停用共存", false, busy.cursor).clicked()
                {
                    self.disable_cursor();
                }
            } else if motion::action_button(ui, Glyph::Plug, "启用共存", true, busy.cursor)
                .clicked()
            {
                self.enable_cursor();
            }
            theme::pill(
                ui,
                if coexist {
                    "Cursor 已接"
                } else {
                    "Cursor 未接"
                },
                coexist,
            );
        });
        if busy.models && !catalog.is_empty() {
            ui.add_space(8.0);
            motion::thin_bar(ui, ui.input(|i| i.time));
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let (icon, _) = ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::hover());
            theme::paint_glyph(ui, icon, Glyph::Search, theme::MUTED);
            ui.add(
                egui::TextEdit::singleline(&mut self.model_filter)
                    .hint_text("按名字筛一下…")
                    .desired_width(240.0),
            );
        });
        ui.add_space(8.0);
        if catalog.is_empty() {
            if busy.models {
                motion::skeleton_grid(ui, ui.input(|i| i.time));
            } else {
                theme::empty_hint(
                    ui,
                    Glyph::Models,
                    "还没有目录",
                    "先去「供应商 → Grok Bot」登录，再点登录卡片上的「同步模型」。",
                );
            }
            return;
        }
        let q = self.model_filter.to_ascii_lowercase();
        let rows: Vec<_> = crate::sand::present_catalog(
            catalog
                .iter()
                .filter(|family| family.id != "default")
                .filter(|family| imported.iter().any(|id| id == &family.id))
                .filter(|family| {
                    q.is_empty()
                        || family.display_name.to_ascii_lowercase().contains(&q)
                        || family.id.to_ascii_lowercase().contains(&q)
                })
                .cloned()
                .collect(),
        );
        let extra: Vec<(String, String, String)> = providers
            .iter()
            .filter(|p| crate::providers::visible_provider(p))
            .flat_map(|p| {
                p.enabled_models.iter().cloned().map(|model| {
                    (
                        crate::catalog::provider_enable_id(&p.id, &model),
                        format!("{} · {}", model, p.label),
                        model,
                    )
                })
            })
            .collect();
        if rows.is_empty() && extra.is_empty() {
            theme::empty_hint(
                ui,
                Glyph::Models,
                "还没有已添加的模型",
                "去「供应商 → Grok Bot」同步目录，勾选要添加的模型。本页右侧开关再注入 Cursor。",
            );
            return;
        }
        let selected = catalog.iter().find(|family| family.id == self.test_model);
        if let Some(family) = selected {
            ui.label(
                RichText::new(format!(
                    "当前模型  {}  ·  gb-{}",
                    family.display_name, family.id
                ))
                .size(12.0)
                .color(theme::MUTED),
            );
            ui.add_space(6.0);
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let cols = theme::model_grid_cols(ui.available_width());
                let gap = 8.0;
                let card_w = ((ui.available_width() - gap * (cols.saturating_sub(1) as f32))
                    / cols as f32)
                    .floor()
                    .max(160.0);
                if !extra.is_empty() {
                    ui.label(
                        RichText::new(
                            "供应商模型（卡片带供应商名；开关只动这一家，不会连坐 Grok Bot）",
                        )
                        .size(12.0)
                        .color(theme::MUTED),
                    );
                    ui.add_space(6.0);
                    for chunk in extra.chunks(cols) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            for (key, name, _) in chunk {
                                let on = enabled.iter().any(|e| e == key);
                                let hit = theme::model_card(ui, card_w, name, false, false, on);
                                if hit.start {
                                    self.set_enabled(key, !on);
                                } else if hit.select {
                                    self.test_model = key.clone();
                                }
                            }
                        });
                        ui.add_space(gap);
                    }
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("Grok Bot 目录")
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                    ui.add_space(6.0);
                }
                for chunk in rows.chunks(cols) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        for family in chunk {
                            let on = enabled.iter().any(|id| id == &family.id);
                            let hit = theme::model_card(
                                ui,
                                card_w,
                                &format!("{} · Grok Bot", family.display_name),
                                false,
                                family.id == self.test_model,
                                on,
                            );
                            if hit.select {
                                self.test_model = family.id.clone();
                            }
                            if hit.start {
                                self.set_enabled(&family.id, !on);
                            }
                        }
                    });
                    ui.add_space(gap);
                }
            });
    }

    fn draw_grok_bot_login(&mut self, ui: &mut egui::Ui, signed_in: bool, busy: &Busy) {
        egui::Frame::new()
            .fill(theme::PANEL)
            .corner_radius(12.0)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width().max(480.0));
                ui.label(RichText::new("Grok Bot 登录").size(16.0).strong());
                ui.label(
                    RichText::new(if signed_in {
                        "官方 Cursor / Grok Bot 会话已接上。第三方供应商在下方单独配置。"
                    } else {
                        "用 Cursor 账号登录 Grok Bot，不是第三方供应商。"
                    })
                    .size(12.0)
                    .color(theme::MUTED),
                );
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    let login_label = if signed_in {
                        "添加账号"
                    } else {
                        "Grok Bot OAuth 登录"
                    };
                    if motion::action_button(ui, Glyph::Session, login_label, true, busy.login)
                        .clicked()
                    {
                        self.start_login();
                    }
                    if signed_in {
                        if motion::action_button(ui, Glyph::Models, "同步模型", true, busy.models)
                            .clicked()
                        {
                            self.fetch_models();
                        }
                        if ui.button("刷新会话").clicked() {
                            self.refresh_session();
                        }
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("账号邮箱").size(12.0).color(theme::MUTED));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.oauth_email)
                            .hint_text("可选，JWT 没有 email 时用来区分账号")
                            .desired_width(280.0),
                    );
                });
                if let Some(pending) = self.snap.as_ref().and_then(|s| s.pending_oauth.clone()) {
                    ui.add_space(8.0);
                    self.draw_pending_oauth(ui, &pending);
                }
                ui.add_space(8.0);
                if ui
                    .add(egui::Button::new(if self.show_json { "收起 JSON" } else { "我有 session JSON" }).frame(false))
                    .clicked()
                {
                    self.show_json = !self.show_json;
                }
                if self.show_json {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.import_json)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY)
                            .hint_text("accessToken / refreshToken / machineId"),
                    );
                    if theme::icon_button(ui, Glyph::Spark, "导入这段 JSON", false).clicked() {
                        self.import_session();
                    }
                }
                ui.add_space(8.0);
                ui.label(
                    RichText::new("登录后点「同步模型」拉官方目录。勾选要添加的模型；「可用」只在本页显示，不会写进 Cursor 选择器。")
                        .size(12.0)
                        .color(theme::MUTED),
                );
            });
    }

    fn draw_pending_oauth(&mut self, ui: &mut egui::Ui, pending: &crate::accounts::PendingOauth) {
        egui::Frame::new()
            .fill(theme::FIELD)
            .stroke(egui::Stroke::new(1.0, theme::ACCENT))
            .corner_radius(10.0)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.label(RichText::new("OAuth 进行中").size(14.0).strong());
                ui.label(RichText::new(&pending.hint).size(12.0).color(theme::MUTED));
                if let Some(code) = &pending.user_code {
                    ui.label(
                        RichText::new(format!("设备码  {code}"))
                            .size(16.0)
                            .strong()
                            .color(theme::ACCENT),
                    );
                }
                ui.add_space(6.0);
                ui.label(RichText::new("登录网址").size(11.0).color(theme::MUTED));
                let mut url = pending.url.clone();
                ui.add(
                    egui::TextEdit::multiline(&mut url)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal_wrapped(|ui| {
                    if ui.button("复制网址").clicked() {
                        ui.ctx().copy_text(pending.url.clone());
                        self.set_notice("已复制登录网址");
                    }
                    if ui.button("打开浏览器").clicked() {
                        if let Err(error) = crate::open::open_url(&pending.url) {
                            self.set_notice(format!("无法打开：{error}"));
                        }
                    }
                    if ui.button("已完成，核实").clicked() {
                        self.verify_oauth();
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("取消").color(theme::DANGER))
                                .fill(theme::DANGER_DIM),
                        )
                        .clicked()
                    {
                        self.cancel_oauth();
                    }
                });
            });
    }

    fn draw_quota_bar(ui: &mut egui::Ui, percent: f64) {
        let width = ui.available_width().max(80.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 8.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 4.0, theme::FIELD);
        let fill_w = (rect.width() * (percent.clamp(0.0, 100.0) as f32) / 100.0).max(2.0);
        let fill = egui::Rect::from_min_size(rect.min, Vec2::new(fill_w, rect.height()));
        let color = if percent <= 8.0 {
            theme::DANGER
        } else if percent <= 25.0 {
            Color32::from_rgb(232, 168, 72)
        } else {
            theme::OK
        };
        ui.painter().rect_filled(fill, 4.0, color);
    }

    fn draw_accounts(
        &mut self,
        ui: &mut egui::Ui,
        grok_slots: &[crate::sand::Account],
        grok_mode: crate::accounts::PoolMode,
        grok_active: &str,
        providers: &[crate::providers::Provider],
        pending: Option<&crate::accounts::PendingOauth>,
        _busy: &Busy,
    ) {
        theme::page_head(
            ui,
            "账号管理",
            "Grok Bot 周用量与各供应商账号分开；额度只在同一供应商内共享或轮换。",
        );
        if let Some(pending) = pending {
            self.draw_pending_oauth(ui, pending);
            ui.add_space(12.0);
        }
        ui.add_space(10.0);
        egui::ScrollArea::vertical()
            .id_salt("accounts-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Grok Bot").size(14.0).strong());
                    ui.add_space(8.0);
                    for mode in [
                        crate::accounts::PoolMode::Share,
                        crate::accounts::PoolMode::QuotaFirst,
                    ] {
                        if theme::brand_chip(ui, Glyph::User, mode.label(), grok_mode == mode)
                            .clicked()
                        {
                            self.set_pool_mode(mode);
                        }
                    }
                });
                ui.add_space(6.0);
                if grok_slots.is_empty() {
                    theme::empty_hint(
                        ui,
                        Glyph::User,
                        "还没有账号",
                        "去「供应商 → Grok Bot」登录或导入 JSON。可添加多个邮箱账号。",
                    );
                }
                let now = crate::sand::now_ms();
                for slot in grok_slots {
                    let id = crate::sand::account_id(slot);
                    let active = id == grok_active;
                    egui::Frame::new()
                        .fill(theme::PANEL)
                        .stroke(egui::Stroke::new(
                            if active { 1.4 } else { 0.0 },
                            theme::ACCENT,
                        ))
                        .corner_radius(10.0)
                        .inner_margin(egui::Margin::symmetric(12, 10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            RichText::new(if slot.email.is_empty() {
                                                slot.display_name.clone()
                                            } else {
                                                slot.email.clone()
                                            })
                                            .size(14.0)
                                            .strong(),
                                        );
                                        if active {
                                            theme::pill(ui, "当前", true);
                                        }
                                        if slot.exhausted {
                                            theme::pill(ui, "额度不足", false);
                                        }
                                    });
                                    if let Some(quota) = &slot.quota {
                                        ui.label(
                                            RichText::new(quota.summary(now))
                                                .size(12.0)
                                                .color(theme::MUTED),
                                        );
                                        if let Some(pct) = quota.remaining_percent {
                                            Self::draw_quota_bar(ui, pct);
                                        }
                                    } else {
                                        ui.label(
                                            RichText::new("点「刷新 Grok Bot 额度」拉取周用量（不是 Cursor 套餐）")
                                                .size(11.0)
                                                .color(theme::MUTED),
                                        );
                                    }
                                    if let Some(err) = &slot.last_error {
                                        ui.label(
                                            RichText::new(err).size(11.0).color(theme::DANGER),
                                        );
                                    }
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new("移除").color(theme::DANGER),
                                            )
                                            .fill(theme::DANGER_DIM)
                                            .small(),
                                        )
                                        .clicked()
                                    {
                                        self.remove_account(&id);
                                    }
                                    if !active && ui.small_button("切到").clicked() {
                                        self.activate_account(&id);
                                    }
                                });
                            });
                        });
                    ui.add_space(8.0);
                }
                ui.add_space(12.0);
                ui.label(RichText::new("第三方供应商 OAuth").size(14.0).strong());
                ui.add_space(6.0);
                let mut any_provider = false;
                for provider in providers.iter().filter(|p| crate::providers::visible_provider(p))
                {
                    let slots = crate::providers::oauth_slots_for_display(provider);
                    if slots.is_empty() {
                        continue;
                    }
                    any_provider = true;
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(format!("{} · {}", provider.label, provider.kind.as_str()))
                                .size(13.0)
                                .strong(),
                        );
                        for mode in [
                            crate::accounts::PoolMode::Share,
                            crate::accounts::PoolMode::QuotaFirst,
                        ] {
                            if theme::brand_chip(
                                ui,
                                Glyph::User,
                                mode.label(),
                                provider.pool_mode == mode,
                            )
                            .clicked()
                            {
                                self.set_provider_pool_mode(&provider.id, mode);
                            }
                        }
                    });
                    for acc in &slots {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&acc.email).size(13.0).strong());
                                if let Some(quota) = &acc.quota {
                                    ui.label(
                                        RichText::new(quota.summary(crate::sand::now_ms()))
                                            .size(12.0)
                                            .color(theme::MUTED),
                                    );
                                } else if let Some(exp) = crate::sand::jwt_exp(&acc.token) {
                                    let now = crate::sand::now_ms() / 1000;
                                    let text = if exp > now {
                                        format!("令牌约 {} 小时后到期", (exp - now) / 3600)
                                    } else {
                                        "令牌已过期，请在供应商页重新授权".into()
                                    };
                                    ui.label(
                                        RichText::new(text).size(11.0).color(theme::MUTED),
                                    );
                                }
                            });
                            if acc.exhausted {
                                theme::pill(ui, "额度不足", false);
                            }
                            if crate::accounts::normalize_email(&provider.active_email)
                                == crate::accounts::normalize_email(&acc.email)
                            {
                                theme::pill(ui, "当前", true);
                            }
                        });
                    }
                    ui.add_space(8.0);
                }
                if !any_provider {
                    ui.label(
                        RichText::new("第三方 OAuth 账号会在对应供应商保存后出现在这里。")
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                }
            });
    }

    fn draw_grok_catalog(
        &mut self,
        ui: &mut egui::Ui,
        catalog: &[crate::sand::SandFamily],
        imported: &[String],
        busy: &Busy,
    ) {
        egui::Frame::new()
            .fill(theme::PANEL)
            .corner_radius(12.0)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.label(RichText::new("从官方目录添加").size(14.0).strong());
                ui.label(
                    RichText::new("旋钮跟 Grok Bot AvailableModels。Grok 4.6 目录是 256K；Cursor 官方选择器里的 200K/500K 是 Cursor Max Mode，不是 Grok Bot Stream 窗口。")
                        .size(12.0)
                        .color(theme::MUTED),
                );
                ui.add_space(8.0);
                if catalog.is_empty() {
                    if busy.models {
                        motion::thin_bar(ui, ui.input(|i| i.time));
                    } else {
                        ui.label(
                            RichText::new("还没有目录。登录后点上面的「同步模型」。")
                                .size(12.0)
                                .color(theme::MUTED),
                        );
                    }
                    return;
                }
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.grok_filter)
                            .hint_text("筛模型…")
                            .desired_width(220.0),
                    );
                    if ui.button("添加勾选").clicked() {
                        self.add_imported();
                    }
                });
                ui.add_space(10.0);
                let q = self.grok_filter.to_ascii_lowercase();
                let listed: Vec<_> = catalog
                    .iter()
                    .filter(|family| family.id != "default")
                    .filter(|family| !imported.iter().any(|id| id == &family.id))
                    .filter(|family| {
                        q.is_empty()
                            || family.display_name.to_ascii_lowercase().contains(&q)
                            || family.id.to_ascii_lowercase().contains(&q)
                    })
                    .cloned()
                    .collect();
                if listed.is_empty() {
                    ui.label(
                        RichText::new(if imported.is_empty() {
                            "没有匹配的模型。"
                        } else {
                            "目录里能加的都已经添加了。"
                        })
                        .size(12.0)
                        .color(theme::MUTED),
                    );
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .id_salt("grok-catalog-pick")
                        .show(ui, |ui| {
                            for family in &listed {
                                ui.horizontal(|ui| {
                                    let mut on = self.checked.contains(&family.id);
                                    if ui.checkbox(&mut on, "").changed() {
                                        if on {
                                            self.checked.insert(family.id.clone());
                                        } else {
                                            self.checked.remove(&family.id);
                                        }
                                    }
                                    ui.vertical(|ui| {
                                        ui.horizontal_wrapped(|ui| {
                                            ui.label(
                                                RichText::new(&family.display_name)
                                                    .size(13.0)
                                                    .strong(),
                                            );
                                            theme::pill(
                                                ui,
                                                if family.available {
                                                    "可用"
                                                } else {
                                                    "仅目录"
                                                },
                                                family.available,
                                            );
                                        });
                                        ui.label(
                                            RichText::new(format!(
                                                "{}  ·  {}",
                                                family.id,
                                                family.knob_summary()
                                            ))
                                            .size(11.0)
                                            .color(theme::MUTED),
                                        );
                                    });
                                });
                                ui.add_space(4.0);
                            }
                        });
                }
            });
        ui.add_space(12.0);
        egui::Frame::new()
            .fill(theme::PANEL)
            .corner_radius(12.0)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.label(RichText::new("已添加").size(14.0).strong());
                ui.label(
                    RichText::new("这些会出现在「模型调度」。去那边开右侧开关才会进 Cursor。")
                        .size(12.0)
                        .color(theme::MUTED),
                );
                ui.add_space(8.0);
                let added: Vec<_> = catalog
                    .iter()
                    .filter(|family| imported.iter().any(|id| id == &family.id))
                    .cloned()
                    .collect();
                if added.is_empty() {
                    ui.label(
                        RichText::new("还没添加。上面勾选后点「添加勾选」。")
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                    return;
                }
                let mut drop: Option<String> = None;
                for family in &added {
                    egui::Frame::new()
                        .fill(theme::FIELD)
                        .corner_radius(10.0)
                        .inner_margin(egui::Margin::symmetric(12, 10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            RichText::new(&family.display_name).size(14.0).strong(),
                                        );
                                        theme::pill(
                                            ui,
                                            if family.available {
                                                "可用"
                                            } else {
                                                "仅目录"
                                            },
                                            family.available,
                                        );
                                    });
                                    ui.label(
                                        RichText::new(family.id.clone())
                                            .size(11.0)
                                            .color(theme::MUTED),
                                    );
                                    ui.label(
                                        RichText::new(family.knob_summary())
                                            .size(11.0)
                                            .color(theme::MUTED),
                                    );
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new("移除").color(theme::DANGER),
                                            )
                                            .fill(theme::DANGER_DIM)
                                            .small(),
                                        )
                                        .clicked()
                                    {
                                        drop = Some(family.id.clone());
                                    }
                                });
                            });
                        });
                    ui.add_space(8.0);
                }
                if let Some(id) = drop {
                    self.remove_imported(&id);
                }
            });
    }

    fn draw_providers(
        &mut self,
        ui: &mut egui::Ui,
        providers: &[crate::providers::Provider],
        catalog: &[crate::sand::SandFamily],
        imported: &[String],
        busy: &Busy,
        signed_in: bool,
    ) {
        theme::page_head(
            ui,
            "供应商",
            "Grok Bot 登录、同步目录、勾选要添加的模型都在这一页。第三方供应商另外配置。",
        );
        egui::ScrollArea::vertical()
            .id_salt("providers-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.scroll_to_form {
                    ui.scroll_to_cursor(Some(Align::TOP));
                    self.scroll_to_form = false;
                }
                self.draw_kind_chips(ui);
                ui.add_space(12.0);
                if self.provider_kind == KIND_GROK_BOT {
                    self.draw_grok_bot_login(ui, signed_in, busy);
                    ui.add_space(12.0);
                    self.draw_grok_catalog(ui, catalog, imported, busy);
                } else {
                    self.draw_providers_inner(ui, busy);
                }
                ui.add_space(16.0);
                self.draw_saved_providers(ui, providers);
            });
    }

    fn draw_kind_chips(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(theme::PANEL)
            .corner_radius(12.0)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.label(RichText::new("类型").size(12.0).color(theme::MUTED));
                ui.horizontal_wrapped(|ui| {
                    for (i, glyph, label) in [
                        (KIND_GROK_BOT, Glyph::Spark, "Grok Bot"),
                        (KIND_OPENAI, Glyph::Openai, "OpenAI"),
                        (KIND_XAI, Glyph::Xai, "xAI"),
                        (KIND_ANTHROPIC, Glyph::Anthropic, "Anthropic"),
                        (KIND_GEMINI, Glyph::Spark, "Gemini"),
                        (KIND_ZHIPU, Glyph::Models, "智谱"),
                        (KIND_KIMI, Glyph::Bot, "Kimi"),
                        (KIND_DEEPSEEK, Glyph::Search, "DeepSeek"),
                        (KIND_GENERIC, Glyph::Providers, "通用"),
                    ] {
                        if theme::brand_chip(ui, glyph, label, self.provider_kind == i).clicked() {
                            self.apply_kind(i, label);
                        }
                    }
                });
            });
    }

    fn apply_kind(&mut self, kind: usize, label: &str) {
        self.provider_kind = kind;
        self.ping_label.clear();
        if kind == KIND_GROK_BOT {
            return;
        }
        let mapped = ui_kind(kind);
        self.provider_base = mapped.default_base_url().into();
        self.provider_label = label.into();
        self.provider_model = crate::providers::default_model_for(&mapped).into();
        self.provider_wire = match mapped.default_wire() {
            crate::providers::WireFormat::ChatCompletions => 0,
            crate::providers::WireFormat::Responses => 1,
            crate::providers::WireFormat::AnthropicMessages => 2,
        };
        if !crate::providers::supports_oauth(ui_kind(kind)) {
            self.provider_auth = 0;
        }
        self.provider_catalog.clear();
        self.provider_picked.clear();
        self.provider_rows.clear();
        self.editing_id.clear();
    }

    fn draw_providers_inner(&mut self, ui: &mut egui::Ui, busy: &Busy) {
        if self.provider_kind == KIND_GENERIC {
            self.provider_auth = 0;
        }
        let oauth_ok = crate::providers::supports_oauth(ui_kind(self.provider_kind));
        egui::Frame::new()
            .fill(theme::PANEL)
            .corner_radius(12.0)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("名称").size(12.0).color(theme::MUTED));
                    ui.add(egui::TextEdit::singleline(&mut self.provider_label).desired_width(180.0));
                });
                ui.add_space(8.0);
                ui.label(RichText::new("API 请求地址").size(12.0).color(theme::MUTED));
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.provider_base).desired_width(360.0));
                    if ui.button("测速").clicked() {
                        self.ping_provider();
                    }
                    if !self.ping_label.is_empty() {
                        let color = if self.ping_label.starts_with("测速失败") {
                            theme::DANGER
                        } else {
                            theme::OK
                        };
                        ui.label(RichText::new(&self.ping_label).size(12.0).color(color));
                    }
                });
                ui.add_space(8.0);
                if oauth_ok {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("登录").size(12.0).color(theme::MUTED));
                        if theme::brand_chip(ui, Glyph::Providers, "API Key", self.provider_auth == 0)
                            .clicked()
                        {
                            self.provider_auth = 0;
                        }
                        if theme::brand_chip(ui, Glyph::Session, "OAuth 登录", self.provider_auth == 1)
                            .clicked()
                        {
                            self.provider_auth = 1;
                        }
                        if self.provider_auth == 1 && ui.button("打开官方登录").clicked() {
                            self.start_provider_oauth();
                        }
                    });
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(if self.provider_auth == 1 {
                            "OAuth token（xAI 设备授权会自动填；OpenAI/Anthropic 请在浏览器登录后把 key 贴这里）"
                        } else {
                            "API Key"
                        })
                        .size(12.0)
                        .color(theme::MUTED),
                    );
                } else {
                    ui.label(RichText::new("API Key（该类型没有官方 OAuth，请用控制台密钥）").size(12.0).color(theme::MUTED));
                }
                ui.add(egui::TextEdit::singleline(&mut self.provider_key).password(true));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("上游格式").size(12.0).color(theme::MUTED));
                    for (i, label) in [
                        (0, "Chat Completions"),
                        (1, "Responses"),
                        (2, "Anthropic Messages"),
                    ] {
                        if theme::brand_chip(ui, Glyph::Models, label, self.provider_wire == i)
                            .clicked()
                        {
                            self.provider_wire = i;
                        }
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("默认模型").size(12.0).color(theme::MUTED));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.provider_model).desired_width(180.0),
                    );
                    if ui.button("导入官方模型清单").clicked() {
                        self.import_provider_models();
                    }
                });
                if !self.provider_catalog.is_empty() {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!(
                            "已拉取 {} 个官方模型。默认不勾选；勾选后才会加入映射并注入 Cursor。",
                            self.provider_catalog.len()
                        ))
                        .size(12.0)
                        .color(theme::MUTED),
                    );
                    egui::ScrollArea::vertical()
                        .max_height(160.0)
                        .id_salt("provider-catalog")
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for id in self.provider_catalog.clone() {
                                    let mut on = self.provider_picked.contains(&id);
                                    if ui.checkbox(&mut on, &id).changed() {
                                        if on {
                                            self.provider_picked.insert(id.clone());
                                            if !self.provider_rows.iter().any(|row| row.alias == id)
                                            {
                                                let catalog = self
                                                    .snap
                                                    .as_ref()
                                                    .map(|snap| snap.catalog.as_slice())
                                                    .unwrap_or(&[]);
                                                self.provider_rows.push(
                                                    crate::providers::map_from_catalog(
                                                        &id, catalog,
                                                    ),
                                                );
                                            }
                                            if self.provider_model.is_empty() {
                                                self.provider_model = id.clone();
                                            }
                                        } else {
                                            self.provider_picked.remove(&id);
                                            self.provider_rows
                                                .retain(|row| row.alias != id && row.upstream != id);
                                        }
                                    }
                                }
                            });
                        });
                    ui.horizontal(|ui| {
                        if ui.small_button("全选").clicked() {
                            self.provider_picked = self.provider_catalog.iter().cloned().collect();
                            self.provider_rows = crate::providers::maps_for_picked(
                                &self.provider_catalog,
                                &self.provider_rows,
                            );
                            let catalog = self
                                .snap
                                .as_ref()
                                .map(|snap| snap.catalog.as_slice())
                                .unwrap_or(&[]);
                            for row in &mut self.provider_rows {
                                crate::providers::apply_official_knobs(row, catalog);
                            }
                        }
                        if ui.small_button("清空勾选").clicked() {
                            self.provider_picked.clear();
                            self.provider_rows.clear();
                        }
                    });
                }
                ui.add_space(10.0);
                ui.label(
                    RichText::new("模型映射")
                        .size(13.0)
                        .strong(),
                );
                ui.label(
                    RichText::new("思考等级划分：none / minimal / low / medium / high / xhigh / max / ultra。导入官方清单时只勾该模型目录里有的档；Fast 和上下文同样对照官方，不编最高能力。")
                        .size(12.0)
                        .color(theme::MUTED),
                );
                ui.add_space(6.0);
                self.draw_mapping_table(ui);
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if motion::action_button(
                        ui,
                        Glyph::Providers,
                        "保存这个供应商",
                        true,
                        busy.provider,
                    )
                    .clicked()
                    {
                        self.save_provider();
                    }
                    if !self.editing_id.is_empty()
                        && ui
                            .add(
                                egui::Button::new(
                                    RichText::new("删除当前供应商").color(theme::DANGER),
                                )
                                .fill(theme::DANGER_DIM),
                            )
                            .clicked()
                    {
                        let id = self.editing_id.clone();
                        self.delete_provider(&id);
                        self.editing_id.clear();
                    }
                });
            });
    }

    fn draw_mapping_table(&mut self, ui: &mut egui::Ui) {
        if self.provider_rows.is_empty() {
            ui.label(
                RichText::new("勾选上方模型后才会出现映射行，也可以手动加行。")
                    .size(12.0)
                    .color(theme::MUTED),
            );
        }
        let mut drop_row: Option<usize> = None;
        let width = ui.available_width();
        let catalog = self
            .snap
            .as_ref()
            .map(|snap| snap.catalog.clone())
            .unwrap_or_default();
        for (index, row) in self.provider_rows.iter_mut().enumerate() {
            egui::Frame::new()
                .fill(theme::FIELD)
                .corner_radius(10.0)
                .inner_margin(egui::Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.set_width(width - 8.0);
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut row.alias)
                                .desired_width((width * 0.42).max(220.0))
                                .hint_text("显示名"),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut row.upstream)
                                .desired_width((width * 0.42).max(220.0))
                                .hint_text("上游 ID"),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(RichText::new("删").color(theme::DANGER))
                                        .fill(theme::DANGER_DIM)
                                        .small(),
                                )
                                .clicked()
                            {
                                drop_row = Some(index);
                            }
                        });
                    });
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(crate::providers::official_knob_line(&row.alias, &catalog))
                            .size(11.0)
                            .color(theme::MUTED),
                    );
                    ui.add_space(6.0);
                    ui.label(RichText::new("思考等级").size(11.0).color(theme::MUTED));
                    ui.horizontal_wrapped(|ui| {
                        for grade in crate::providers::THINKING_GRADES {
                            let mut on = row.efforts.iter().any(|item| item == grade);
                            if ui.checkbox(&mut on, *grade).changed() {
                                if on {
                                    if !row.efforts.iter().any(|item| item == grade) {
                                        row.efforts.push((*grade).to_owned());
                                    }
                                } else {
                                    row.efforts.retain(|item| item != grade);
                                }
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("预设等级").size(11.0).color(theme::MUTED));
                        let grades = row.efforts.clone();
                        let mut current = row
                            .effort
                            .clone()
                            .filter(|value| !value.is_empty())
                            .unwrap_or_else(|| "auto".into());
                        egui::ComboBox::from_id_salt(format!("effort-default-{index}"))
                            .selected_text(if current == "auto" {
                                "自动".into()
                            } else {
                                crate::sand::axis_value_label(&current)
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut current, "auto".into(), "自动");
                                for grade in &grades {
                                    ui.selectable_value(
                                        &mut current,
                                        grade.clone(),
                                        crate::sand::axis_value_label(grade),
                                    );
                                }
                            });
                        row.effort = Some(current);
                        let mut fast = row.fast.unwrap_or(false);
                        if ui.checkbox(&mut fast, "Fast").changed() {
                            row.fast = Some(fast);
                        }
                    });
                });
            ui.add_space(8.0);
        }
        if let Some(index) = drop_row {
            let removed = self.provider_rows.remove(index);
            self.provider_picked.remove(&removed.alias);
            self.provider_picked.remove(&removed.upstream);
        }
        if ui.small_button("加一行映射").clicked() {
            self.provider_rows
                .push(crate::providers::ModelMap::default());
        }
    }

    fn draw_saved_providers(
        &mut self,
        ui: &mut egui::Ui,
        providers: &[crate::providers::Provider],
    ) {
        let listed: Vec<_> = providers
            .iter()
            .filter(|p| crate::providers::visible_provider(p))
            .cloned()
            .collect();
        ui.label(RichText::new("已保存的供应商").size(13.0).strong());
        if listed.is_empty() {
            ui.label(
                RichText::new("还没有第三方。选 OpenAI / xAI / Anthropic / 通用，填地址后保存。")
                    .size(12.0)
                    .color(theme::MUTED),
            );
            return;
        }
        ui.add_space(6.0);
        for provider in &listed {
            let editing = self.editing_id == provider.id;
            egui::Frame::new()
                .fill(theme::PANEL)
                .stroke(egui::Stroke::new(
                    if editing { 1.4 } else { 0.0 },
                    theme::ACCENT,
                ))
                .corner_radius(10.0)
                .inner_margin(egui::Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (mark, _) =
                            ui.allocate_exact_size(Vec2::splat(28.0), egui::Sense::hover());
                        ui.painter().rect_filled(mark, 7.0, theme::ACCENT_DIM);
                        theme::paint_glyph(
                            ui,
                            mark.shrink(6.0),
                            theme::brand_glyph(provider.kind),
                            theme::ACCENT,
                        );
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&provider.label).size(14.0).strong());
                            ui.label(
                                RichText::new(format!(
                                    "{} · {} · {}",
                                    provider.kind.as_str(),
                                    provider.wire.label(),
                                    provider.base_url
                                ))
                                .size(11.0)
                                .color(theme::MUTED),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(RichText::new("删除").color(theme::DANGER))
                                        .fill(theme::DANGER_DIM),
                                )
                                .clicked()
                            {
                                self.delete_provider(&provider.id);
                            }
                            if ui.button("修改").clicked() {
                                self.load_provider(provider);
                                self.scroll_to_form = true;
                            }
                        });
                    });
                });
            ui.add_space(6.0);
        }
    }

    fn draw_pool(&mut self, ui: &mut egui::Ui, pool: &[String]) {
        theme::page_head(
            ui,
            "提示池",
            "写在这里的句子，每次发送都会自动加在你的话前面。历史压缩按官方 10% 预留，不用再调预算。",
        );
        ui.add(
            egui::TextEdit::multiline(&mut self.pool_draft)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text("例如：回答尽量短，先给结论"),
        );
        ui.add_space(8.0);
        if theme::icon_button(ui, Glyph::Pool, "加进提示池", true).clicked() {
            self.add_pool_entry();
        }
        ui.add_space(12.0);
        if pool.is_empty() {
            theme::empty_hint(
                ui,
                Glyph::Pool,
                "池子还是空的",
                "先加一条常用叮嘱，发对话时会自动带上。",
            );
            return;
        }
        for (i, entry) in pool.iter().enumerate() {
            egui::Frame::new()
                .fill(theme::PANEL)
                .corner_radius(10.0)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(entry).size(13.0));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if theme::icon_button(ui, Glyph::Trash, "去掉", false).clicked() {
                                self.remove_pool_entry(i);
                            }
                        });
                    });
                });
            ui.add_space(6.0);
        }
    }

    fn load_provider(&mut self, provider: &crate::providers::Provider) {
        self.editing_id = provider.id.clone();
        self.ping_label.clear();
        self.provider_kind = match provider.kind {
            crate::providers::ProviderKind::Openai => KIND_OPENAI,
            crate::providers::ProviderKind::Xai => KIND_XAI,
            crate::providers::ProviderKind::Anthropic => KIND_ANTHROPIC,
            crate::providers::ProviderKind::Gemini => KIND_GEMINI,
            crate::providers::ProviderKind::Zhipu => KIND_ZHIPU,
            crate::providers::ProviderKind::Kimi => KIND_KIMI,
            crate::providers::ProviderKind::Deepseek => KIND_DEEPSEEK,
            crate::providers::ProviderKind::Generic => KIND_GENERIC,
        };
        self.provider_label = provider.label.clone();
        self.provider_base = provider.base_url.clone();
        self.provider_key = if provider.oauth_token.is_empty() {
            provider.api_key.clone()
        } else {
            provider.oauth_token.clone()
        };
        self.provider_auth = if crate::providers::supports_oauth(provider.kind) {
            match provider.auth_mode {
                crate::providers::AuthMode::Oauth => 1,
                crate::providers::AuthMode::ApiKey => 0,
            }
        } else {
            0
        };
        self.provider_wire = match provider.wire {
            crate::providers::WireFormat::ChatCompletions => 0,
            crate::providers::WireFormat::Responses => 1,
            crate::providers::WireFormat::AnthropicMessages => 2,
        };
        self.provider_model = crate::providers::chat_model_for_provider(provider);
        self.provider_catalog = provider.catalog.clone();
        self.provider_picked = provider.enabled_models.iter().cloned().collect();
        self.provider_rows = provider.model_map.clone();
        self.provider_effort = provider.effort.clone().unwrap_or_default();
        self.provider_context = provider.context.clone().unwrap_or_default();
        self.set_notice(format!("正在修改 {}，改完再保存。", provider.label));
    }

    fn draw_overview(
        &mut self,
        ui: &mut egui::Ui,
        account: &crate::sand::PublicAccount,
        coexist: bool,
        catalog: &[crate::sand::SandFamily],
        enabled: &[String],
        providers: &[crate::providers::Provider],
    ) {
        egui::ScrollArea::vertical()
            .id_salt("overview-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("概览").size(22.0).color(theme::TEXT));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        for (span, label) in [
                            (10 * 60_000_u64, "近10分钟"),
                            (60 * 60_000, "近1小时"),
                            (86_400_000, "近1自然日"),
                            (7 * 86_400_000, "近一周"),
                            (30 * 86_400_000, "近一个月"),
                        ] {
                            if theme::text_chip(ui, label, self.overview_span_ms == span).clicked()
                            {
                                self.overview_span_ms = span;
                            }
                        }
                    });
                });
                ui.label(
                    RichText::new("柱状图始终按时间轴画出（无调用的日期是灰柱）。数字只来自本机测试对话和 Cursor 共存转发。")
                        .size(13.0)
                        .color(theme::MUTED),
                );
                ui.add_space(12.0);
                let until = crate::usage::now_ms();
                let since = until.saturating_sub(self.overview_span_ms);
                let summary = crate::usage::summarize(since);
                let cal = crate::usage::summarize_window(
                    until.saturating_sub(17 * 7 * 86_400_000),
                    until,
                    86_400_000,
                );
                let saved = providers
                    .iter()
                    .filter(|p| crate::providers::visible_provider(p))
                    .count();
                ui.horizontal(|ui| {
                    let full = ui.available_width();
                    let chart_w = (full * 0.58).max(320.0);
                    let cal_w = (full - chart_w - 16.0).max(220.0);
                    ui.vertical(|ui| {
                        ui.set_width(chart_w);
                        draw_trend_chart(ui, &summary, 200.0);
                    });
                    ui.add_space(16.0);
                    ui.vertical(|ui| {
                        ui.set_width(cal_w);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("贡献日历").size(14.0).strong());
                            ui.label(
                                RichText::new("17 周")
                                    .size(12.0)
                                    .color(theme::MUTED),
                            );
                        });
                        ui.add_space(6.0);
                        draw_calendar(ui, &cal, cal_w, 200.0);
                    });
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let rate = if summary.rows == 0 {
                        0.0
                    } else {
                        summary.ok as f32 / summary.rows as f32
                    };
                    egui::Frame::new()
                        .fill(theme::PANEL)
                        .corner_radius(10.0)
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            motion::gauge(
                                ui,
                                rate,
                                false,
                                "成功率",
                                &if summary.rows == 0 {
                                    "—".into()
                                } else {
                                    format!("{:.1}%", rate * 100.0)
                                },
                            );
                        });
                    metric_card(ui, "LLM 调用", &summary.rows.to_string(), summary.rows > 0);
                    metric_card(
                        ui,
                        "成功 / 异常",
                        &format!("{} / {}", summary.ok, summary.err),
                        summary.rows > 0 && summary.err == 0,
                    );
                    metric_card(
                        ui,
                        "Token",
                        &if summary.tokens == 0 {
                            "—".into()
                        } else {
                            summary.tokens.to_string()
                        },
                        summary.tokens > 0,
                    );
                    let avg = if summary.rows == 0 {
                        0
                    } else {
                        summary.latency_ms / summary.rows as u64
                    };
                    metric_card(
                        ui,
                        "平均延迟",
                        &if avg == 0 {
                            "—".into()
                        } else {
                            format!("{avg} ms")
                        },
                        avg > 0,
                    );
                });
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    metric_card(
                        ui,
                        "登录",
                        if account.exhausted {
                            "额度用尽"
                        } else if account.signed_in {
                            "已登录"
                        } else {
                            "未登录"
                        },
                        account.signed_in && !account.exhausted,
                    );
                    metric_card(ui, "共存", if coexist { "已接 Cursor" } else { "未接" }, coexist);
                    metric_card(
                        ui,
                        "已启动",
                        &format!(
                            "{}/{}",
                            enabled.len(),
                            catalog.iter().filter(|f| f.id != "default").count()
                        ),
                        !enabled.is_empty(),
                    );
                    metric_card(ui, "供应商", &saved.to_string(), saved > 0);
                });
                ui.add_space(12.0);
                if !summary.by_model.is_empty() {
                    ui.label(RichText::new("按模型").size(14.0).strong());
                    for (model, count) in &summary.by_model {
                        ui.label(
                            RichText::new(format!("{model}  ·  {count} 次"))
                                .size(12.0)
                                .color(theme::TEXT),
                        );
                    }
                    ui.add_space(8.0);
                }
                if !summary.recent.is_empty() {
                    ui.label(RichText::new("最近请求").size(14.0).strong());
                    for row in &summary.recent {
                        ui.label(
                            RichText::new(format!(
                                "{}  {}  {}  {} ms  {}",
                                crate::usage::stamp_label(row.ts_ms),
                                row.source,
                                row.model,
                                row.latency_ms,
                                if row.ok { "200" } else { "ERR" }
                            ))
                            .size(12.0)
                            .color(theme::MUTED),
                        );
                    }
                } else {
                    ui.label(
                        RichText::new("还没有请求行。趋势图的灰柱是时间轴占位，发测试对话或 Cursor 共存成功后会变绿。")
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                }
            });
    }

    fn draw_ext(&mut self, ui: &mut egui::Ui, pool: &[String]) {
        ui.horizontal(|ui| {
            for (tab, glyph, label) in [
                (ExtTab::Pool, Glyph::Pool, "提示池"),
                (ExtTab::Skills, Glyph::Skill, "Skills"),
                (ExtTab::Mcp, Glyph::Mcp, "MCP"),
            ] {
                if theme::brand_chip(ui, glyph, label, self.ext_tab == tab).clicked() {
                    self.ext_tab = tab;
                    if tab == ExtTab::Skills {
                        self.skills = crate::skills::scan();
                    }
                }
            }
        });
        ui.add_space(12.0);
        egui::ScrollArea::vertical()
            .id_salt("ext-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| match self.ext_tab {
                ExtTab::Pool => self.draw_pool(ui, pool),
                ExtTab::Skills => self.draw_skills(ui),
                ExtTab::Mcp => self.draw_mcp(ui),
            });
    }

    fn draw_skills(&mut self, ui: &mut egui::Ui) {
        theme::page_head(
            ui,
            "Skills",
            "扫描本机 SKILL.md。开关会复制或移除 ~/.cursor/skills 下的同名目录。",
        );
        if ui.button("刷新扫描").clicked() {
            self.skills = crate::skills::scan();
        }
        ui.add_space(8.0);
        if self.skills.is_empty() {
            theme::empty_hint(
                ui,
                Glyph::Skill,
                "没扫到 skill",
                "把带 SKILL.md 的目录放到 ~/.cursor/skills 或 ~/.grok/skills。",
            );
            return;
        }
        for index in 0..self.skills.len() {
            let mut on = self.skills[index].enabled;
            let name = self.skills[index].name.clone();
            let desc = self.skills[index].description.clone();
            egui::Frame::new()
                .fill(theme::PANEL)
                .corner_radius(10.0)
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut on, "").changed() {
                            match crate::skills::set_enabled(&self.skills[index], on) {
                                Ok(()) => {
                                    self.skills[index].enabled = on;
                                    self.set_notice(if on {
                                        format!("已启用 {name}")
                                    } else {
                                        format!("已停用 {name}")
                                    });
                                }
                                Err(error) => self.set_notice(format!("skill 失败：{error}")),
                            }
                        }
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&name).size(14.0).strong());
                            if !desc.is_empty() {
                                ui.label(RichText::new(&desc).size(12.0).color(theme::MUTED));
                            }
                        });
                    });
                });
            ui.add_space(6.0);
        }
    }

    fn draw_mcp(&mut self, ui: &mut egui::Ui) {
        theme::page_head(
            ui,
            "MCP",
            "预设对齐 CC Switch。应用到 Cursor 时只改带标记的键，不会整文件覆盖 mcp.json。",
        );
        if motion::action_button(ui, Glyph::Mcp, "应用到 Cursor", true, false).clicked() {
            self.mcp_apply_confirm = true;
        }
        if self.mcp_apply_confirm {
            ui.add_space(6.0);
            ui.label(
                RichText::new("确认写入 ~/.cursor/mcp.json？只改本工具标记过的键。")
                    .size(12.0)
                    .color(theme::MUTED),
            );
            ui.horizontal(|ui| {
                if ui.button("确认写入").clicked() {
                    match crate::mcp::merge_into_cursor(&self.mcp_servers) {
                        Ok(()) => {
                            self.persist_mcp();
                            self.set_notice("已合并进 ~/.cursor/mcp.json，重启 Cursor 生效。");
                        }
                        Err(error) => self.set_notice(format!("写入失败：{error}")),
                    }
                    self.mcp_apply_confirm = false;
                }
                if ui.button("取消").clicked() {
                    self.mcp_apply_confirm = false;
                }
            });
        }
        ui.add_space(8.0);
        for server in &mut self.mcp_servers {
            ui.horizontal(|ui| {
                ui.checkbox(&mut server.enabled, "");
                ui.label(RichText::new(&server.name).size(13.0).strong());
                ui.label(
                    RichText::new(format!("{:?} · {}", server.transport, server.command))
                        .size(11.0)
                        .color(theme::MUTED),
                );
            });
        }
        ui.add_space(10.0);
        ui.label(RichText::new("自定义 stdio").size(12.0).color(theme::MUTED));
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.mcp_custom_id)
                    .hint_text("id")
                    .desired_width(120.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.mcp_custom_cmd)
                    .hint_text("command")
                    .desired_width(160.0),
            );
            if ui.button("添加").clicked() && !self.mcp_custom_id.trim().is_empty() {
                if !crate::mcp::command_allowed(&self.mcp_custom_cmd) {
                    self.set_notice(format!(
                        "command 不在白名单：{}（允许 npx/node/python/uvx/bun/deno）",
                        self.mcp_custom_cmd
                    ));
                } else {
                    self.mcp_servers.push(crate::mcp::McpServer {
                        id: self.mcp_custom_id.trim().to_owned(),
                        name: self.mcp_custom_id.trim().to_owned(),
                        transport: crate::mcp::McpTransport::Stdio,
                        command: self.mcp_custom_cmd.clone(),
                        args: Vec::new(),
                        url: String::new(),
                        enabled: true,
                    });
                    self.mcp_custom_id.clear();
                }
            }
        });
    }

    fn persist_mcp(&self) {
        let servers = self.mcp_servers.clone();
        self.rt.spawn(async move {
            let mut prefs = store::load_prefs().await.unwrap_or_default();
            prefs.mcp = servers;
            let _ = store::save_prefs(&prefs).await;
        });
    }

    fn draw_settings(&mut self, ui: &mut egui::Ui, coexist: bool, busy: &Busy) {
        ui.horizontal(|ui| {
            for (tab, label) in [
                (SettingsTab::Network, "网络服务"),
                (SettingsTab::Data, "数据管理"),
                (SettingsTab::Version, "版本"),
            ] {
                if theme::brand_chip(ui, Glyph::Settings, label, self.settings_tab == tab).clicked()
                {
                    self.settings_tab = tab;
                }
            }
        });
        ui.add_space(12.0);
        match self.settings_tab {
            SettingsTab::Network => self.draw_network(ui, coexist, busy),
            SettingsTab::Data => self.draw_data(ui),
            SettingsTab::Version => self.draw_version(ui, busy),
        }
    }

    fn draw_network(&mut self, ui: &mut egui::Ui, coexist: bool, busy: &Busy) {
        theme::page_head(
            ui,
            "网络服务",
            "关窗口进托盘。官方 Cursor 直连 api2，不经过本软件。不启动本软件时官方聊天不受影响。只有本软件在跑时，目录和 gb-* 才进 47821。托盘「退出」才停。",
        );
        ui.label(RichText::new("共存入口  127.0.0.1:47821（hook，不是 MITM）").size(14.0));
        ui.label(
            RichText::new("47822 MITM 已停用，不要写 Cursor http.proxy。")
                .size(12.0)
                .color(theme::MUTED),
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if coexist {
                if motion::action_button(ui, Glyph::Plug, "停用共存", false, busy.cursor).clicked()
                {
                    self.disable_cursor();
                }
            } else if motion::action_button(ui, Glyph::Plug, "启用共存", true, busy.cursor)
                .clicked()
            {
                self.enable_cursor();
            }
            if ui.button("复制 API").clicked() {
                ui.ctx().copy_text("http://127.0.0.1:47821/v1".into());
                self.set_notice("已复制 API 地址");
            }
            if ui.button("健康检查").clicked() {
                self.health_check();
            }
        });
        theme::pill(
            ui,
            if coexist {
                "Cursor 共存已开"
            } else {
                "Cursor 共存未开"
            },
            coexist,
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new("共存与官方隔离：官方账号/官方模型始终直连 api2。本软件没开时钩子探测 47821 失败就全部走官方，Cursor 不空等。只有 gb-* 在本软件运行时才进 47821。启用后完全退出 Cursor 再开。托盘「退出」会撤掉 47822 并恢复钩子。")
                .size(12.0)
                .color(theme::MUTED),
        );
        ui.add_space(16.0);
        ui.label(
            RichText::new("出站代理（可选，Clash 等）")
                .size(14.0)
                .strong(),
        );
        ui.label(
            RichText::new("只给 Grok-Bot-Auth 自己访问 Grok Stream / xAI / 官方目录。能直连就留空，不要填 47822。")
                .size(12.0)
                .color(theme::MUTED),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.outbound_proxy)
                    .hint_text("http://127.0.0.1:7890")
                    .desired_width(320.0),
            );
            if ui.button("应用").clicked() {
                self.apply_outbound_proxy();
            }
        });
    }

    fn apply_outbound_proxy(&self) {
        let proxy = self.outbound_proxy.clone();
        let state = self.state.clone();
        let notice = self.notice.clone();
        self.rt.spawn(async move {
            let trimmed = proxy.trim().to_owned();
            let result = async {
                state
                    .client
                    .set_outbound((!trimmed.is_empty()).then_some(trimmed.as_str()))?;
                let mut prefs = crate::store::load_prefs().await.unwrap_or_default();
                prefs.outbound_proxy = trimmed.clone();
                crate::store::save_prefs(&prefs).await
            }
            .await;
            *notice.lock().unwrap() = match result {
                Ok(()) if trimmed.is_empty() => "出站代理已关闭，直连上游。".into(),
                Ok(()) => format!("出站代理已设为 {trimmed}"),
                Err(error) => format!("出站代理失败：{error}"),
            };
        });
    }

    fn health_check(&self) {
        let notice = self.notice.clone();
        self.rt.spawn(async move {
            let started = std::time::Instant::now();
            let text = match reqwest::Client::new()
                .get("http://127.0.0.1:47821/health")
                .timeout(std::time::Duration::from_secs(3))
                .send()
                .await
            {
                Ok(resp) => format!(
                    "健康检查 HTTP {} · {} ms",
                    resp.status().as_u16(),
                    started.elapsed().as_millis()
                ),
                Err(error) => format!("健康检查失败：{error}"),
            };
            *notice.lock().unwrap() = text;
        });
    }

    fn draw_data(&mut self, ui: &mut egui::Ui) {
        theme::page_head(
            ui,
            "数据管理",
            "导出备份到 ~/.grok-bot-auth/backups。导入只覆盖 prefs，不自动退出 Grok Bot 会话。",
        );
        if ui.button("导出备份").clicked() {
            match crate::data_io::export_prefs() {
                Ok(path) => self.set_notice(format!("已导出 {}", path.display())),
                Err(error) => self.set_notice(format!("导出失败：{error}")),
            }
        }
        if ui.button("清除第三方供应商文件").clicked() {
            match crate::data_io::clear_providers_file() {
                Ok(()) => self.set_notice("providers.json 已清空，重启后刷新列表。"),
                Err(error) => self.set_notice(format!("清除失败：{error}")),
            }
        }
        ui.label(
            RichText::new(format!("数据目录 {}", crate::store::data_dir().display()))
                .size(12.0)
                .color(theme::MUTED),
        );
    }

    fn draw_version(&mut self, ui: &mut egui::Ui, busy: &Busy) {
        theme::page_head(
            ui,
            "版本",
            "Grok-Bot-Auth 内置检查。没有自己的发布源时会如实说明，不会覆盖 exe。",
        );
        ui.label(
            RichText::new(format!("Grok-Bot-Auth  {}", crate::update::app_version()))
                .size(16.0)
                .strong(),
        );
        ui.label(
            RichText::new(match crate::update::detect_cursor_version() {
                Some(v) => format!("Cursor {v}"),
                None => "未探测到 Cursor.exe".into(),
            })
            .size(13.0)
            .color(theme::MUTED),
        );
        ui.add_space(8.0);
        if motion::action_button(ui, Glyph::Settings, "检查更新", true, busy.provider).clicked()
        {
            self.check_update();
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(&self.version_notice)
                .size(13.0)
                .color(theme::TEXT),
        );
        let live = self.notice();
        if !live.is_empty() {
            ui.label(RichText::new(live).size(13.0).color(theme::MUTED));
        }
        ui.add_space(12.0);
        ui.label(RichText::new("更新日志").size(14.0).strong());
        egui::ScrollArea::vertical()
            .max_height(280.0)
            .show(ui, |ui| {
                ui.label(
                    RichText::new(crate::update::changelog())
                        .size(13.0)
                        .color(theme::TEXT),
                );
            });
    }

    fn check_update(&self) {
        self.patch_busy(|b| b.provider = true);
        let notice = self.notice.clone();
        let busy = self.busy.clone();
        self.rt.spawn(async move {
            let http = reqwest::Client::new();
            let report = crate::update::check_remote(&http)
                .await
                .unwrap_or_else(|error| crate::update::VersionReport {
                    app: crate::update::app_version().into(),
                    notice: error.to_string(),
                    ..Default::default()
                });
            *notice.lock().unwrap() = report.notice.clone();
            if let Ok(mut b) = busy.lock() {
                b.provider = false;
            }
        });
    }

    fn save_provider(&self) {
        if self.provider_kind == KIND_GROK_BOT {
            self.set_notice("Grok Bot 用上面的 OAuth 登录，不是第三方供应商。");
            return;
        }
        self.patch_busy(|b| b.provider = true);
        let kind = ui_kind(self.provider_kind);
        let wire = match self.provider_wire {
            1 => crate::providers::WireFormat::Responses,
            2 => crate::providers::WireFormat::AnthropicMessages,
            _ => crate::providers::WireFormat::ChatCompletions,
        };
        let auth_mode = if crate::providers::supports_oauth(kind) && self.provider_auth == 1 {
            crate::providers::AuthMode::Oauth
        } else {
            crate::providers::AuthMode::ApiKey
        };
        let id = if self.editing_id.is_empty() {
            format!("{}-{}", kind.as_str(), self.provider_label)
        } else {
            self.editing_id.clone()
        };
        let existing = self
            .snap
            .as_ref()
            .and_then(|snap| snap.providers.iter().find(|item| item.id == id).cloned());
        let mut provider = crate::providers::Provider {
            id,
            kind,
            label: self.provider_label.clone(),
            base_url: self.provider_base.clone(),
            api_key: self.provider_key.clone(),
            auth_mode,
            oauth_token: if auth_mode == crate::providers::AuthMode::Oauth {
                self.provider_key.clone()
            } else {
                String::new()
            },
            oauth_refresh: existing
                .as_ref()
                .map(|item| item.oauth_refresh.clone())
                .unwrap_or_default(),
            wire,
            model_map: self.provider_rows.clone(),
            catalog: self.provider_catalog.clone(),
            enabled_models: {
                let mut ids: Vec<String> = self.provider_picked.iter().cloned().collect();
                ids.sort();
                ids
            },
            selected_model: self.provider_model.clone(),
            effort: crate::providers::optional_knob(&self.provider_effort),
            context: crate::providers::optional_knob(&self.provider_context),
            oauth_accounts: existing
                .as_ref()
                .map(|item| item.oauth_accounts.clone())
                .unwrap_or_default(),
            pool_mode: existing
                .as_ref()
                .map(|item| item.pool_mode)
                .unwrap_or_default(),
            active_email: existing
                .as_ref()
                .map(|item| item.active_email.clone())
                .unwrap_or_default(),
        };
        if auth_mode == crate::providers::AuthMode::Oauth && !self.provider_key.trim().is_empty() {
            crate::providers::upsert_provider_account(
                &mut provider,
                crate::providers::provider_account_from_token(
                    &self.provider_key,
                    &self.oauth_email,
                ),
            );
        }
        let state = self.state.clone();
        let notice = self.notice.clone();
        let busy = self.busy.clone();
        self.rt.spawn(async move {
            let snapshot = {
                let mut list = state.providers.lock().await;
                crate::providers::upsert_provider(&mut list, provider);
                list.clone()
            };
            let _ = crate::store::save_providers(&snapshot).await;
            let _ = state.persist_prefs().await;
            if let Ok(mut b) = busy.lock() {
                b.provider = false;
            }
            *notice.lock().unwrap() = "供应商已保存（未改动 Grok Bot 会话）".into();
        });
    }

    fn delete_provider(&mut self, id: &str) {
        if self.editing_id == id {
            self.editing_id.clear();
        }
        if self.source == id {
            self.source = "grok".into();
        }
        let id = id.to_owned();
        let state = self.state.clone();
        let notice = self.notice.clone();
        self.rt.spawn(async move {
            let mut list = state.providers.lock().await;
            list.retain(|item| item.id != id);
            let snapshot = list.clone();
            drop(list);
            let _ = crate::store::save_providers(&snapshot).await;
            let _ = state.persist_prefs().await;
            *notice.lock().unwrap() = "已删除该供应商。".into();
        });
    }

    fn ping_provider(&self) {
        let kind = ui_kind(self.provider_kind);
        let provider = crate::providers::Provider {
            kind,
            base_url: self.provider_base.clone(),
            api_key: self.provider_key.clone(),
            auth_mode: if self.provider_auth == 1 {
                crate::providers::AuthMode::Oauth
            } else {
                crate::providers::AuthMode::ApiKey
            },
            oauth_token: self.provider_key.clone(),
            ..crate::providers::Provider::default()
        };
        self.patch_busy(|b| b.provider = true);
        let state = self.state.clone();
        let ping_fill = self.ping_fill.clone();
        let busy = self.busy.clone();
        self.rt.spawn(async move {
            let result = state.client.ping_provider(&provider).await;
            if let Ok(mut b) = busy.lock() {
                b.provider = false;
            }
            let text = match result {
                Ok((ms, status)) => format!("测速 {ms} ms · HTTP {status}"),
                Err(error) => format!("测速失败：{error}"),
            };
            *ping_fill.lock().unwrap() = Some(text);
        });
    }

    fn start_provider_oauth(&mut self) {
        let kind = ui_kind(self.provider_kind);
        if !crate::providers::supports_oauth(kind) {
            self.set_notice("通用供应商没有官方 OAuth，请用 API Key");
            return;
        }
        match crate::providers::oauth_portal_url(kind) {
            Err(message) => {
                self.set_notice(message);
                return;
            }
            Ok(portal) => {
                if kind != crate::providers::ProviderKind::Xai {
                    let state = self.state.clone();
                    let url = portal.to_owned();
                    self.rt.spawn(async move {
                        *state.pending_oauth.lock().await = Some(crate::accounts::PendingOauth {
                            source: crate::accounts::PendingKind::Portal,
                            url: url.clone(),
                            user_code: None,
                            hint: "浏览器登录后把 token/key 贴进输入框，再点保存。可取消。".into(),
                        });
                    });
                    if let Err(error) = crate::open::open_url(portal) {
                        self.set_notice(format!("无法打开 OAuth：{error}"));
                    } else {
                        self.set_notice("已打开官方登录页。完成后把 token 贴进输入框再保存。");
                    }
                    return;
                }
            }
        }
        self.patch_busy(|b| b.provider = true);
        let state = self.state.clone();
        let notice = self.notice.clone();
        let busy = self.busy.clone();
        let fill = self.oauth_fill.clone();
        self.rt.spawn(async move {
            *state.oauth_cancel.lock().await = false;
            let result = async {
                let (device, user, url) = state.client.xai_oauth_begin().await?;
                *state.pending_oauth.lock().await = Some(crate::accounts::PendingOauth {
                    source: crate::accounts::PendingKind::Xai {
                        device_code: device.clone(),
                    },
                    url: url.clone(),
                    user_code: Some(user.clone()),
                    hint: "在浏览器完成 xAI 设备授权。可复制网址、取消，或点「已完成，核实」。".into(),
                });
                if let Err(error) = crate::open::open_url(&url) {
                    *notice.lock().unwrap() = format!("浏览器没打开，请复制网址：{error}");
                } else {
                    *notice.lock().unwrap() = format!("xAI 设备码 {user}，等待授权…");
                }
                let mut waits = 0u32;
                loop {
                    waits += 1;
                    if waits > 80 || *state.oauth_cancel.lock().await {
                        break;
                    }
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {}
                        _ = state.oauth_kick.notified() => {}
                    }
                    match state.client.xai_oauth_poll(&device).await {
                        Ok(Some((token, refresh))) => {
                            let mut providers = state.providers.lock().await;
                            crate::providers::apply_xai_oauth_tokens(
                                &mut providers,
                                token.clone(),
                                refresh.clone(),
                            );
                            let _ = crate::store::save_providers(&providers).await;
                            drop(providers);
                            let _ = state.persist_prefs().await;
                            return Ok((token, refresh));
                        }
                        Ok(None) => continue,
                        Err(error) => return Err(error),
                    }
                }
                if *state.oauth_cancel.lock().await {
                    Err(crate::error::Error::Msg("已取消".into()))
                } else {
                    Err(crate::error::Error::Msg("xAI OAuth 超时".into()))
                }
            }
            .await;
            *state.pending_oauth.lock().await = None;
            if let Ok(mut b) = busy.lock() {
                b.provider = false;
            }
            match result {
                Ok((token, refresh)) => {
                    *fill.lock().unwrap() = Some(token);
                    *notice.lock().unwrap() = if refresh.as_ref().is_some_and(|s| !s.is_empty()) {
                        "xAI 授权完成，refresh 已保存，过期会自动续，不用再点保存。".into()
                    } else {
                        "xAI 授权完成，但这次没有下发 refresh。自动续期不可用，请再授权一次并同意全部权限。".into()
                    };
                }
                Err(error) => {
                    *notice.lock().unwrap() = format!("OAuth 失败：{error}");
                }
            }
        });
    }

    fn import_provider_models(&self) {
        let kind = ui_kind(self.provider_kind);
        let provider = crate::providers::Provider {
            kind,
            base_url: self.provider_base.clone(),
            api_key: self.provider_key.clone(),
            auth_mode: if self.provider_auth == 1 {
                crate::providers::AuthMode::Oauth
            } else {
                crate::providers::AuthMode::ApiKey
            },
            oauth_token: self.provider_key.clone(),
            ..crate::providers::Provider::default()
        };
        self.patch_busy(|b| b.provider = true);
        let state = self.state.clone();
        let notice = self.notice.clone();
        let busy = self.busy.clone();
        let fill = self.catalog_fill.clone();
        self.rt.spawn(async move {
            let result = state.client.list_provider_models(&provider).await;
            if let Ok(mut b) = busy.lock() {
                b.provider = false;
            }
            match result {
                Ok(ids) => {
                    let n = ids.len();
                    *fill.lock().unwrap() = Some(ids);
                    *notice.lock().unwrap() = format!("已导入 {n} 个模型，打开下拉选择。");
                }
                Err(error) => {
                    *notice.lock().unwrap() = format!("导入清单失败：{error}");
                }
            }
        });
    }

    fn add_pool_entry(&mut self) {
        let entry = self.pool_draft.trim().to_owned();
        if entry.is_empty() {
            return;
        }
        self.pool_draft.clear();
        let state = self.state.clone();
        self.rt.spawn(async move {
            state.prompt_pool.lock().await.push(entry);
            let _ = state.persist_prefs().await;
        });
    }

    fn remove_pool_entry(&self, index: usize) {
        let state = self.state.clone();
        self.rt.spawn(async move {
            {
                let mut pool = state.prompt_pool.lock().await;
                if index < pool.len() {
                    pool.remove(index);
                }
            }
            let _ = state.persist_prefs().await;
        });
    }

    fn import_session(&self) {
        let raw = self.import_json.clone();
        let state = self.state.clone();
        let notice = self.notice.clone();
        self.rt.spawn(async move {
            let result = async {
                let body: ImportBody = serde_json::from_str(&raw)?;
                let mut account = import_account(body)?;
                if let Ok(Some(access)) = state.client.sand_access(&account).await {
                    account.sand_state = Some(access);
                }
                crate::sand::fill_account_identity(&mut account);
                state.commit_grok_account(account).await?;
                Ok::<_, crate::error::Error>(())
            }
            .await;
            match result {
                Ok(()) => {
                    *notice.lock().unwrap() = "会话已导入，正在同步模型…".into();
                    let fetch_state = state.clone();
                    let fetch_notice = notice.clone();
                    tokio::spawn(async move {
                        let synced = async {
                            let account =
                                fetch_state.account.lock().await.clone().ok_or_else(|| {
                                    crate::error::Error::Msg("sign in first".into())
                                })?;
                            let body = fetch_state.client.available_models(&account).await?;
                            let families = parse_sand_families(&body);
                            let n = families.len();
                            *fetch_state.models.lock().await = families;
                            let _ = fetch_state.persist_prefs().await;
                            Ok::<_, crate::error::Error>(n)
                        }
                        .await;
                        *fetch_notice.lock().unwrap() = match synced {
                            Ok(n) => format!("会话已导入，已同步 {n} 个官方模型。在本页勾选添加。"),
                            Err(error) => format!("会话已导入，同步失败：{error}"),
                        };
                    });
                }
                Err(error) => {
                    *notice.lock().unwrap() = error.to_string();
                }
            }
        });
    }

    fn start_login(&self) {
        self.patch_busy(|b| b.login = true);
        let state = self.state.clone();
        let notice = self.notice.clone();
        let busy = self.busy.clone();
        let email_hint = self.oauth_email.clone();
        self.rt.spawn(async move {
            *state.oauth_cancel.lock().await = false;
            let (start, session) = begin_login();
            *state.pending_oauth.lock().await = Some(crate::accounts::PendingOauth {
                source: crate::accounts::PendingKind::Grok {
                    login_id: start.id.clone(),
                },
                url: start.login_url.clone(),
                user_code: None,
                hint: "必须用本软件打开的登录页（带 challenge/uuid）。浏览器里 All set / Open Grok Bot 是官方 Grok Bot 应用，令牌不会回到这里。".into(),
            });
            if let Err(error) = crate::open::open_url(&start.login_url) {
                *notice.lock().unwrap() = format!("浏览器没打开，请复制网址：{error}");
            } else {
                *notice.lock().unwrap() = "已打开登录页，等待授权…".into();
            }
            state.logins.lock().await.insert(start.id.clone(), session);
            let mut waits = 0u32;
            loop {
                waits += 1;
                if waits > 200 {
                    break;
                }
                if *state.oauth_cancel.lock().await {
                    break;
                }
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_millis(1200)) => {}
                    _ = state.oauth_kick.notified() => {}
                }
                if *state.oauth_cancel.lock().await {
                    break;
                }
                let Some(sess) = state.logins.lock().await.get(&start.id).cloned() else {
                    break;
                };
                match state.client.poll_login(&sess.uuid, &sess.verifier).await {
                    Ok(None) => {
                        *notice.lock().unwrap() = "还没完成。浏览器确认后再点「已完成，核实」。".into();
                    }
                    Ok(Some((access, refresh))) => {
                        let mut account = crate::sand::Account {
                            access_token: access,
                            refresh_token: Some(refresh),
                            machine_id: sess.machine_id,
                            display_name: "Grok Bot".into(),
                            email: email_hint.clone(),
                            ..crate::sand::Account::default()
                        };
                        if let Ok(Some(access_state)) = state.client.sand_access(&account).await {
                            account.sand_state = Some(access_state);
                        }
                        crate::sand::fill_account_identity(&mut account);
                        let email = crate::sand::account_id(&account);
                        if let Err(error) = state.commit_grok_account(account).await {
                            *notice.lock().unwrap() = error.to_string();
                        } else {
                            *notice.lock().unwrap() =
                                format!("{email} 已加入账号池，正在同步模型…");
                        }
                        state.logins.lock().await.remove(&start.id);
                        *state.pending_oauth.lock().await = None;
                        if let Ok(mut b) = busy.lock() {
                            b.login = false;
                        }
                        let fetch_state = state.clone();
                        let fetch_notice = notice.clone();
                        let fetch_busy = busy.clone();
                        tokio::spawn(async move {
                            if let Ok(mut b) = fetch_busy.lock() {
                                b.models = true;
                            }
                            let result = async {
                                let account = fetch_state
                                    .account
                                    .lock()
                                    .await
                                    .clone()
                                    .ok_or_else(|| crate::error::Error::Msg("sign in first".into()))?;
                                let body = fetch_state.client.available_models(&account).await?;
                                let families = parse_sand_families(&body);
                                let n = families.len();
                                *fetch_state.models.lock().await = families;
                                let _ = fetch_state.persist_prefs().await;
                                let _ = fetch_state.refresh_grok_quotas().await;
                                Ok::<_, crate::error::Error>(n)
                            }
                            .await;
                            if let Ok(mut b) = fetch_busy.lock() {
                                b.models = false;
                            }
                            *fetch_notice.lock().unwrap() = match result {
                                Ok(n) => format!("已同步 {n} 个官方模型。去「账号」看额度和重置时间。"),
                                Err(error) => format!("登录完成，同步失败：{error}"),
                            };
                        });
                        return;
                    }
                    Err(error) => {
                        *notice.lock().unwrap() = error.to_string();
                        *state.pending_oauth.lock().await = None;
                        if let Ok(mut b) = busy.lock() {
                            b.login = false;
                        }
                        return;
                    }
                }
            }
            state.logins.lock().await.remove(&start.id);
            *state.pending_oauth.lock().await = None;
            if let Ok(mut b) = busy.lock() {
                b.login = false;
            }
            if *state.oauth_cancel.lock().await {
                *notice.lock().unwrap() = "已取消登录".into();
            } else {
                *notice.lock().unwrap() = "登录超时".into();
            }
        });
    }

    fn cancel_oauth(&self) {
        let state = self.state.clone();
        let busy = self.busy.clone();
        let notice = self.notice.clone();
        self.rt.spawn(async move {
            *state.oauth_cancel.lock().await = true;
            state.oauth_kick.notify_waiters();
            *state.pending_oauth.lock().await = None;
            if let Ok(mut b) = busy.lock() {
                b.login = false;
                b.provider = false;
            }
            *notice.lock().unwrap() = "已取消 OAuth".into();
        });
    }

    fn verify_oauth(&self) {
        let state = self.state.clone();
        self.set_notice("正在核实登录…");
        self.rt.spawn(async move {
            state.oauth_kick.notify_waiters();
        });
    }

    fn set_provider_pool_mode(&self, provider_id: &str, mode: crate::accounts::PoolMode) {
        let provider_id = provider_id.to_owned();
        let state = self.state.clone();
        let notice = self.notice.clone();
        self.rt.spawn(async move {
            match state.set_provider_pool_mode(&provider_id, mode).await {
                Ok(()) => {
                    *notice.lock().unwrap() = format!("{}：{}", provider_id, mode.label());
                }
                Err(error) => *notice.lock().unwrap() = error.to_string(),
            }
        });
    }

    fn set_pool_mode(&self, mode: crate::accounts::PoolMode) {
        let state = self.state.clone();
        let notice = self.notice.clone();
        self.rt.spawn(async move {
            match state.set_grok_mode(mode).await {
                Ok(()) => *notice.lock().unwrap() = format!("调配方式：{}", mode.label()),
                Err(error) => *notice.lock().unwrap() = error.to_string(),
            }
        });
    }

    fn activate_account(&self, email: &str) {
        let email = email.to_owned();
        let state = self.state.clone();
        let notice = self.notice.clone();
        self.rt.spawn(async move {
            match state.activate_grok_email(&email).await {
                Ok(()) => *notice.lock().unwrap() = format!("已切到 {email}"),
                Err(error) => *notice.lock().unwrap() = error.to_string(),
            }
        });
    }

    fn remove_account(&self, email: &str) {
        let email = email.to_owned();
        let state = self.state.clone();
        let notice = self.notice.clone();
        self.rt.spawn(async move {
            match state.remove_grok_email(&email).await {
                Ok(()) => *notice.lock().unwrap() = format!("已移除 {email}"),
                Err(error) => *notice.lock().unwrap() = error.to_string(),
            }
        });
    }

    fn refresh_quotas_silent(&self) {
        let state = self.state.clone();
        self.rt.spawn(async move {
            let _ = state.refresh_all_quotas().await;
        });
    }

    fn refresh_session(&self) {
        let state = self.state.clone();
        let notice = self.notice.clone();
        self.rt.spawn(async move {
            let result = async {
                let current = state
                    .account
                    .lock()
                    .await
                    .clone()
                    .ok_or_else(|| crate::error::Error::Msg("sign in first".into()))?;
                let mut next = state.client.refresh(&current).await?;
                crate::sand::fill_account_identity(&mut next);
                state.commit_grok_account(next).await?;
                Ok::<_, crate::error::Error>(())
            }
            .await;
            *notice.lock().unwrap() = match result {
                Ok(()) => "已刷新".into(),
                Err(error) => error.to_string(),
            };
        });
    }

    fn fetch_models(&self) {
        self.patch_busy(|b| b.models = true);
        let state = self.state.clone();
        let notice = self.notice.clone();
        let busy = self.busy.clone();
        self.rt.spawn(async move {
            let result = async {
                let account = state
                    .account
                    .lock()
                    .await
                    .clone()
                    .ok_or_else(|| crate::error::Error::Msg("sign in first".into()))?;
                let body = state.client.available_models(&account).await?;
                let families = parse_sand_families(&body);
                let n = families.len();
                *state.models.lock().await = families;
                let _ = state.persist_prefs().await;
                Ok::<_, crate::error::Error>(n)
            }
            .await;
            if let Ok(mut b) = busy.lock() {
                b.models = false;
            }
            *notice.lock().unwrap() = match result {
                Ok(n) => {
                    format!("已获取 {n} 个模型。右侧开关启动后才会进 Cursor，不会整目录灌入。")
                }
                Err(error) => error.to_string(),
            };
        });
    }

    fn add_imported(&mut self) {
        let checked: Vec<String> = self.checked.iter().cloned().collect();
        if checked.is_empty() {
            self.set_notice("先勾选要添加的模型");
            return;
        }
        self.checked.clear();
        if let Some(snap) = self.snap.as_mut() {
            for id in &checked {
                if !snap.imported.iter().any(|item| item == id) {
                    snap.imported.push(id.clone());
                }
            }
        }
        let state = self.state.clone();
        let notice = self.notice.clone();
        self.rt.spawn(async move {
            let mut imported = state.imported.lock().await;
            for id in &checked {
                if !imported.iter().any(|existing| existing == id) {
                    imported.push(id.clone());
                }
            }
            let n = checked.len();
            drop(imported);
            let _ = state.persist_prefs().await;
            *notice.lock().unwrap() =
                format!("已添加 {n} 个模型。去「模型调度」右侧开关才会进 Cursor。");
        });
    }

    fn remove_imported(&mut self, id: &str) {
        if let Some(snap) = self.snap.as_mut() {
            snap.imported.retain(|item| item != id);
            snap.enabled.retain(|item| item != id);
        }
        let id = id.to_owned();
        let state = self.state.clone();
        let notice = self.notice.clone();
        self.rt.spawn(async move {
            state.imported.lock().await.retain(|item| item != &id);
            state.enabled.lock().await.retain(|item| item != &id);
            let _ = state.persist_prefs().await;
            *notice.lock().unwrap() = format!("{id} 已从 Grok Bot 列表移除");
        });
    }

    fn set_enabled(&mut self, id: &str, on: bool) {
        self.enabled_pending.insert(id.to_owned(), on);
        if let Some(snap) = self.snap.as_mut() {
            if on {
                if !snap.enabled.iter().any(|item| item == id) {
                    snap.enabled.push(id.to_owned());
                }
            } else {
                snap.enabled.retain(|item| item != id);
            }
        }
        let id = id.to_owned();
        let provider = crate::catalog::is_provider_enable_id(&id);
        let state = self.state.clone();
        let notice = self.notice.clone();
        self.rt.spawn(async move {
            if on && !provider {
                let mut imported = state.imported.lock().await;
                if !imported.iter().any(|existing| existing == &id) {
                    imported.push(id.clone());
                }
            }
            let mut enabled = state.enabled.lock().await;
            if on {
                if !enabled.iter().any(|existing| existing == &id) {
                    enabled.push(id.clone());
                }
                *state.preferred_model.lock().await = Some(id.clone());
            } else {
                enabled.retain(|existing| existing != &id);
            }
            drop(enabled);
            let _ = state.persist_prefs().await;
            *notice.lock().unwrap() = if on {
                if provider {
                    format!("{id} 已启用。完全退出 Cursor 后按供应商名区分，不会和 Grok Bot 同名开关连坐。")
                } else {
                    format!(
                        "{id} 已启用。完全退出 Cursor 后在 Add Models 里找「{} · Grok Bot」（id=gb-{id}）。选择器不加「可用」标签。",
                        id
                    )
                }
            } else {
                format!("{id} 已停用")
            };
        });
    }

    fn enable_cursor(&self) {
        self.patch_busy(|b| b.cursor = true);
        let state = self.state.clone();
        let notice = self.notice.clone();
        let busy = self.busy.clone();
        self.rt.spawn(async move {
            let result = start_coexist(&state).await;
            if let Ok(mut b) = busy.lock() {
                b.cursor = false;
            }
            *notice.lock().unwrap() = match result {
                Ok(url) => format!("{url} 必须完全退出 Cursor 再开。官方账号直连，不要走 47822。"),
                Err(error) => error.to_string(),
            };
        });
    }

    #[allow(dead_code)]
    fn select_model(&mut self, id: String) {
        self.test_model = id;
    }

    fn persist_knobs(&self) {
        let effort = self.effort.clone();
        let context = self.context.clone();
        let budget = crate::compact::official_budget();
        let state = self.state.clone();
        self.rt.spawn(async move {
            *state.effort.lock().await = effort;
            *state.context.lock().await = context;
            *state.compact_budget.lock().await = budget;
            let _ = state.persist_prefs().await;
        });
    }

    fn test_model_chat(&mut self) {
        let grok = self.source == "grok";
        let model = if grok {
            self.test_model.clone()
        } else {
            let stored = self.rt.block_on(async {
                self.state
                    .providers
                    .lock()
                    .await
                    .iter()
                    .find(|item| item.id == self.source)
                    .cloned()
            });
            let picked = self.provider_model.trim();
            if !picked.is_empty() {
                picked.to_owned()
            } else {
                stored
                    .as_ref()
                    .map(crate::providers::chat_model_for_provider)
                    .unwrap_or_default()
            }
        };
        let prompt = self.test_prompt.clone();
        let effort = self.effort.clone();
        let fast = self.test_fast;
        let source = self.source.clone();
        let history_store = self.history.clone();
        let history = history_store.lock().map(|h| h.clone()).unwrap_or_default();
        if model.is_empty() {
            self.set_notice("先选择模型");
            return;
        }
        self.persist_knobs();
        if let Ok(mut turns) = history_store.lock() {
            turns.push(crate::compact::ChatTurn::new("user", prompt.clone()));
        }
        let state = self.state.clone();
        let notice = self.notice.clone();
        let think = self.test_think.clone();
        let out = self.test_out.clone();
        *think.lock().unwrap() = String::new();
        *out.lock().unwrap() = String::new();
        self.set_notice(format!("正在测试 {model} …"));
        self.patch_busy(|b| b.chat = true);
        let busy = self.busy.clone();
        self.rt.spawn(async move {
            let result = async {
                let pool = state.prompt_pool.lock().await.clone();
                let budget = crate::compact::official_budget();
                let turns = crate::compact::prepare_outbound(&pool, &history, &prompt, budget);
                let mut last_error = None;
                for _ in 0..8 {
                    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
                    let send = async {
                        if grok {
                            let account = match state.pick_grok_account().await {
                                Some(account) => account,
                                None => {
                                    return Err((
                                        String::new(),
                                        crate::error::Error::Msg("没有可用 Grok Bot 账号".into()),
                                    ));
                                }
                            };
                            state
                                .client
                                .stream_chat_turns(
                                    &account,
                                    &model,
                                    &turns,
                                    Some(&effort),
                                    fast,
                                    None,
                                    tx,
                                )
                                .await
                                .map_err(|error| (crate::sand::account_id(&account), error))
                        } else {
                            let provider = match state
                                .providers
                                .lock()
                                .await
                                .iter()
                                .find(|item| item.id == source)
                                .cloned()
                            {
                                Some(provider) => provider,
                                None => {
                                    return Err((
                                        String::new(),
                                        crate::error::Error::Msg("provider not found".into()),
                                    ));
                                }
                            };
                            state
                                .client
                                .stream_provider(&provider, &model, &turns, tx)
                                .await
                                .map_err(|error| (provider.active_email.clone(), error))
                        }
                    };
                    let recv = async {
                        while let Some(delta) = rx.recv().await {
                            if let Some(error) = delta.error {
                                return Err((String::new(), crate::error::Error::Msg(error)));
                            }
                            if !delta.thinking.is_empty() {
                                think.lock().unwrap().push_str(&delta.thinking);
                            }
                            if !delta.text.is_empty() {
                                out.lock().unwrap().push_str(&delta.text);
                            }
                        }
                        Ok(())
                    };
                    match tokio::try_join!(send, recv) {
                        Ok(_) => {
                            last_error = None;
                            break;
                        }
                        Err((email, error)) => {
                            let text = error.to_string();
                            if grok && crate::accounts::is_quota_error(&text) && !email.is_empty() {
                                let _ = state.mark_grok_quota(&email, &text).await;
                                last_error = Some(error);
                                *think.lock().unwrap() = String::new();
                                *out.lock().unwrap() = String::new();
                                continue;
                            }
                            return Err(error);
                        }
                    }
                }
                if let Some(error) = last_error {
                    return Err(error);
                }
                Ok::<_, crate::error::Error>(())
            }
            .await;
            let preview = out.lock().map(|s| s.clone()).unwrap_or_default();
            *notice.lock().unwrap() = match result {
                Ok(()) if preview.contains("outdated") || preview.contains("Please upgrade") => {
                    format!(
                        "测试被服务端拒绝（不一定是版本）：{}",
                        preview.chars().take(80).collect::<String>()
                    )
                }
                Ok(()) if preview.trim().is_empty() || preview == "测试中…" => {
                    *out.lock().unwrap() = "（没有正文）".into();
                    format!("测试完成但没有正文：{model}")
                }
                Ok(()) => {
                    if let Ok(mut turns) = history_store.lock() {
                        turns.push(crate::compact::ChatTurn::new("assistant", preview.clone()));
                    }
                    let _ = crate::usage::append(&crate::usage::UsageRow {
                        ts_ms: crate::usage::now_ms(),
                        source: source.clone(),
                        model: model.clone(),
                        latency_ms: 0,
                        tokens: 0,
                        ok: true,
                    });
                    format!("测试完成：{model}")
                }
                Err(error) => {
                    *out.lock().unwrap() = format!("失败：{error}");
                    format!("测试失败：{error}")
                }
            };
            if let Ok(mut b) = busy.lock() {
                b.chat = false;
            }
        });
    }

    fn disable_cursor(&self) {
        self.patch_busy(|b| b.cursor = true);
        let state = self.state.clone();
        let notice = self.notice.clone();
        let busy = self.busy.clone();
        self.rt.spawn(async move {
            let result = stop_coexist(&state).await;
            if let Ok(mut b) = busy.lock() {
                b.cursor = false;
            }
            *notice.lock().unwrap() = match result {
                Ok(()) => "已停用共存并已保存。重启 Cursor 后只走官方账号。".into(),
                Err(error) => error.to_string(),
            };
        });
    }
}

fn pick_test_model(
    catalog: &[crate::sand::SandFamily],
    enabled: &[String],
    imported: &[String],
) -> Option<String> {
    for id in enabled.iter().chain(imported.iter()) {
        if id.contains('/') {
            continue;
        }
        if catalog.iter().any(|family| {
            family.id == *id
                && family.available
                && crate::sand::grok_bot_inference_allowed(&family.id)
        }) {
            return Some(id.clone());
        }
    }
    catalog
        .iter()
        .find(|family| family.available && crate::sand::grok_bot_inference_allowed(&family.id))
        .map(|family| family.id.clone())
}

fn draw_bubble(ui: &mut egui::Ui, mine: bool, text: &str, think: &str) {
    if text.is_empty() && think.is_empty() {
        return;
    }
    ui.add_space(6.0);
    ui.with_layout(
        if mine {
            Layout::right_to_left(Align::Min)
        } else {
            Layout::left_to_right(Align::Min)
        },
        |ui| {
            let fill = if mine {
                theme::USER_BUBBLE
            } else {
                theme::BOT_BUBBLE
            };
            let max = (ui.available_width() * 0.78).max(180.0);
            egui::Frame::new()
                .fill(fill)
                .corner_radius(12.0)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_max_width(max);
                    ui.horizontal(|ui| {
                        let (icon, _) =
                            ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::hover());
                        theme::paint_glyph(
                            ui,
                            icon,
                            if mine { Glyph::User } else { Glyph::Bot },
                            if mine { theme::ACCENT } else { theme::OK },
                        );
                        ui.label(
                            RichText::new(if mine { "你" } else { "Grok Bot" })
                                .size(11.0)
                                .color(theme::MUTED),
                        );
                    });
                    if !think.is_empty() {
                        ui.label(
                            RichText::new(think)
                                .italics()
                                .size(12.0)
                                .color(theme::MUTED),
                        );
                    }
                    if !text.is_empty() {
                        ui.add(
                            egui::Label::new(RichText::new(text).size(14.0).color(theme::TEXT))
                                .wrap(),
                        );
                    }
                });
        },
    );
}

#[allow(dead_code)]
fn effort_label(value: &str) -> String {
    crate::providers::effort_display(value)
}

#[cfg(test)]
mod tests {
    #[test]
    fn effort_label_is_english() {
        assert_eq!(super::effort_label("none"), "None");
        assert_eq!(super::effort_label("minimal"), "Minimal");
        assert_eq!(super::effort_label("low"), "Low");
        assert_eq!(super::effort_label("xhigh"), "Extra High");
        assert_eq!(super::effort_label("max"), "Max");
        assert_eq!(super::effort_label("ultra"), "Ultra");
        assert_eq!(super::effort_label("auto"), "Auto");
    }

    #[test]
    fn pick_test_model_prefers_enabled_grok() {
        let catalog = vec![
            crate::sand::SandFamily {
                id: "claude-haiku-4-5".into(),
                display_name: "Claude Haiku 4.5".into(),
                available: true,
                ..crate::sand::SandFamily::default()
            },
            crate::sand::SandFamily {
                id: "grok-4.6".into(),
                display_name: "Cursor Grok 4.6".into(),
                available: true,
                ..crate::sand::SandFamily::default()
            },
        ];
        let picked = super::pick_test_model(&catalog, &["grok-4.6".into()], &[]);
        assert_eq!(picked.as_deref(), Some("grok-4.6"));
    }

    #[test]
    fn calendar_cell_fits_side_column() {
        let cell = super::calendar_cell(360.0, 200.0, 17);
        assert!((8.0..=22.0).contains(&cell));
        let grid_w = 8.0 + 17.0 * cell + 16.0 * 3.0;
        assert!(grid_w <= 360.5);
    }
}

fn draw_trend_chart(ui: &mut egui::Ui, summary: &crate::usage::UsageSummary, height: f32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("使用趋势").size(14.0).strong());
        ui.label(
            RichText::new("灰柱是时间轴，绿柱是成功调用")
                .size(12.0)
                .color(theme::MUTED),
        );
    });
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().max(80.0), height),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(rect, 10.0, theme::PANEL);
    let bars = &summary.bars;
    if bars.is_empty() {
        return;
    }
    let inner = rect.shrink2(Vec2::new(12.0, 10.0));
    let label_h = 16.0;
    let plot = egui::Rect::from_min_max(inner.min, egui::pos2(inner.max.x, inner.max.y - label_h));
    let n = bars.len() as f32;
    let gap = if n > 40.0 { 1.0 } else { 3.0 };
    let bar_w = ((plot.width() - gap * (n - 1.0)) / n).max(2.0);
    let peak = bars.iter().map(|b| b.count).max().unwrap_or(0).max(1) as f32;
    for (index, bar) in bars.iter().enumerate() {
        let x = plot.left() + index as f32 * (bar_w + gap);
        let track =
            egui::Rect::from_min_size(egui::pos2(x, plot.top()), Vec2::new(bar_w, plot.height()));
        ui.painter()
            .rect_filled(track, 2.0, Color32::from_rgb(42, 48, 54));
        if bar.count > 0 {
            let h = (plot.height() * (bar.count as f32 / peak)).max(3.0);
            let fill =
                egui::Rect::from_min_size(egui::pos2(x, plot.bottom() - h), Vec2::new(bar_w, h));
            ui.painter().rect_filled(fill, 2.0, theme::OK);
        }
        let show_label = bars.len() <= 14 || index % ((bars.len() / 8).max(1)) == 0;
        if show_label {
            ui.painter().text(
                egui::pos2(x + bar_w / 2.0, inner.bottom() - 2.0),
                egui::Align2::CENTER_BOTTOM,
                &bar.label,
                egui::FontId::proportional(10.0),
                theme::MUTED,
            );
        }
    }
}

fn calendar_cell(width: f32, height: f32, weeks: usize) -> f32 {
    let weeks = weeks.max(1) as f32;
    let gap = 3.0;
    let pad = 8.0;
    let by_w = (width - pad - gap * (weeks - 1.0)) / weeks;
    let by_h = (height - pad - gap * 6.0) / 7.0;
    by_w.min(by_h).clamp(8.0, 22.0)
}

fn draw_calendar(ui: &mut egui::Ui, summary: &crate::usage::UsageSummary, width: f32, height: f32) {
    let gap = 3.0;
    let cols = (summary.bars.len() / 7).max(1);
    let cell = calendar_cell(width, height, cols);
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(width.max(80.0), height), egui::Sense::hover());
    ui.painter().rect_filled(rect, 8.0, theme::PANEL);
    let peak = summary
        .bars
        .iter()
        .map(|b| b.count)
        .max()
        .unwrap_or(0)
        .max(1);
    for (index, bar) in summary.bars.iter().enumerate() {
        let week = index / 7;
        let dow = index % 7;
        let x = rect.left() + 4.0 + week as f32 * (cell + gap);
        let y = rect.top() + 4.0 + dow as f32 * (cell + gap);
        let cell_rect = egui::Rect::from_min_size(egui::pos2(x, y), Vec2::splat(cell));
        let fill = if bar.count == 0 {
            Color32::from_rgb(42, 48, 54)
        } else {
            let t = (bar.count as f32 / peak as f32).clamp(0.15, 1.0);
            Color32::from_rgb(
                (0.0 + 0.0 * t) as u8,
                (80.0 + 120.0 * t) as u8,
                (60.0 + 80.0 * t) as u8,
            )
        };
        ui.painter().rect_filled(cell_rect, 2.0, fill);
    }
}

fn metric_card(ui: &mut egui::Ui, key: &str, value: &str, ok: bool) {
    egui::Frame::new()
        .fill(theme::PANEL)
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_width(120.0);
            ui.label(RichText::new(key).size(11.0).color(theme::MUTED));
            ui.label(RichText::new(value).size(15.0).strong().color(if ok {
                theme::TEXT
            } else {
                theme::MUTED
            }));
        });
}

fn ui_kind(kind: usize) -> crate::providers::ProviderKind {
    match kind {
        KIND_XAI => crate::providers::ProviderKind::Xai,
        KIND_ANTHROPIC => crate::providers::ProviderKind::Anthropic,
        KIND_GEMINI => crate::providers::ProviderKind::Gemini,
        KIND_ZHIPU => crate::providers::ProviderKind::Zhipu,
        KIND_KIMI => crate::providers::ProviderKind::Kimi,
        KIND_DEEPSEEK => crate::providers::ProviderKind::Deepseek,
        KIND_GENERIC => crate::providers::ProviderKind::Generic,
        _ => crate::providers::ProviderKind::Openai,
    }
}
