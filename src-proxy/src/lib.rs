use actix_cors::Cors;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, web};
use log::{debug, error, info, warn};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

#[actix_web::main]
pub async fn main() -> std::io::Result<()> {
    // 默认 info：debug 级别会打印含密码/令牌的请求体与响应，需显式 RUST_LOG=debug 才开启
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    info!("Starting universal proxy server at http://127.0.0.1:3030");

    // 复用单个 Client（reqwest 内部是 Arc + 连接池），避免每个请求都新建
    // TCP/TLS 连接；并加上连接/请求超时，防止上游卡死时 worker 无限挂起。
    // danger_accept_invalid_certs 的原因见 proxy_handler 处注释。
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap_or_default();
    let client = web::Data::new(client);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://tauri.localhost")
            .allowed_origin("http://localhost:1420")
            .allowed_origin("http://127.0.0.1:1420")
            // .allowed_origin("file://")
            .allowed_methods(vec!["GET", "POST", "OPTIONS"])
            .allowed_headers(vec![
                "Authorization",
                "Content-Type",
                "Accept",
                "Origin",
                "X-Requested-With",
                "batchId",
            ])
            .supports_credentials()
            .max_age(3600);

        App::new().app_data(client.clone()).wrap(cors).service(
            web::resource("/api/proxy/{endpoint:.*}")
                .route(web::post().to(proxy_handler))
                .route(web::get().to(proxy_handler_get)),
        )
    })
    .bind("127.0.0.1:3030")?
    .run()
    .await
}

#[derive(Debug, Deserialize)]
struct ProxyRequest {
    original_url: String,
    #[serde(default)]
    batch_id: Option<String>,
    #[serde(default)]
    class_type: Option<String>,
    #[serde(default)]
    class_id: Option<String>,
    #[serde(default)]
    secret_val: Option<String>,
    #[serde(default)]
    params: Option<HashMap<String, String>>,
    // 新增登录相关字段
    #[serde(default)]
    loginname: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    captcha: Option<String>,
    #[serde(default)]
    uuid: Option<String>,
}

/// SSRF 白名单：仅允许把请求转发到 https 协议、主机为 icourses.jlu.edu.cn 的上游。
/// CORS 拦不住这条路径（curl/原生客户端不受 CORS 约束），必须在服务端校验。
fn is_allowed_upstream(url: &str) -> bool {
    match reqwest::Url::parse(url) {
        Ok(u) => u.scheme() == "https" && u.host_str() == Some("icourses.jlu.edu.cn"),
        Err(_) => false,
    }
}

async fn proxy_handler_get(
    req: HttpRequest,
    path: web::Path<String>,
    client: web::Data<reqwest::Client>,
) -> HttpResponse {
    let endpoint = path.into_inner();
    let params = req.query_string();
    debug!("Handling GET proxy request for endpoint: {endpoint}");
    debug!("Query params: {params}");

    // 获取原始URL
    let original_url = match endpoint.as_str() {
        "profile/index.html" => "https://icourses.jlu.edu.cn/xsxk/profile/index.html",
        "elective/grablessons" => {
            &format!("https://icourses.jlu.edu.cn/xsxk/elective/grablessons?{params}")
        }
        _ => {
            return HttpResponse::BadRequest().json(json!({
                "error": "Invalid endpoint for GET request"
            }));
        }
    };

    let auth_token = match req.headers().get(actix_web::http::header::AUTHORIZATION) {
        Some(token) => match token.to_str() {
            Ok(t) => Some(t.to_string()),
            Err(e) => {
                error!("Invalid authorization token: {e}");
                return HttpResponse::BadRequest().json(json!({
                    "error": "Invalid authorization token"
                }));
            }
        },
        None => None,
    };

    let mut headers = HeaderMap::new();
    if let Some(token) = auth_token {
        match HeaderValue::from_str(&token) {
            Ok(v) => {
                headers.insert(AUTHORIZATION, v);
            }
            Err(e) => {
                error!("Invalid authorization header value: {e}");
                return HttpResponse::BadRequest()
                    .json(json!({ "error": "Invalid authorization header" }));
            }
        }
    }

    debug!("Sending GET request to: {original_url}");

    match client.get(original_url).headers(headers).send().await {
        Ok(response) => {
            let status = response.status();
            debug!("Received response with status: {status}");

            match response.text().await {
                Ok(text) => HttpResponse::Ok().content_type("text/html").body(text),
                Err(e) => {
                    error!("Failed to get response text: {e}");
                    HttpResponse::InternalServerError().json(json!({
                        "error": format!("Failed to read response: {}", e)
                    }))
                }
            }
        }
        Err(e) => {
            error!("Request failed: {e}");
            HttpResponse::InternalServerError().json(json!({
                "error": format!("Request failed: {}", e)
            }))
        }
    }
}

