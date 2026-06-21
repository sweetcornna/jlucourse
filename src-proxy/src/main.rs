//! 独立运行的代理二进制（用于纯 Web 调试流程：
//! `cargo run --manifest-path src-proxy/Cargo.toml`）。
//!
//! 历史上这里维护了一份与 `lib.rs` 平行、且已经漂移的实现
//! （缺少 127.0.0.1 来源、缺少 grablessons GET 路由、登录体用 JSON 而非
//! urlencoded、缺少 UA/Origin/Referer 处理），导致 Web 调试流程的行为
//! 比 Tauri 内嵌版本更差。现在二进制直接复用库实现，单一事实来源。
fn main() -> std::io::Result<()> {
    funky_lesson_proxy::main()
}
