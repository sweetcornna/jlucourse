use crate::external_browser::open_external_browser;
use crate::external_link::ExternalLink;
use funky_lesson_core::{
    client::gloo,
    crypto,
    error::{ErrorKind, Result},
    model::structs::{BatchInfo, CourseInfo, EnrollmentStatus},
};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::*;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use wasm_bindgen::prelude::*;

// Toast types
#[derive(Clone, PartialEq)]
pub enum ToastType {
    Success,
    Error,
    Info,
    Warning,
}

// Toast data structure
#[derive(Clone)]
pub struct Toast {
    pub id: u32,
    pub message: String,
    pub toast_type: ToastType,
}

// 全局 Toast 状态
static TOAST_ID: AtomicU32 = AtomicU32::new(0);
static TOAST_STORE: LazyLock<Mutex<RwSignal<Vec<Toast>>>> =
    LazyLock::new(|| Mutex::new(RwSignal::new(Vec::new())));

// Toast 工具函数
pub fn show_toast(message: String, toast_type: ToastType) {
    let id = TOAST_ID.fetch_add(1, Ordering::Relaxed);

    let toast = Toast {
        id,
        message,
        toast_type,
    };

    // 添加到全局存储
    if let Ok(toasts_signal) = TOAST_STORE.lock() {
        toasts_signal.update(|toasts| toasts.push(toast));

        spawn_local(async move {
            set_timeout(3000).await;
            if let Ok(toasts_signal) = TOAST_STORE.lock() {
                toasts_signal.update(|toasts| {
                    toasts.retain(|t| t.id != id);
                });
            }
        });
    }
}

// 便捷函数
pub fn toast_success(message: impl Into<String>) {
    show_toast(message.into(), ToastType::Success);
}

pub fn toast_error(message: impl Into<String>) {
    show_toast(message.into(), ToastType::Error);
}

pub fn toast_info(message: impl Into<String>) {
    show_toast(message.into(), ToastType::Info);
}

pub fn toast_warning(message: impl Into<String>) {
    show_toast(message.into(), ToastType::Warning);
}

// ---- UI helpers (presentation only) ---------------------------------------

// 根据状态文本推断登录状态行的语义类型
fn status_kind(msg: &str) -> &'static str {
    if msg.contains("成功") {
        "success"
    } else if msg.contains("失败") || msg.contains("错误") {
        "error"
    } else if msg.contains("请输入") || msg.contains("请重新") || msg.contains("请登录") {
        "warning"
    } else if msg.contains("正在") || msg.contains("设置批次") || msg.contains("获取") {
        "loading"
    } else {
        "idle"
    }
}

// 根据选课状态文本推断日志行配色
fn log_kind(s: &str) -> &'static str {
    if s.contains("成功") || s.contains("已选") {
        "success"
    } else if s.contains("等待") || s.contains("未开始") {
        "wait"
    } else if s.contains("错误")
        || s.contains("失败")
        || s.contains("已满")
        || s.contains("未登录")
        || s.contains("参数")
    {
        "error"
    } else {
        "info"
    }
}

// 把 "[课程名]状态" 拆分为 (标签, 正文)
fn split_tag(s: &str) -> (String, String) {
    if let Some(idx) = s.find(']') {
        (s[..=idx].to_string(), s[idx + 1..].trim_start().to_string())
    } else {
        (String::new(), s.to_string())
    }
}

// Leptos资源和信号
#[derive(Clone)]
pub struct AppState {
    pub token: RwSignal<Option<String>>,
    pub batch_id: RwSignal<Option<String>>,
    pub batch_list: RwSignal<Vec<BatchInfo>>,
    pub selected_courses: RwSignal<Vec<CourseInfo>>,
    pub favorite_courses: RwSignal<Vec<CourseInfo>>,
    pub enrollment_status: RwSignal<EnrollmentStatus>,
    pub should_continue: RwSignal<bool>,
    // 单调递增的运行代号：每次开始抢课 +1。worker 捕获开始时的代号，
    // 一旦发现代号变了（用户停止后又重新开始）便自行退出，避免上一轮的残留
    // worker 复用共享信号、干扰新一轮的运行状态。
    pub run_epoch: Arc<AtomicUsize>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            token: RwSignal::new(None),
            batch_id: RwSignal::new(None),
            batch_list: RwSignal::new(Vec::new()),
            selected_courses: RwSignal::new(Vec::new()),
            favorite_courses: RwSignal::new(Vec::new()),
            enrollment_status: RwSignal::new(EnrollmentStatus::default()),
            should_continue: RwSignal::new(false),
            run_epoch: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn reset_for_login(&self) {
        self.token.set(None);
        self.batch_id.set(None);
        self.batch_list.set(Vec::new());
        self.selected_courses.set(Vec::new());
        self.favorite_courses.set(Vec::new());
        self.enrollment_status.set(EnrollmentStatus::default());
        self.should_continue.set(false);
    }

    pub fn reset_for_batch_selection(&self) {
        self.batch_id.set(None);
        self.selected_courses.set(Vec::new());
        self.favorite_courses.set(Vec::new());
        self.enrollment_status.set(EnrollmentStatus::default());
        self.should_continue.set(false);
    }
}

