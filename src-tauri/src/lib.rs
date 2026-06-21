// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use tauri::Manager;

/// 兜底（A）：在内嵌 webview 里打开官方选课网站，并把 app 已登录的 token 注入
/// 官网的 sessionStorage["token"]（官网 config.min.js 据此设置 Authorization）。
/// 复用同一 token = 同一会话 = 同一设备，不会触发二次登录、不会把自己挤下线。
/// 同步命令：Tauri 在主线程执行，满足 macOS 等平台「窗口必须在主线程创建」。
#[tauri::command]
fn open_official_fallback(app: tauri::AppHandle, token: String) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    // 已打开则聚焦，避免重复建窗。
    if let Some(win) = app.get_webview_window("official") {
        let _ = win.set_focus();
        return Ok(());
    }

    let url = tauri::Url::parse("https://icourses.jlu.edu.cn/xsxk/profile/index.html")
        .map_err(|e| e.to_string())?;
    // serde_json 序列化保证 token 作为 JS 字符串字面量安全嵌入（防注入）。
    let token_js = serde_json::to_string(&token).map_err(|e| e.to_string())?;
    let script = format!("try{{window.sessionStorage.setItem('token', {token_js});}}catch(e){{}}");

    WebviewWindowBuilder::new(&app, "official", WebviewUrl::External(url))
        .title("吉林大学选课 · 官方网站（兜底）")
        .inner_size(1100.0, 800.0)
        .initialization_script(&script)
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
