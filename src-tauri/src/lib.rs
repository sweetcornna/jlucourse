// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use tauri::Manager;

/// 兜底：在内嵌 webview 里打开官方选课网站，供手动登录后手动选课。
/// 说明：app 的登录态在本地代理侧（reqwest 维护着服务端会话/cookie），无法可靠转移到
/// webview——实测把 token 注入 sessionStorage 后，鉴权接口 elective/user 仍被官网以 HTML
/// 拒绝（同一 token 经代理却可用，且 token/UA/cookie 都已对齐）。故此处不做自动登录；
/// 脚本失效时的「就地补刀」用每门课的「选一次」（走代理、登录态有效）。
#[tauri::command]
fn open_official_fallback(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    // 已打开则聚焦，避免重复建窗。
    if let Some(win) = app.get_webview_window("official") {
        let _ = win.set_focus();
        return Ok(());
    }

    let url = tauri::Url::parse("https://icourses.jlu.edu.cn/xsxk/profile/index.html")
        .map_err(|e| e.to_string())?;
    WebviewWindowBuilder::new(&app, "official", WebviewUrl::External(url))
        .title("吉林大学选课 · 官方网站")
        .inner_size(1100.0, 800.0)
        // 与代理一致的 Chrome UA（官网对 UA 较敏感，统一更稳）。
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/139.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

// #[cfg_attr(mobile, tauri::mobile_entry_point)]
// #[tokio::main]

#[cfg(not(mobile))]
#[tokio::main]
pub async fn run() {
    tokio::spawn(async move {
        let _ = tokio::task::spawn_blocking(|| {
            let _ = funky_lesson_proxy::main();
        })
        .await;
    });
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![open_official_fallback])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(mobile)]
#[tauri::mobile_entry_point]
pub async fn run() {
    // Mobile implementation

    tokio::spawn(async move {
        let _ = tokio::task::spawn_blocking(|| {
            let _ = funky_lesson_proxy::main();
        })
        .await;
    });
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![open_official_fallback])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