// Toast Component — 现代化、克制的轻量通知
#[component]
pub fn ToastContainer() -> impl IntoView {
    let toasts = if let Ok(toasts_signal) = TOAST_STORE.lock() {
        *toasts_signal
    } else {
        RwSignal::new(Vec::new())
    };

    view! {
        <div class="toast-container">
            <For
                each=move || toasts.get()
                key=|toast| toast.id
                children=move |toast| {
                    let cls = match toast.toast_type {
                        ToastType::Success => "toast toast--success",
                        ToastType::Error => "toast toast--error",
                        ToastType::Info => "toast toast--info",
                        ToastType::Warning => "toast toast--warning",
                    };

                    let icon = match toast.toast_type {
                        ToastType::Success => view! {
                            <svg class="toast__icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>
                        }.into_any(),
                        ToastType::Error => view! {
                            <svg class="toast__icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 8v4M12 16h.01"/></svg>
                        }.into_any(),
                        ToastType::Info => view! {
                            <svg class="toast__icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/></svg>
                        }.into_any(),
                        ToastType::Warning => view! {
                            <svg class="toast__icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><path d="M12 9v4M12 17h.01"/></svg>
                        }.into_any(),
                    };

                    let close_toast = {
                        let toast_id = toast.id;
                        move |_| {
                            if let Ok(toasts_signal) = TOAST_STORE.lock() {
                                toasts_signal.update(|toasts| {
                                    toasts.retain(|t| t.id != toast_id);
                                });
                            }
                        }
                    };

                    view! {
                        <div class=cls>
                            {icon}
                            <span class="toast__msg">{toast.message}</span>
                            <button class="toast__close" on:click=close_toast aria-label="关闭通知">"×"</button>
                        </div>
                    }
                }
            />
        </div>
    }
}

// 登录函数
pub async fn login(
    username: &str,
    password: &str,
    captcha: &str,
    uuid: &str,
    app_state: &AppState,
) -> Result<()> {
    // 初始化
    gloo::create_client().await?;

    // 获取AES密钥
    let aes_key = gloo::get_aes_key_proxy().await?;

    // 加密密码并登录
    let encrypted_password = crypto::encrypt_password(password, &aes_key)?;
    let login_resp =
        gloo::send_login_request_proxy(username, &encrypted_password, captcha, uuid).await?;

    if login_resp["code"] == 200 && login_resp["msg"] == "登录成功" {
        let token = login_resp["data"]["token"]
            .as_str()
            .ok_or_else(|| ErrorKind::ParseError("Invalid token".to_string()))?
            .to_string();

        let batch_list: Vec<BatchInfo> =
            serde_json::from_value(login_resp["data"]["student"]["electiveBatchList"].clone())?;

        // 更新状态
        app_state.token.set(Some(token));
        app_state.batch_list.set(batch_list);
        Ok(())
    } else {
        Err(
            ErrorKind::ParseError(login_resp["msg"].as_str().unwrap_or("登录失败").to_string())
                .into(),
        )
    }
}

// 获取验证码
pub async fn get_captcha() -> Result<(String, String)> {
    gloo::get_captcha_proxy().await
}

// 设置选课批次
pub async fn set_batch(batch_idx: usize, app_state: &AppState) -> Result<()> {
    let token = app_state
        .token
        .get()
        .ok_or_else(|| ErrorKind::ParseError("No token available".to_string()))?;
    let batch_list = app_state.batch_list.get();

    if batch_idx >= batch_list.len() {
        return Err(ErrorKind::ParseError("Invalid batch index".to_string()).into());
    }

    let batch_id = batch_list[batch_idx].code.clone();
    let resp = gloo::set_batch_proxy(&batch_id, &token).await?;

    if resp["code"] != 200 {
        return Err(ErrorKind::ParseError("Failed to set batch".to_string()).into());
    }

    app_state.batch_id.set(Some(batch_id));
    Ok(())
}

// 获取课程列表
pub async fn get_courses(app_state: &AppState) -> Result<()> {
    let token = app_state
        .token
        .get()
        .ok_or_else(|| ErrorKind::ParseError("No token available".to_string()))?;
    let batch_id = app_state
        .batch_id
        .get()
        .ok_or_else(|| ErrorKind::ParseError("No batch id selected".to_string()))?;

    let selected = gloo::get_selected_courses_proxy(&token, &batch_id).await?;
    let selected_courses: Vec<CourseInfo> = if selected["code"] == 200 {
        serde_json::from_value(selected["data"].clone())?
    } else {
        return Err(ErrorKind::CourseError(
            selected["msg"]
                .as_str()
                .unwrap_or("获取已选课程失败")
                .to_string(),
        )
        .into());
    };

    let favorite = gloo::get_favorite_courses_proxy(&token, &batch_id).await?;
    let favorite_courses: Vec<CourseInfo> = if favorite["code"] == 200 {
        serde_json::from_value(favorite["data"].clone())?
    } else {
        return Err(ErrorKind::CourseError(
            favorite["msg"]
                .as_str()
                .unwrap_or("获取收藏课程失败")
                .to_string(),
        )
        .into());
    };

    app_state.selected_courses.set(selected_courses);
    app_state.favorite_courses.set(favorite_courses);
    Ok(())
}

// 单次选课请求结果的语义判定（纯函数，便于单元测试）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnrollDecision {
    /// 写入该课程状态行的文案
    label: &'static str,
    /// 该课程是否已到达终态、应停止其自身的轮询
    stop_self: bool,
    /// 是否为致命错误（如未登录），应停止所有课程的轮询
    fatal: bool,
}

