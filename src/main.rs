#![windows_subsystem = "windows"]

use grok_bot_auth::api::{router, start_coexist, AppState};
use grok_bot_auth::client::SandClient;
use grok_bot_auth::store;
use grok_bot_auth::ui::GrokBotApp;

const BIND: &str = "127.0.0.1:47821";

#[cfg(windows)]
mod wincon {
    #[link(name = "kernel32")]
    extern "system" {
        pub fn AllocConsole() -> i32;
        pub fn SetConsoleOutputCP(cp: u32) -> i32;
    }
}

#[cfg(windows)]
fn alloc_console() {
    unsafe {
        wincon::AllocConsole();
        wincon::SetConsoleOutputCP(65001);
    }
}

struct CursorMitmPark;

impl Drop for CursorMitmPark {
    fn drop(&mut self) {
        let _ = grok_bot_auth::settings::clear_proxy_settings();
        let _ = grok_bot_auth::cursor_redirect::restore();
        let _ = grok_bot_auth::cursor_ws::restore_agent_websocket();
        let _ = grok_bot_auth::cursor_ws::scrub_transport_leftovers();
    }
}

fn main() -> eframe::Result<()> {
    let _park = CursorMitmPark;
    std::panic::set_hook(Box::new(|info| {
        let path = std::env::temp_dir().join("grok-bot-auth-panic.log");
        let _ = std::fs::write(path, format!("{info}"));
    }));
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let state = rt.block_on(async {
        let prefs_boot = store::load_prefs().await.unwrap_or_default();
        let outbound = (!prefs_boot.outbound_proxy.trim().is_empty())
            .then_some(prefs_boot.outbound_proxy.as_str());
        let client = SandClient::with_outbound(outbound)
            .or_else(|_| SandClient::new())
            .expect("http client");
        let grok_pool = store::load_grok_pool().await.unwrap_or_default();
        let account = grok_pool
            .slots
            .iter()
            .find(|slot| grok_bot_auth::sand::account_id(slot) == grok_pool.active_id)
            .cloned()
            .or_else(|| grok_pool.slots.first().cloned());
        let mut prefs = store::load_prefs().await.unwrap_or_default();
        let from_file = store::load_providers().await.unwrap_or_default();
        prefs.providers =
            grok_bot_auth::providers::merge_provider_secrets(prefs.providers, from_file);
        AppState::new(client, account, prefs, grok_pool)
    });
    let server_state = state.clone();
    let auto_coexist = rt.block_on(async {
        let saved = *state.coexist.lock().await;
        if !saved {
            if matches!(
                grok_bot_auth::settings::current_proxy(),
                Ok(Some(url)) if url.contains("47822")
            ) {
                let _ = grok_bot_auth::settings::clear_proxy_settings();
            }
            let _ = grok_bot_auth::cursor_ws::scrub_transport_leftovers();
        }
        saved
    });
    if std::env::args().any(|arg| arg == "--headless") {
        #[cfg(windows)]
        alloc_console();
        let park = state.clone();
        let boot = state.clone();
        rt.block_on(async move {
            match tokio::net::TcpListener::bind(BIND).await {
                Ok(listener) => {
                    eprintln!("headless relay {BIND}");
                    if auto_coexist {
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        match start_coexist(&boot).await {
                            Ok(url) => eprintln!("restored coexist {url}"),
                            Err(error) => eprintln!("restore coexist failed: {error}"),
                        }
                    }
                    if let Err(error) = axum::serve(listener, router(server_state)).await {
                        eprintln!("local relay stopped: {error}");
                    }
                }
                Err(error) => eprintln!("local relay {BIND} unavailable: {error}"),
            }
            if let Err(error) = grok_bot_auth::api::park_cursor_mitm(&park).await {
                eprintln!("park Cursor 反代 failed: {error}");
            }
        });
        return Ok(());
    }
    let boot = state.clone();
    rt.spawn(async move {
        match tokio::net::TcpListener::bind(BIND).await {
            Ok(listener) => {
                if auto_coexist {
                    let boot = boot.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        match start_coexist(&boot).await {
                            Ok(url) => eprintln!("restored coexist {url}"),
                            Err(error) => eprintln!("restore coexist failed: {error}"),
                        }
                    });
                }
                let _ = axum::serve(listener, router(server_state)).await;
            }
            Err(error) => eprintln!("local relay {BIND} unavailable: {error}"),
        }
    });
    let icon = egui::IconData {
        rgba: include_bytes!("../assets/icon.rgba").to_vec(),
        width: 256,
        height: 256,
    };
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([960.0, 640.0])
            .with_max_inner_size([1680.0, 1120.0])
            .with_title("Grok-Bot-Auth")
            .with_icon(icon),
        ..Default::default()
    };
    let handle = rt.handle().clone();
    std::mem::forget(rt);
    let result = eframe::run_native(
        "Grok-Bot-Auth",
        native,
        Box::new(move |cc| Ok(Box::new(GrokBotApp::new(cc, handle, state)))),
    );
    // Window close already parks; this covers panic-free return if on_exit skipped.
    let _ = grok_bot_auth::settings::clear_proxy_settings();
    let _ = grok_bot_auth::cursor_redirect::restore();
    let _ = grok_bot_auth::cursor_ws::restore_agent_websocket();
    let _ = grok_bot_auth::cursor_ws::scrub_transport_leftovers();
    result
}
