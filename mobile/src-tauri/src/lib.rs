//! The Nylonite floor app: a Tauri shell around the React client.
//!
//! # There is no second interface
//!
//! `frontendDist` points at `client/dist-mobile`, which is the same client the
//! browser gets, built in a mode that wires the transport to a bearer token and
//! an absolute server (see `client/app/platform/mobile.ts`). Every screen, the
//! design system, the four shells and the render gate's sixty-eight fixtures
//! are the ones that already exist. That is the whole argument for a webview
//! shell over a second native client, and it holds exactly as long as this
//! crate stays small.
//!
//! What belongs here is what the web cannot do: the native HTTP client below,
//! and — next — a camera that decodes a barcode and hands the string to the
//! same `onScan` a wedge scanner types into.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the Nylonite shell");
}