fn classify_enroll(code: i64, msg: &str, try_if_capacity_full: bool) -> EnrollDecision {
    let d = |label, stop_self, fatal| EnrollDecision {
        label,
        stop_self,
        fatal,
    };
    match (code, msg) {
        (200, _) => d("选课成功", true, false),
        (500, "该课程已在选课结果中") => d("已选", true, false),
        (500, "本轮次选课暂未开始") => d("未开始", false, false),
        (500, "课容量已满") if !try_if_capacity_full => d("已满", true, false),
        (500, "课容量已满") => d("等待中", false, false),
        (500, "参数校验不通过") => d("参数错误", false, false),
        (401, _) => d("未登录", true, true),
        _ => d("失败", false, false),
    }
}

/// 在 done 标记里，从 start 开始环形查找下一门未完成课程的下标。
fn next_pending(start: usize, done: &[bool]) -> Option<usize> {
    let n = done.len();
    if n == 0 {
        return None;
    }
    (0..n).map(|off| (start + off) % n).find(|&i| !done[i])
}

// 并发抢课的工作协程数量（沿用原设计的 12 路并发以提升抢中率）。
const ENROLL_WORKERS: usize = 12;

// 选课函数
pub async fn enroll_courses(
    courses: Vec<CourseInfo>,
    try_if_capacity_full: bool,
    app_state: &AppState,
) -> Result<()> {
    if courses.is_empty() {
        return Ok(());
    }

    let token = app_state
        .token
        .get()
        .ok_or_else(|| ErrorKind::ParseError("No token available".to_string()))?;
    let batch_id = app_state
        .batch_id
        .get()
        .ok_or_else(|| ErrorKind::ParseError("No batch id selected".to_string()))?;

    app_state.should_continue.set(true);
    app_state.enrollment_status.update(|status| {
        status.is_running = true;
        status.total_requests = 0;
        status.course_statuses = courses
            .iter()
            .map(|c| format!("[{}]等待中", c.KCM))
            .collect();
    });

    let count = courses.len();
    let courses = Arc::new(courses);
    // 每门课一个“已完成”标记：成功 / 已选 / 已满(不再尝试) 后置位，worker 便不再请求它。
    let done: Arc<Vec<AtomicBool>> = Arc::new((0..count).map(|_| AtomicBool::new(false)).collect());
    // 共享游标：多个 worker 据此环形领取下一门未完成课程；课程数少于 worker 数时
    // 多个 worker 会并发抢同一门课，恢复原先的高并发抢中策略，但写入按下标 get_mut 保护、
    // 终止改用 per-course done 标记，规避旧实现的下标竞争 / 越界 / “一门成功停全部”。
    let cursor = Arc::new(AtomicUsize::new(0));
    // active 计数在所有 worker 退出后把 is_running 复位（修复 UI 卡在“运行中”）。
    let active = Arc::new(AtomicUsize::new(ENROLL_WORKERS));
    // 本轮运行代号：用于让“停止后立刻重新开始”时，上一轮的残留 worker 自行退出。
    let epoch = app_state
        .run_epoch
        .fetch_add(1, Ordering::AcqRel)
        .wrapping_add(1);

    for _ in 0..ENROLL_WORKERS {
        let token = token.clone();
        let batch_id = batch_id.clone();
        let app_state = app_state.clone();
        let courses = courses.clone();
        let done = done.clone();
        let cursor = cursor.clone();
        let active = active.clone();

        spawn_local(async move {
            // 仅当用户未停止且仍是本轮（代号未被新一轮覆盖）时继续。
            while app_state.should_continue.get()
                && app_state.run_epoch.load(Ordering::Acquire) == epoch
            {
                // 领取下一门未完成课程；全部完成则退出。
                let snapshot: Vec<bool> = done.iter().map(|b| b.load(Ordering::Acquire)).collect();
                let start = cursor.fetch_add(1, Ordering::Relaxed);
                let Some(idx) = next_pending(start, &snapshot) else {
                    break;
                };
                let course = &courses[idx];

                app_state.enrollment_status.update(|status| {
                    status.total_requests = status.total_requests.saturating_add(1);
                });

                let result = gloo::select_course_proxy(
                    &token,
                    &batch_id,
                    &course.teaching_class_type.clone().unwrap_or_default(),
                    &course.JXBID,
                    &course.secret_val.clone().unwrap_or_default(),
                )
                .await;

                let (label, stop_self, fatal) = match result {
                    Ok(json) => {
                        let code = json["code"].as_i64().unwrap_or(0);
                        let msg = json["msg"].as_str().unwrap_or("");
                        let decision = classify_enroll(code, msg, try_if_capacity_full);
                        (decision.label, decision.stop_self, decision.fatal)
                    }
                    Err(e) => {
                        log::error!("请求错误: {e:?}");
                        ("请求错误", false, false)
                    }
                };

                // 先标记终态，再决定是否写状态：N 个 worker 并发抢同一门课时，
                // 若该课已被某个 worker 的终态结果敲定（done=true），慢到的非终态
                // 重复请求就不应把“选课成功”覆盖回“等待中”。
                if stop_self {
                    done[idx].store(true, Ordering::Release);
                }
                if stop_self || !done[idx].load(Ordering::Acquire) {
                    app_state.enrollment_status.update(|s| {
                        if let Some(slot) = s.course_statuses.get_mut(idx) {
                            *slot = format!("[{}]{}", course.KCM, label);
                        }
                    });
                }

                // 致命错误（如未登录）停止全部课程。
                if fatal {
                    app_state.should_continue.set(false);
                }
                // 用户已停止、或已开启新一轮，则立即退出，不必再等待 200ms。
                if !app_state.should_continue.get()
                    || app_state.run_epoch.load(Ordering::Acquire) != epoch
                {
                    break;
                }

                // 短暂延迟避免请求过快
                set_timeout(200).await;
            }

            // 本 worker 退出：最后一个退出者复位运行状态——但仅当仍是本轮，
            // 以免上一轮的残留 worker 把新一轮的运行状态误关掉。
            if active.fetch_sub(1, Ordering::AcqRel) == 1
                && app_state.run_epoch.load(Ordering::Acquire) == epoch
            {
                app_state.enrollment_status.update(|status| {
                    status.is_running = false;
                });
                app_state.should_continue.set(false);
            }
        });
    }

    Ok(())
}

