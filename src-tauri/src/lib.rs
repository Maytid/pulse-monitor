pub mod sampler;

use sampler::{Sampler, Tick};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent};

struct AppState(Arc<Mutex<Sampler>>);

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

#[tauri::command]
fn get_history(state: State<AppState>) -> Vec<Tick> {
    state.0.lock().unwrap().history.iter().cloned().collect()
}

#[tauri::command]
fn kill_proc(pid: i32, force: bool) -> Result<(), String> {
    let sig = if force { libc::SIGKILL } else { libc::SIGTERM };
    let ret = unsafe { libc::kill(pid, sig) };
    if ret == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let mut s = Sampler::new();
            s.sample(now());
            let shared = Arc::new(Mutex::new(s));
            let bg = Arc::clone(&shared);
            let handle: AppHandle = app.handle().clone();

            thread::spawn(move || loop {
                thread::sleep(Duration::from_secs(1));
                let tick = bg.lock().unwrap().sample(now());
                let _ = handle.emit("tick", &tick);
            });

            app.manage(AppState(shared));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![get_history, kill_proc])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let RunEvent::Reopen {
                has_visible_windows,
                ..
            } = event
            {
                if !has_visible_windows {
                    if let Some(window) = app_handle
                        .get_webview_window("main")
                        .or_else(|| app_handle.webview_windows().values().next().cloned())
                    {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        });
}