mod ai;
mod audio;
mod commands;
mod error;
mod frontmost_app;
mod groq;
pub mod meeting;
mod paste;
mod settings;
mod whisper_engine;

use commands::AppState;
use tauri::Manager;
use tauri::tray::TrayIconBuilder;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    let settings = commands::load_settings(app);
                    let state = app.state::<AppState>();
                    let rs = state.recording_state.lock().unwrap().clone();

                    match settings.recording_mode {
                        settings::RecordingMode::Toggle => {
                            if event.state == ShortcutState::Pressed {
                                let app = app.clone();
                                match rs {
                                    commands::RecordingState::Idle => {
                                        tauri::async_runtime::spawn(async move {
                                            if let Err(e) = commands::execute_start(app).await {
                                                log::error!("hotkey start failed: {e}");
                                            }
                                        });
                                    }
                                    commands::RecordingState::Recording => {
                                        tauri::async_runtime::spawn(async move {
                                            if let Err(e) = commands::execute_stop(app).await {
                                                log::error!("hotkey stop failed: {e}");
                                            }
                                        });
                                    }
                                    commands::RecordingState::Processing => {}
                                }
                            }
                        }
                        settings::RecordingMode::PushToTalk => {
                            let app = app.clone();
                            match event.state {
                                ShortcutState::Pressed => {
                                    if rs == commands::RecordingState::Idle {
                                        tauri::async_runtime::spawn(async move {
                                            if let Err(e) = commands::execute_start(app).await {
                                                log::error!("hotkey start failed: {e}");
                                            }
                                        });
                                    }
                                }
                                ShortcutState::Released => {
                                    if rs == commands::RecordingState::Recording {
                                        tauri::async_runtime::spawn(async move {
                                            if let Err(e) = commands::execute_stop(app).await {
                                                log::error!("hotkey stop failed: {e}");
                                            }
                                        });
                                    }
                                }
                            }
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            app.manage(AppState::new(app_data_dir));
            app.manage(crate::meeting::MeetingState::new());

            let settings = commands::load_settings(app.handle());
            if let Err(e) = app
                .handle()
                .global_shortcut()
                .register(settings.hotkey.as_str())
            {
                log::warn!("Failed to register hotkey '{}': {e}", settings.hotkey);
            }

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("iSpeak")
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { button: tauri::tray::MouseButton::Left, button_state: tauri::tray::MouseButtonState::Up, .. } = event {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_recording,
            commands::cancel_recording,
            commands::list_microphones,
            commands::get_installed_models,
            commands::download_model,
            commands::delete_model,
            commands::get_settings,
            commands::update_settings,
            crate::meeting::commands::meeting_enqueue_file,
            crate::meeting::commands::meeting_cancel,
            crate::meeting::commands::meeting_queue_snapshot,
            crate::meeting::commands::meeting_export,
            crate::meeting::commands::meeting_list_history,
            crate::meeting::commands::meeting_get_history,
            crate::meeting::commands::meeting_delete_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running iSpeak");
}