// 停止选课
pub fn stop_enrollment(app_state: &AppState) {
    app_state.should_continue.set(false);
    app_state.enrollment_status.update(|status| {
        status.is_running = false;
    });
}

// 手动「选一次」：用 app 现有登录态对单门课打一发选课请求，作为自动脚本的兜底（B）。
// 复用同一 token/批次，不新开会话，桌面 / Web / Android 都可用。
fn manual_select_once(course: CourseInfo, app_state: AppState) {
    spawn_local(async move {
        let Some(token) = app_state.token.get() else {
            toast_error("未登录，请先登录");
            return;
        };
        let Some(batch_id) = app_state.batch_id.get() else {
            toast_error("请先选择批次");
            return;
        };
        match gloo::select_course_proxy(
            &token,
            &batch_id,
            &course.teaching_class_type.clone().unwrap_or_default(),
            &course.JXBID,
            &course.secret_val.clone().unwrap_or_default(),
        )
        .await
        {
            Ok(json) => {
                let code = json["code"].as_i64().unwrap_or(0);
                let msg = json["msg"].as_str().unwrap_or("");
                let decision = classify_enroll(code, msg, true);
                let text = format!("[{}]{}", course.KCM, decision.label);
                if decision.fatal {
                    toast_error(text);
                } else if decision.stop_self {
                    toast_success(text);
                } else {
                    toast_info(text);
                }
            }
            Err(e) => {
                log::error!("手动选课请求错误: {e:?}");
                toast_error(format!("[{}]请求错误", course.KCM));
            }
        }
    });
}

// 通过全局 __TAURI__（withGlobalTauri=true）调用自定义命令；非 Tauri 环境返回 false。
// 兼容多种 invoke 暴露形态（core.invoke / invoke / tauri.invoke）以防版本差异。
#[wasm_bindgen(inline_js = r#"
export function __funky_tauri_invoke(cmd, payload) {
  try {
    const t = window.__TAURI__;
    const invoke = t && ((t.core && t.core.invoke) || t.invoke || (t.tauri && t.tauri.invoke));
    if (typeof invoke === 'function') {
      Promise.resolve(invoke(cmd, payload)).catch(function (e) { console.error('open_official_fallback invoke failed:', e); });
      return true;
    }
  } catch (e) { console.error(e); }
  return false;
}
"#)]
extern "C" {
    fn __funky_tauri_invoke(cmd: &str, payload: JsValue) -> bool;
}

// 兜底（A）：打开官方选课网站。
// 桌面端（Tauri）→ 内嵌 webview，并把当前 token / 批次注入官网 sessionStorage（同一会话/同一设备）。
// 非桌面端（Web / Android 无原生子窗口）→ 退化为系统浏览器打开（需自行登录）。
fn open_official_fallback(app_state: AppState) {
    let Some(token) = app_state.token.get() else {
        toast_error("请先登录后再使用兜底");
        return;
    };
    let payload = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &payload,
        &JsValue::from_str("token"),
        &JsValue::from_str(&token),
    );
    if __funky_tauri_invoke("open_official_fallback", payload.into()) {
        toast_info("已在内嵌窗口打开官方选课网站（已带入登录态）");
    } else {
        match open_external_browser("https://icourses.jlu.edu.cn/xsxk/profile/index.html") {
            Ok(()) => toast_warning("未检测到桌面环境：已用系统浏览器打开（需自行登录）"),
            Err(_) => toast_error("无法打开官方网站"),
        }
    }
}

// Utility functions
// 使用 gloo-timers 的 TimeoutFuture，避免手写 setTimeout/Promise 时的三处 unwrap
// （window() / set_timeout / Promise 拒绝任一失败都会 panic 掉整个 WASM 模块，
//   而该延时在 12 路抢课循环里每 200ms 调用一次，panic 会让全部线程与 UI 同时死掉）。
async fn set_timeout(ms: u32) {
    gloo_timers::future::TimeoutFuture::new(ms).await;
}

