//! Everything that stands between a request and a route.
//!
//! A server on 127.0.0.1 is still reachable from every web page the user has
//! open, so each request is checked, in this order, and refused on the first
//! failure:
//!
//! 1. `Host` must name this server (defeats DNS rebinding).
//! 2. An `Origin` header means a browser is calling; it is refused unless the
//!    user allowed that origin. Desktop clients send none.
//! 3. A rate limit, applied before the token check so guessing is throttled.
//! 4. The bearer token, compared in constant time.
//!
//! No CORS headers are sent except for an allowed origin. Nothing here logs a
//! header value.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use subtle::ConstantTimeEq;
use tauri::Manager;

use super::routes::error_response;
use super::ApiState;
use crate::state::AppState;

/// Steady requests per second, and how many may arrive at once.
const REFILL_PER_SECOND: f64 = 20.0;
const BURST: f64 = 40.0;

/// A token bucket shared by every client of the server.
pub struct RateLimiter {
    inner: Mutex<(f64, Instant)>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new((BURST, Instant::now())),
        }
    }

    pub fn allow(&self) -> bool {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let elapsed = now.duration_since(guard.1).as_secs_f64();
        guard.0 = (guard.0 + elapsed * REFILL_PER_SECOND).min(BURST);
        guard.1 = now;
        if guard.0 >= 1.0 {
            guard.0 -= 1.0;
            true
        } else {
            false
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Does `host` name this server? Only the loopback names on its own port.
pub fn host_is_allowed(host: &str, port: u16) -> bool {
    let host = host.trim().to_ascii_lowercase();
    host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}")
}

pub fn origin_is_allowed(origin: &str, allowed: &[String]) -> bool {
    allowed
        .iter()
        .any(|entry| entry.trim_end_matches('/').eq_ignore_ascii_case(origin.trim_end_matches('/')))
}

/// Constant-time comparison of a presented `Authorization` header to the token.
pub fn bearer_matches(header: Option<&str>, token: &str) -> bool {
    let Some(presented) = header.and_then(|value| value.strip_prefix("Bearer ")) else {
        return false;
    };
    presented.trim().as_bytes().ct_eq(token.as_bytes()).into()
}

fn header_str(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

/// What a request is judged against.
#[derive(Clone)]
pub struct Policy {
    pub port: u16,
    pub token: Arc<String>,
    pub limiter: Arc<RateLimiter>,
}

pub async fn guard(State(api): State<ApiState>, request: Request, next: Next) -> Response {
    let allowed_origins = api
        .app
        .state::<AppState>()
        .settings()
        .local_api_allowed_origins;
    apply_policy(&api.policy, &allowed_origins, request, next).await
}

/// Judge one request and, if it passes, run it. Separate from `guard` so it can
/// be tested without a running app.
pub async fn apply_policy(
    policy: &Policy,
    allowed_origins: &[String],
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let headers = request.headers().clone();

    let origin = header_str(&headers, header::ORIGIN).map(str::to_string);
    let origin_ok = origin
        .as_deref()
        .is_some_and(|origin| origin_is_allowed(origin, allowed_origins));

    let is_preflight = method == Method::OPTIONS && origin_ok;
    let mut response = match refusal(policy, &headers, origin.as_deref(), origin_ok, is_preflight) {
        Some(refusal) => refusal,
        None if method == Method::OPTIONS => {
            // A browser's preflight for an allowed origin: answer it here, with
            // no route involved.
            StatusCode::NO_CONTENT.into_response()
        }
        None => next.run(request).await,
    };

    let response_headers = response.headers_mut();
    response_headers.insert(
        "X-Clide-Version",
        HeaderValue::from_static(env!("CARGO_PKG_VERSION")),
    );
    if origin_ok {
        if let Some(value) = origin.as_deref().and_then(|o| HeaderValue::from_str(o).ok()) {
            response_headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, value);
            response_headers.insert(header::VARY, HeaderValue::from_static("Origin"));
            response_headers.insert(
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                HeaderValue::from_static("authorization, content-type"),
            );
            response_headers.insert(
                header::ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static("GET, POST, OPTIONS"),
            );
        }
    }

    // Method, path and status only.
    tracing::debug!(%method, path, status = response.status().as_u16(), "api request");
    response
}

/// The response that refuses this request, or `None` if it may proceed.
fn refusal(
    api: &Policy,
    headers: &HeaderMap,
    origin: Option<&str>,
    origin_ok: bool,
    is_preflight: bool,
) -> Option<Response> {
    let host = header_str(headers, header::HOST).unwrap_or("");
    if !host_is_allowed(host, api.port) {
        return Some(error_response(
            StatusCode::FORBIDDEN,
            "forbidden_host",
            "This server only answers to 127.0.0.1 or localhost.",
        ));
    }

    if origin.is_some() && !origin_ok {
        return Some(error_response(
            StatusCode::FORBIDDEN,
            "forbidden_origin",
            "Requests from web pages are not allowed unless their origin is on the allow-list in Clide's settings.",
        ));
    }

    if !api.limiter.allow() {
        return Some(error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many requests. Slow down.",
        ));
    }

    // A browser's preflight cannot carry the token; it carries nothing but an
    // allowed origin's question, which has already been answered above.
    if is_preflight {
        return None;
    }

    if !bearer_matches(header_str(headers, header::AUTHORIZATION), &api.token) {
        let mut response = error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "A valid bearer token is required.",
        );
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer realm=\"clide\""),
        );
        return Some(response);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_names_on_our_port_are_allowed_hosts() {
        assert!(host_is_allowed("127.0.0.1:47815", 47815));
        assert!(host_is_allowed("localhost:47815", 47815));
        assert!(host_is_allowed("LOCALHOST:47815", 47815));
        assert!(!host_is_allowed("evil.com", 47815));
        assert!(!host_is_allowed("evil.com:47815", 47815));
        assert!(!host_is_allowed("127.0.0.1:9999", 47815));
        assert!(!host_is_allowed("127.0.0.1", 47815));
        assert!(!host_is_allowed("", 47815));
        // A rebinding name that merely starts like ours.
        assert!(!host_is_allowed("127.0.0.1:47815.evil.com", 47815));
    }

    #[test]
    fn origins_must_be_listed_exactly() {
        let allowed = vec!["http://localhost:3000".to_string()];
        assert!(origin_is_allowed("http://localhost:3000", &allowed));
        assert!(origin_is_allowed("http://localhost:3000/", &allowed));
        assert!(!origin_is_allowed("http://localhost:3001", &allowed));
        assert!(!origin_is_allowed("http://evil.com", &allowed));
        assert!(!origin_is_allowed("http://evil.com", &[]));
    }

    #[test]
    fn the_bearer_token_must_match_exactly() {
        let token = "a".repeat(64);
        assert!(bearer_matches(Some(&format!("Bearer {token}")), &token));
        assert!(!bearer_matches(Some(&format!("Bearer {}", "b".repeat(64))), &token));
        assert!(!bearer_matches(Some("Bearer short"), &token));
        assert!(!bearer_matches(Some(&token), &token), "the scheme is required");
        assert!(!bearer_matches(Some("Basic abc"), &token));
        assert!(!bearer_matches(None, &token));
    }

    #[test]
    fn the_rate_limiter_allows_a_burst_then_refuses() {
        let limiter = RateLimiter::new();
        let allowed = (0..200).filter(|_| limiter.allow()).count();
        assert!((40..=45).contains(&allowed), "allowed {allowed}");
    }

    // --- the whole middleware, through a real router --------------------------

    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn app(allowed: Vec<String>) -> Router {
        let policy = Policy {
            port: 47815,
            token: Arc::new(TOKEN.to_string()),
            limiter: Arc::new(RateLimiter::new()),
        };
        Router::new()
            .route("/v1/health", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(move |request: Request, next: Next| {
                let policy = policy.clone();
                let allowed = allowed.clone();
                async move { apply_policy(&policy, &allowed, request, next).await }
            }))
    }

    async fn call(router: &Router, build: impl FnOnce(axum::http::request::Builder) -> axum::http::request::Builder) -> Response {
        let builder = HttpRequest::builder().uri("/v1/health");
        router
            .clone()
            .oneshot(build(builder).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    fn good(builder: axum::http::request::Builder) -> axum::http::request::Builder {
        builder
            .header("host", "127.0.0.1:47815")
            .header("authorization", format!("Bearer {TOKEN}"))
    }

    #[tokio::test]
    async fn a_correct_request_passes_and_carries_the_version() {
        let response = call(&app(vec![]), good).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("x-clide-version").unwrap(),
            env!("CARGO_PKG_VERSION")
        );
        assert!(response.headers().get("access-control-allow-origin").is_none());
    }

    #[tokio::test]
    async fn no_token_and_a_wrong_token_are_refused() {
        let router = app(vec![]);
        let missing = call(&router, |b| b.header("host", "127.0.0.1:47815")).await;
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
        assert!(missing.headers().contains_key("www-authenticate"));
        assert!(missing.headers().contains_key("x-clide-version"));

        let wrong = call(&router, |b| {
            b.header("host", "127.0.0.1:47815")
                .header("authorization", format!("Bearer {}", "0".repeat(64)))
        })
        .await;
        assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn a_foreign_host_is_refused_even_with_the_right_token() {
        let response = call(&app(vec![]), |b| {
            b.header("host", "evil.com:47815")
                .header("authorization", format!("Bearer {TOKEN}"))
        })
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn an_unlisted_origin_is_refused_even_with_the_right_token() {
        let response = call(&app(vec![]), |b| good(b).header("origin", "http://evil.com")).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(response.headers().get("access-control-allow-origin").is_none());
    }

    #[tokio::test]
    async fn a_listed_origin_passes_and_gets_cors_for_itself_only() {
        let router = app(vec!["http://localhost:3000".into()]);
        let response = call(&router, |b| good(b).header("origin", "http://localhost:3000")).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("access-control-allow-origin").unwrap(),
            "http://localhost:3000"
        );
    }

    #[tokio::test]
    async fn a_preflight_from_a_listed_origin_needs_no_token() {
        let router = app(vec!["http://localhost:3000".into()]);
        let response = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("OPTIONS")
                    .uri("/v1/health")
                    .header("host", "localhost:47815")
                    .header("origin", "http://localhost:3000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        // From an unlisted origin the same preflight is refused.
        let refused = router
            .oneshot(
                HttpRequest::builder()
                    .method("OPTIONS")
                    .uri("/v1/health")
                    .header("host", "localhost:47815")
                    .header("origin", "http://evil.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_flood_is_rate_limited() {
        let router = app(vec![]);
        let mut limited = 0;
        for _ in 0..120 {
            if call(&router, good).await.status() == StatusCode::TOO_MANY_REQUESTS {
                limited += 1;
            }
        }
        assert!(limited > 0, "nothing was rate limited");
    }
}
