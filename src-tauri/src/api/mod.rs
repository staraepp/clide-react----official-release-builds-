//! The optional local API: other apps on this Mac can transcribe audio with
//! Clide and watch what it is doing.
//!
//! Off by default. When on it listens on 127.0.0.1 only, every request needs
//! the bearer token, and nothing leaves the machine. See `auth` for the checks
//! and `routes` for what is served.

pub mod auth;
pub mod bus;
pub mod routes;
pub mod token;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tokio::sync::watch;

use crate::settings::MIN_API_PORT;
use crate::state::AppState;

/// Largest upload accepted.
pub const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;

/// What every request handler can reach.
#[derive(Clone)]
pub struct ApiState {
    pub app: AppHandle,
    pub policy: auth::Policy,
    /// True while an uploaded file is being transcribed: one at a time.
    pub transcribing: Arc<AtomicBool>,
    /// Flipped when the server stops, to end open event streams.
    pub shutdown: Arc<watch::Sender<bool>>,
}

struct ApiServer {
    port: u16,
    shutdown: Arc<watch::Sender<bool>>,
    task: tauri::async_runtime::JoinHandle<()>,
}

impl Drop for ApiServer {
    fn drop(&mut self) {
        // End open streams, then drop the listener along with the serve task:
        // the port is closed as soon as this returns.
        let _ = self.shutdown.send(true);
        self.task.abort();
    }
}

#[derive(Default)]
pub struct ApiRuntime {
    server: Option<ApiServer>,
    /// Why the server could not start, in words for the settings page.
    last_error: Option<String>,
}

pub type ApiSlot = Mutex<ApiRuntime>;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiStatus {
    pub running: bool,
    pub port: u16,
    pub address: Option<String>,
    pub error: Option<String>,
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/v1/health", get(routes::health))
        .route("/v1/models", get(routes::models))
        .route(
            "/v1/audio/transcriptions",
            post(routes::transcribe).layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES)),
        )
        .route("/v1/events", get(routes::events))
        .fallback(routes::not_found)
        .layer(middleware::from_fn_with_state(state.clone(), auth::guard))
        .with_state(state)
}

/// Bring the server in line with the settings: start it, stop it, or move it
/// to another port. Idempotent; call it after any change that matters.
pub fn apply(app: &AppHandle) {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let mut runtime = state.api.lock().unwrap_or_else(|e| e.into_inner());

    let wanted = settings
        .local_api_enabled
        .then_some(settings.local_api_port);

    match (wanted, runtime.server.as_ref()) {
        (None, None) => {
            runtime.last_error = None;
            return;
        }
        (Some(port), Some(server)) if server.port == port => return,
        _ => {}
    }

    // Stopping first means a port change never has two listeners, and the old
    // port is free the moment this returns.
    runtime.server = None;
    runtime.last_error = None;

    let Some(port) = wanted else {
        tracing::info!("local API stopped");
        return;
    };

    if port < MIN_API_PORT {
        runtime.last_error = Some(format!("Port {port} is reserved. Choose {MIN_API_PORT} or higher."));
        return;
    }

    match start(app, port) {
        Ok(server) => {
            tracing::info!(port, "local API listening on 127.0.0.1");
            runtime.server = Some(server);
        }
        Err(message) => {
            tracing::warn!(port, %message, "local API could not start");
            runtime.last_error = Some(message);
        }
    }
}

fn start(app: &AppHandle, port: u16) -> Result<ApiServer, String> {
    let state = app.state::<AppState>();

    let token = token::load_or_create(&state.db.lock())
        .map_err(|error| format!("The API token could not be read: {error}"))?;

    let listener = bind(port)?;

    // Anything left in the uploads directory is from a run that was cut short.
    if let Ok(directory) = app.path().app_cache_dir() {
        let _ = std::fs::remove_dir_all(directory.join("api-uploads"));
    }

    let (shutdown, _) = watch::channel(false);
    let shutdown = Arc::new(shutdown);

    let api = ApiState {
        app: app.clone(),
        policy: auth::Policy {
            port,
            token: Arc::new(token),
            limiter: Arc::new(auth::RateLimiter::new()),
        },
        transcribing: Arc::new(AtomicBool::new(false)),
        shutdown: Arc::clone(&shutdown),
    };
    let service = router(api);

    let task = tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(error) => {
                tracing::warn!(%error, "local API listener failed");
                return;
            }
        };
        if let Err(error) = axum::serve(listener, service).await {
            tracing::warn!(%error, "local API stopped");
        }
    });

    Ok(ApiServer {
        port,
        shutdown,
        task,
    })
}

/// Listen on 127.0.0.1 only — never 0.0.0.0 — so nothing off this machine can
/// connect. Bound synchronously so a busy port is reported to the settings page
/// instead of vanishing inside a background task.
fn bind(port: u16) -> Result<std::net::TcpListener, String> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", port)).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AddrInUse {
            format!("Port {port} is already in use. Choose another.")
        } else {
            format!("Could not listen on port {port}: {error}")
        }
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Could not listen on port {port}: {error}"))?;
    Ok(listener)
}

/// Stop and start again: used when the token changes, so streams that were
/// opened with the old one end.
pub fn restart(app: &AppHandle) {
    {
        let state = app.state::<AppState>();
        state.api.lock().unwrap_or_else(|e| e.into_inner()).server = None;
    }
    apply(app);
}

pub fn status(app: &AppHandle) -> ApiStatus {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let runtime = state.api.lock().unwrap_or_else(|e| e.into_inner());

    ApiStatus {
        running: runtime.server.is_some(),
        port: settings.local_api_port,
        address: runtime
            .server
            .as_ref()
            .map(|server| format!("http://127.0.0.1:{}", server.port)),
        error: runtime.last_error.clone(),
    }
}

/// Stop the server as the app quits.
pub fn shutdown(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.api.lock().unwrap_or_else(|e| e.into_inner()).server = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_server_listens_on_loopback_only() {
        let listener = bind(0).unwrap();
        let address = listener.local_addr().unwrap();
        assert!(address.ip().is_loopback(), "bound to {address}");
        assert!(!address.ip().is_unspecified());
    }

    #[test]
    fn a_busy_port_is_reported_in_plain_words() {
        let first = bind(0).unwrap();
        let port = first.local_addr().unwrap().port();
        let message = bind(port).unwrap_err();
        assert!(message.contains("already in use"), "{message}");
    }
}