#[component]
pub fn App() -> impl IntoView {
    let app_state = RwSignal::new(AppState::new());

    let (username, set_username) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (captcha, set_captcha) = signal(String::new());
    let (captcha_image_src, set_captcha_image_src) = signal(String::new());
    let (captcha_uuid, set_captcha_uuid) = signal(String::new());
    let (status_message, set_status_message) = signal("请登录".to_string());
    let (step, set_step) = signal(1);
    // 抢课中状态由真实的 enrollment_status.is_running 派生，避免出现“UI 显示运行中
    // 但任务其实早已结束”的不一致（所有任务退出后 is_running 会被复位）。
    let is_enrolling = Memo::new(move |_| app_state.get().enrollment_status.with(|s| s.is_running));

    // Back button handler
    let handle_back = move |_| {
        let current_step = step.get();
        match current_step {
            2 => {
                // 从批次选择回到登录
                set_step.set(1);
                app_state.get().reset_for_login();
                set_status_message.set("请重新登录".to_string());
                toast_info("已返回登录页面");
            }
            3 => {
                // 从课程选择回到批次选择
                set_step.set(2);
                app_state.get().reset_for_batch_selection();
                set_status_message.set("请重新选择批次".to_string());
                toast_info("已返回批次选择");
            }
            _ => {}
        }
    };

    // 获取验证码
    let handle_get_captcha = move |_| {
        spawn_local(async move {
            match get_captcha().await {
                Ok((uuid, captcha_b64)) => {
                    set_captcha_uuid.set(uuid);
                    let image_src = captcha_b64.to_string();
                    set_captcha_image_src.set(image_src);
                    toast_success("验证码已刷新");
                }
                Err(e) => {
                    let error_msg = format!("获取验证码失败：{e:?}");
                    set_status_message.set(error_msg.clone());
                    toast_error(error_msg);
                }
            }
        });
    };

    // 登录处理
    let handle_login = {
        let username = username;
        let password = password;
        let captcha = captcha;
        let captcha_uuid = captcha_uuid;
        let set_status_message = set_status_message;
        let set_step = set_step;
        let app_state = app_state;
        let handle_get_captcha = handle_get_captcha;

        move |ev: web_sys::SubmitEvent| {
            ev.prevent_default();

            if username.get().is_empty() {
                let msg = "请输入学号".to_string();
                set_status_message.set(msg.clone());
                toast_warning(msg);
                return;
            }
            if password.get().is_empty() {
                let msg = "请输入密码".to_string();
                set_status_message.set(msg.clone());
                toast_warning(msg);
                return;
            }
            if captcha.get().is_empty() {
                let msg = "请输入验证码".to_string();
                set_status_message.set(msg.clone());
                toast_warning(msg);
                return;
            }

            let current_state = app_state.get();
            let set_status_message = set_status_message;
            let set_step = set_step;
            let captcha_uuid = captcha_uuid;
            let username = username;
            let password = password;
            let captcha = captcha;
            let handle_get_captcha = handle_get_captcha;

            spawn_local(async move {
                match login(
                    &username.get(),
                    &password.get(),
                    &captcha.get(),
                    &captcha_uuid.get(),
                    &current_state,
                )
                .await
                {
                    Ok(()) => {
                        set_step.set(2);
                        set_status_message.set("登录成功！".to_string());
                        toast_success("登录成功！");
                    }
                    Err(e) => {
                        let error_msg = format!("登录失败：{e:?}");
                        set_status_message.set(error_msg.clone());
                        toast_error(error_msg);
                        // 登录失败：刷新验证码图并清空输入框（旧验证码对新图无效）
                        set_captcha.set(String::new());
                        handle_get_captcha(());
                    }
                }
            });
        }
    };

    // 选择批次
    let handle_batch_select = move |idx: usize| {
        let current_state = app_state.get();
        set_status_message.set("正在设置批次...".to_string());

        spawn_local(async move {
            match set_batch(idx, &current_state).await {
                Ok(()) => {
                    set_step.set(3);
                    match get_courses(&current_state).await {
                        Ok(()) => {
                            set_status_message.set("获取课程成功".to_string());
                            toast_success("批次设置成功，已获取课程列表");
                        }
                        Err(e) => {
                            let error_msg = format!("获取课程失败：{e:?}");
                            set_status_message.set(error_msg.clone());
                            toast_error(error_msg);
                        }
                    }
                }
                Err(e) => {
                    let error_msg = format!("选择批次失败：{e:?}");
                    set_status_message.set(error_msg.clone());
                    toast_error(error_msg);
                }
            }
        });
    };

    // 开始抢课
    let handle_enroll = move |_| {
        let current_state = app_state.get();
        let courses = current_state.favorite_courses.get();
        if courses.is_empty() {
            toast_warning("暂无收藏课程，请先到选课网站收藏想抢的课程");
            return;
        }
        // 立即给出反馈（任务在下个微任务才真正把 is_running 置位）
        current_state
            .enrollment_status
            .update(|s| s.is_running = true);
        toast_info("开始抢课...");

        spawn_local(async move {
            if let Err(e) = enroll_courses(courses, true, &current_state).await {
                let error_msg = format!("抢课出错：{e:?}");
                set_status_message.set(error_msg.clone());
                toast_error(error_msg);
                current_state
                    .enrollment_status
                    .update(|s| s.is_running = false);
            }
        });
    };

    // 停止抢课
    let handle_stop_enroll = move |_| {
        let current_state = app_state.get();
        stop_enrollment(&current_state);
        toast_warning("已停止抢课");
    };

    // 初始化时获取验证码
    Effect::new(move |_| {
        handle_get_captcha(());
    });

    // 在使用 batch_list 时使用 app_state
    let batch_list = move || app_state.get().batch_list.get();

    view! {
        <div class="app-root">

            // ============================ 登录 ============================
            <div class="screen" class:hidden=move || step.get() != 1>
                <div class="login">
                    <form class="login__card" on:submit=handle_login>
                        <div class="brand">
                            <div class="brand__mark" aria-hidden="true">
                                <svg viewBox="0 0 24 24" fill="none"><path d="M13 2 4.6 13.2a.8.8 0 0 0 .64 1.28H10.2l-1.1 7.2a.5.5 0 0 0 .9.38L19.4 10.8a.8.8 0 0 0-.64-1.28H13.7L14.9 2.6A.5.5 0 0 0 13 2Z" fill="currentColor"/></svg>
                            </div>
                            <div class="brand__name">"FunkyLesson"</div>
                            <div class="brand__sub">"吉林大学选课助手"</div>
                        </div>

                        <div class="field">
                            <label class="field__label" for="username">"学号"<span class="req" aria-hidden="true">"*"</span></label>
                            <input
                                id="username"
                                class="input"
                                type="text"
                                inputmode="numeric"
                                autocomplete="username"
                                placeholder="请输入学号"
                                on:input=move |ev| set_username.set(event_target_value(&ev))
                            />
                        </div>

                        <div class="field">
                            <label class="field__label" for="password">"密码"<span class="req" aria-hidden="true">"*"</span></label>
                            <input
                                id="password"
                                class="input"
                                type="password"
                                autocomplete="current-password"
                                placeholder="请输入密码（默认身份证后6位）"
                                on:input=move |ev| set_password.set(event_target_value(&ev))
                            />
                        </div>

                        <div class="field">
                            <label class="field__label" for="captcha">"验证码"<span class="req" aria-hidden="true">"*"</span></label>
                            <div class="captcha-row">
                                <input
                                    id="captcha"
                                    class="input"
                                    type="text"
                                    maxlength="4"
                                    autocomplete="off"
                                    placeholder="请输入验证码"
                                    on:input=move |ev| set_captcha.set(event_target_value(&ev))
                                />
                                <div class="captcha-img" role="img" aria-label="验证码图片">
                                    <img src=move || captcha_image_src.get() alt="验证码" />
                                </div>
                                <button
                                    type="button"
                                    class="captcha-refresh"
                                    aria-label="刷新验证码"
                                    title="刷新验证码"
                                    on:click=move |_| handle_get_captcha(())
                                >
                                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-2.64-6.36"/><path d="M21 3v6h-6"/></svg>
                                </button>
                            </div>
                        </div>

                        <p class="status-line" data-kind=move || status_kind(&status_message.get())>
                            {move || status_message.get()}
                        </p>

                        <button class="btn btn--primary btn--block btn--lg" type="submit">"登录"</button>

                        <div class="gh-wrap">
                            <ExternalLink
                                href="https://github.com/Islatri/funky-lesson".to_string()
                                class="gh-link".to_string()
                            >
                                <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 .5A11.5 11.5 0 0 0 .5 12a11.5 11.5 0 0 0 7.86 10.92c.58.1.79-.25.79-.56v-2c-3.2.7-3.88-1.37-3.88-1.37-.53-1.34-1.3-1.7-1.3-1.7-1.06-.72.08-.71.08-.71 1.17.08 1.79 1.2 1.79 1.2 1.04 1.78 2.73 1.27 3.4.97.1-.76.41-1.27.74-1.56-2.55-.29-5.24-1.28-5.24-5.7 0-1.26.45-2.29 1.19-3.1-.12-.29-.52-1.46.11-3.05 0 0 .97-.31 3.18 1.18a11 11 0 0 1 5.8 0c2.2-1.49 3.17-1.18 3.17-1.18.63 1.59.23 2.76.11 3.05.74.81 1.19 1.84 1.19 3.1 0 4.43-2.7 5.4-5.27 5.69.42.36.79 1.07.79 2.16v3.2c0 .31.21.67.8.56A11.5 11.5 0 0 0 23.5 12 11.5 11.5 0 0 0 12 .5Z"/></svg>
                                <span>"在 GitHub 上查看源码"</span>
                            </ExternalLink>
                        </div>
                    </form>
                </div>
            </div>

            // ============================ 选择批次 ============================
            <div class="screen" class:hidden=move || step.get() != 2>
                <div class="appbar">
                    <button class="icon-btn" type="button" on:click=handle_back aria-label="返回">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 12H5"/><path d="m12 19-7-7 7-7"/></svg>
                    </button>
                    <div class="appbar__title">"选择批次"</div>
                    <span></span>
                </div>
                <div class="batch-wrap">
                    <p class="batch-hint">"请选择要参与的选课批次，进入后即可获取课程并开始抢课。"</p>
                    <div class="batch-list">
                        <For
                            each=move || batch_list().into_iter().enumerate()
                            key=|(_idx, batch)| batch.code.clone()
                            children=move |(idx, batch)| {
                                let handle_select = handle_batch_select;
                                let code = batch.code.clone();
                                view! {
                                    <button
                                        class="batch-card"
                                        type="button"
                                        on:click=move |_| handle_select(idx)
                                        disabled=move || is_enrolling.get()
                                    >
                                        <span class="batch-card__icon" aria-hidden="true">
                                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="18" rx="2"/><path d="M16 2v4M8 2v4M3 10h18"/></svg>
                                        </span>
                                        <span class="batch-card__body">
                                            <span class="batch-card__name">{batch.name}</span>
                                            <span class="batch-card__code">"批次代码："<span class="mono">{code}</span></span>
                                        </span>
                                        <span class="batch-card__go" aria-hidden="true">
                                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m9 18 6-6-6-6"/></svg>
                                        </span>
                                    </button>
                                }
                            }
                        />
                    </div>
                </div>
            </div>

            // ============================ 抢课控制台 ============================
            <div class="screen" class:hidden=move || step.get() != 3>
                <div class="appbar">
                    <button class="icon-btn" type="button" on:click=handle_back disabled=move || is_enrolling.get() aria-label="返回">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 12H5"/><path d="m12 19-7-7 7-7"/></svg>
                    </button>
                    <div class="appbar__title">"抢课控制台"</div>
                    <span></span>
                </div>

                <div class="console">
                    <div class="console__inner">

                        // 控制条
                        <div class="card control-bar">
                            <div class="stat">
                                <span class="stat__label">"总请求次数"</span>
                                <span class="stat__value">
                                    {move || app_state.get().enrollment_status.get().total_requests}
                                </span>
                            </div>
                            <span class="status-pill" data-state=move || if is_enrolling.get() { "running" } else { "idle" }>
                                <span class="dot"></span>
                                <span>{move || if is_enrolling.get() { "运行中" } else { "已停止" }}</span>
                            </span>
                            <div class="control-actions">
                                <button
                                    class="btn btn--primary"
                                    type="button"
                                    on:click=handle_enroll
                                    disabled=move || is_enrolling.get()
                                >
                                    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z"/></svg>
                                    "开始抢课"
                                </button>
                                <button
                                    class="btn btn--danger"
                                    type="button"
                                    on:click=handle_stop_enroll
                                    disabled=move || !is_enrolling.get()
                                >
                                    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="6" y="6" width="12" height="12" rx="1.5"/></svg>
                                    "停止抢课"
                                </button>
                            </div>
                        </div>

                        // 脚本兜底入口（A）：打开内嵌官方选课网站，带入登录态
                        <div class="fallback">
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><path d="M12 9v4M12 17h.01"/></svg>
                            <span class="fallback__grow">"脚本抢不到？打开官方选课页手动兜底，登录态自动带入。"</span>
                            <button
                                class="btn btn--ghost btn--sm"
                                type="button"
                                on:click=move |_| open_official_fallback(app_state.get())
                            >"官方选课网站"</button>
                        </div>

                        // 实时状态日志
                        <div class="log" data-live=move || if is_enrolling.get() { "true" } else { "false" }>
                            <div class="log__head">
                                <span class="log__title">"实时状态"<span class="live"><i></i>"LIVE"</span></span>
                            </div>
                            <div class="log__body" role="log" aria-live="polite">
                                <Show
                                    when=move || app_state.get().enrollment_status.get().course_statuses.is_empty()
                                    fallback=move || view! {
                                        <For
                                            each=move || app_state.get().enrollment_status.get().course_statuses
                                            key=|status| status.clone()
                                            children=move |status| {
                                                let kind = log_kind(&status);
                                                let (tag, msg) = split_tag(&status);
                                                view! {
                                                    <div class="log-line" data-kind=kind>
                                                        <span class="tag">{tag}</span>
                                                        <span class="msg">{msg}</span>
                                                    </div>
                                                }
                                            }
                                        />
                                    }
                                >
                                    <div class="log-line log-empty">"尚未开始 — 点击“开始抢课”后，这里会实时显示每门课程的选课状态。"</div>
                                </Show>
                            </div>
                        </div>

                        // 课程列表
                        <div class="course-grid">
                            <div class="card course-card">
                                <div class="course-card__head">
                                    <span class="dot dot--green"></span>
                                    <h4>"已选课程"</h4>
                                    <span class="badge">{move || app_state.get().selected_courses.get().len()}</span>
                                </div>
                                <Show
                                    when=move || app_state.get().selected_courses.get().is_empty()
                                    fallback=move || view! {
                                        <ul class="course-list">
                                            <For
                                                each=move || app_state.get().selected_courses.get()
                                                key=|course| course.JXBID.clone()
                                                children=move |course| {
                                                    view! {
                                                        <li class="course-row">
                                                            <span class="course-row__pip pip--green"></span>
                                                            <div class="course-row__body">
                                                                <div class="course-row__name">{course.KCM}</div>
                                                                <div class="course-row__meta">
                                                                    {format!("教师: {} | ID: ", course.SKJS)}
                                                                    <span class="mono">{course.JXBID}</span>
                                                                </div>
                                                            </div>
                                                        </li>
                                                    }
                                                }
                                            />
                                        </ul>
                                    }
                                >
                                    <div class="course-empty">"暂无已选课程"</div>
                                </Show>
                            </div>

                            <div class="card course-card">
                                <div class="course-card__head">
                                    <span class="dot dot--blue"></span>
                                    <h4>"待选课程（即收藏课程）"</h4>
                                    <span class="badge">{move || app_state.get().favorite_courses.get().len()}</span>
                                    <button
                                        class="btn btn--ghost btn--sm"
                                        type="button"
                                        title="对所有收藏课程各手动选一次（脚本兜底）"
                                        on:click=move |_| {
                                            let st = app_state.get();
                                            for course in st.favorite_courses.get() {
                                                manual_select_once(course, st.clone());
                                            }
                                        }
                                    >"全部各选一次"</button>
                                </div>
                                <Show
                                    when=move || app_state.get().favorite_courses.get().is_empty()
                                    fallback=move || view! {
                                        <ul class="course-list">
                                            <For
                                                each=move || app_state.get().favorite_courses.get()
                                                key=|course| course.JXBID.clone()
                                                children=move |course| {
                                                    let c = course.clone();
                                                    view! {
                                                        <li class="course-row">
                                                            <span class="course-row__pip pip--blue"></span>
                                                            <div class="course-row__body">
                                                                <div class="course-row__name">{course.KCM}</div>
                                                                <div class="course-row__meta">
                                                                    {format!("教师: {} | ID: ", course.SKJS)}
                                                                    <span class="mono">{course.JXBID}</span>
                                                                </div>
                                                            </div>
                                                            <button
                                                                class="btn btn--ghost btn--sm"
                                                                type="button"
                                                                title="用当前登录态手动选一次（脚本兜底）"
                                                                on:click=move |_| manual_select_once(c.clone(), app_state.get())
                                                            >"选一次"</button>
                                                        </li>
                                                    }
                                                }
                                            />
                                        </ul>
                                    }
                                >
                                    <div class="course-empty">"暂无收藏课程，请先到选课网站收藏想抢的课程"</div>
                                </Show>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <ToastContainer />
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_enroll, log_kind, next_pending, split_tag, status_kind};

    #[test]
    fn next_pending_skips_done_and_wraps() {
        assert_eq!(next_pending(0, &[false, false, false]), Some(0));
        assert_eq!(next_pending(1, &[false, false, false]), Some(1));
        assert_eq!(next_pending(0, &[true, false, true]), Some(1)); // 跳过已完成的 0
        assert_eq!(next_pending(2, &[false, true, true]), Some(0)); // 从 2 环绕回 0
        assert_eq!(next_pending(5, &[true, false]), Some(1)); // start 超出长度也能环绕
        assert_eq!(next_pending(0, &[true, true]), None); // 全部完成
        assert_eq!(next_pending(0, &[]), None); // 空
    }

    #[test]
    fn classify_enroll_terminal_and_retry_states() {
        // 成功 / 已选：停止本课程，不致命
        let s = classify_enroll(200, "", false);
        assert_eq!((s.label, s.stop_self, s.fatal), ("选课成功", true, false));
        let s = classify_enroll(500, "该课程已在选课结果中", true);
        assert_eq!((s.label, s.stop_self, s.fatal), ("已选", true, false));

        // 容量已满：try=false 停止本课程标“已满”；try=true 继续“等待中”
        let s = classify_enroll(500, "课容量已满", false);
        assert_eq!((s.label, s.stop_self, s.fatal), ("已满", true, false));
        let s = classify_enroll(500, "课容量已满", true);
        assert_eq!((s.label, s.stop_self, s.fatal), ("等待中", false, false));

        // 暂未开始 / 参数错误 / 未知：继续轮询，非终态、非致命
        let s = classify_enroll(500, "本轮次选课暂未开始", true);
        assert_eq!((s.label, s.stop_self, s.fatal), ("未开始", false, false));
        let s = classify_enroll(500, "参数校验不通过", true);
        assert_eq!((s.label, s.stop_self, s.fatal), ("参数错误", false, false));
        let s = classify_enroll(500, "天降陨石", true);
        assert_eq!((s.label, s.stop_self, s.fatal), ("失败", false, false));

        // 未登录：致命，应停止所有课程
        let s = classify_enroll(401, "", true);
        assert_eq!((s.label, s.stop_self, s.fatal), ("未登录", true, true));
    }

    #[test]
    fn status_kind_maps_login_states() {
        assert_eq!(status_kind("登录成功！"), "success");
        assert_eq!(status_kind("获取课程成功"), "success");
        assert_eq!(status_kind("登录失败：xxx"), "error");
        // 同时含「获取」与「失败」时，错误优先
        assert_eq!(status_kind("获取验证码失败"), "error");
        assert_eq!(status_kind("请输入学号"), "warning");
        assert_eq!(status_kind("请重新登录"), "warning");
        assert_eq!(status_kind("请登录"), "warning");
        assert_eq!(status_kind("正在设置批次..."), "loading");
        assert_eq!(status_kind("获取课程列表"), "loading");
        assert_eq!(status_kind(""), "idle");
    }

    #[test]
    fn log_kind_maps_course_states() {
        assert_eq!(log_kind("[高数]选课成功"), "success");
        assert_eq!(log_kind("[高数]已选"), "success");
        assert_eq!(log_kind("[高数]等待中"), "wait");
        assert_eq!(log_kind("[高数]未开始"), "wait");
        assert_eq!(log_kind("[高数]请求错误"), "error");
        assert_eq!(log_kind("[高数]已满"), "error");
        assert_eq!(log_kind("[高数]未登录"), "error");
        assert_eq!(log_kind("[高数]参数错误"), "error");
        assert_eq!(log_kind("[高数]失败"), "error");
        assert_eq!(log_kind("[高数]进行中"), "info");
    }

    #[test]
    fn split_tag_separates_bracket_prefix() {
        assert_eq!(
            split_tag("[高数]选课成功"),
            ("[高数]".to_string(), "选课成功".to_string())
        );
        assert_eq!(split_tag("[A] msg"), ("[A]".to_string(), "msg".to_string()));
        assert_eq!(split_tag("[空]"), ("[空]".to_string(), String::new()));
        assert_eq!(
            split_tag("no bracket"),
            (String::new(), "no bracket".to_string())
        );
    }
}