async fn proxy_handler(
    body: web::Json<ProxyRequest>,
    req: HttpRequest,
    path: web::Path<String>,
    client: web::Data<reqwest::Client>,
) -> HttpResponse {
    let endpoint = path.into_inner();
    debug!("Handling proxy request for endpoint: {endpoint}");

    // SSRF 防护：只允许转发到吉大选课服务器。合法前端只请求 icourses.jlu.edu.cn，
    // 这里拒绝任何其它目标，避免本地代理被当作任意 URL 的开放转发器。
    if !is_allowed_upstream(&body.original_url) {
        warn!("Rejected non-allowlisted upstream URL");
        return HttpResponse::Forbidden().json(json!({ "error": "upstream host not allowed" }));
    }

    // 注：共享 Client（在 main 构建）关闭了证书校验——服务器证书本身有效
    // （DigiCert，*.jlu.edu.cn），但只下发叶子证书、不附带 RapidSSL 中间证书；
    // rustls 不会自动补链（webpki 根 + 无 AIA），直接开启校验会导致登录中断。
    // 需配合“固定中间证书为附加根”或平台校验器并真机联调后才能移除（见后续建议）。

    let auth_token = match req.headers().get(actix_web::http::header::AUTHORIZATION) {
        Some(token) => match token.to_str() {
            Ok(t) => Some(t.to_string()),
            Err(e) => {
                error!("Invalid authorization token: {e}");
                return HttpResponse::BadRequest().json(json!({
                    "error": "Invalid authorization token"
                }));
            }
        },
        None => None,
    };

    let keep_alive = match req.headers().get("Connection") {
        Some(token) => match token.to_str() {
            Ok(t) => t.eq_ignore_ascii_case("keep-alive"),
            Err(e) => {
                error!("Invalid Connection token: {e}");
                false
            }
        },
        None => false,
    };

    let batch_id = match req.headers().get("BatchId") {
        Some(token) => match token.to_str() {
            Ok(t) => Some(t.to_string()),
            Err(e) => {
                error!("Invalid BatchId token: {e}");
                return HttpResponse::BadRequest().json(json!({
                    "error": "Invalid BatchId token"
                }));
            }
        },
        None => None,
    };

    let mut headers = HeaderMap::new();
    // 配置常用headers
    headers.insert("User-Agent", HeaderValue::from_str("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/139.0.0.0 Safari/537.36").unwrap());
    // headers.insert("Accept-Encoding", HeaderValue::from_str("gzip, deflate, br").unwrap());
    // headers.insert("Accept", HeaderValue::from_str("*/*").unwrap());
    // headers.insert("Content-Type", HeaderValue::from_str("application/json, text/plain, */*").unwrap());
    // headers.insert("Content-Length", HeaderValue::from_str("0").unwrap());
    // headers.insert("Connection", HeaderValue::from_str("keep-alive").unwrap());
    // headers.insert("Host", HeaderValue::from_str("icourses.jlu.edu.cn").unwrap());
    // headers.insert("host", HeaderValue::from_str("icourses.jlu.edu.cn").unwrap());

    if let Some(token) = auth_token {
        match HeaderValue::from_str(&token) {
            Ok(v) => {
                headers.insert(AUTHORIZATION, v);
            }
            Err(e) => {
                error!("Invalid authorization header value: {e}");
                return HttpResponse::BadRequest()
                    .json(json!({ "error": "Invalid authorization header" }));
            }
        }
    }
    if keep_alive {
        headers.insert("Connection", HeaderValue::from_str("keep-alive").unwrap());
    }
    if body.original_url.contains("icourses.jlu.edu.cn") {
        headers.insert(
            "Origin",
            HeaderValue::from_str("https://icourses.jlu.edu.cn").unwrap(),
        );
    }
    if body.original_url.contains("xsxk/sc/clazz/list")
        && let Some(batch_id) = batch_id
    {
        let referer =
            format!("https://icourses.jlu.edu.cn/xsxk/profile/index.html?batchId={batch_id}");
        match HeaderValue::from_str(&referer) {
            Ok(v) => {
                headers.insert("Referer", v);
            }
            Err(e) => warn!("Skipping invalid Referer header: {e}"),
        }
    }

    // 构建请求体
    let mut request_body = HashMap::new();
    if body.original_url.contains("xsxk/elective/user")
        && let Some(batch_id) = &body.batch_id
    {
        request_body.insert("batchId", batch_id);
    }
    if let Some(loginname) = &body.loginname {
        request_body.insert("loginname", loginname);
    }
    if let Some(password) = &body.password {
        request_body.insert("password", password);
    }
    if let Some(captcha) = &body.captcha {
        request_body.insert("captcha", captcha);
    }
    if let Some(uuid) = &body.uuid {
        request_body.insert("uuid", uuid);
    }

    // 构建查询参数
    let mut query_params = HashMap::new();
    // if let Some(batch_id) = &body.batch_id {
    //     query_params.insert("batchId", batch_id);
    // }
    if let Some(class_type) = &body.class_type {
        query_params.insert("clazzType", class_type);
    }
    if let Some(class_id) = &body.class_id {
        query_params.insert("clazzId", class_id);
    }
    if let Some(secret_val) = &body.secret_val {
        query_params.insert("secretVal", secret_val);
    }
    if let Some(extra_params) = &body.params {
        query_params.extend(extra_params.iter().map(|(k, v)| (k.as_str(), v)));
    }

    debug!("Sending request to: {}", body.original_url);
    debug!("Query params: {query_params:?}");
    // 不记录 headers（含 Authorization）与 request_body（含登录密码），避免凭据落入日志

    let mut request = client.post(&body.original_url).headers(headers);

    // 添加请求体或查询参数
    if !request_body.is_empty() {
        // 应当以 urlencoded 形式发送
        match serde_urlencoded::to_string(&request_body) {
            Ok(encoded) => {
                request = request
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .body(encoded);
            }
            Err(e) => {
                error!("Failed to encode request body: {e}");
                return HttpResponse::InternalServerError()
                    .json(json!({ "error": "Failed to encode request body" }));
            }
        }
    }
    if !query_params.is_empty() {
        request = request.query(&query_params);
    }

    match request.send().await {
        Ok(response) => {
            let status = response.status();
            debug!("Received response with status: {status}");

            match response.text().await {
                Ok(text) => {
                    // 不记录响应正文（登录响应含会话 token）
                    debug!("Received {} bytes from upstream", text.len());
                    match serde_json::from_str::<serde_json::Value>(&text) {
                        Ok(json_value) => HttpResponse::Ok()
                            .content_type("application/json")
                            .json(json_value),
                        Err(e) => {
                            warn!("Failed to parse response as JSON: {e}, returning raw text");
                            HttpResponse::Ok().content_type("text/plain").body(text)
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to get response text: {e}");
                    HttpResponse::InternalServerError().json(json!({
                        "error": format!("Failed to read response: {}", e)
                    }))
                }
            }
        }
        Err(e) => {
            error!("Request failed: {e}");
            HttpResponse::InternalServerError().json(json!({
                "error": format!("Request failed: {}", e)
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_allowed_upstream;

    #[test]
    fn allows_only_https_jlu_host() {
        // 合法目标
        assert!(is_allowed_upstream(
            "https://icourses.jlu.edu.cn/xsxk/elective/clazz/add"
        ));
        assert!(is_allowed_upstream(
            "https://icourses.jlu.edu.cn/xsxk/auth/login"
        ));

        // 非法目标
        assert!(!is_allowed_upstream("http://icourses.jlu.edu.cn/x")); // 非 https
        assert!(!is_allowed_upstream("https://evil.com/")); // 其它主机
        assert!(!is_allowed_upstream(
            "https://icourses.jlu.edu.cn.evil.com/"
        )); // 后缀伪装
        assert!(!is_allowed_upstream(
            "https://user@icourses.jlu.edu.cn.evil.com/"
        )); // userinfo 伪装
        assert!(!is_allowed_upstream("file:///etc/passwd"));
        assert!(!is_allowed_upstream("not a url"));
    }
}
