//! What the local API serves.

use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::extract::{Multipart, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::stream::{self, Stream};
use serde_json::{json, Value};
use tauri::Manager;
use tokio::sync::broadcast::error::RecvError;

use super::bus::ApiEvent;
use super::ApiState;
use crate::audio::decode::{self, DecodeError};
use crate::audio::speech;
use crate::dictation::text;
use crate::insertion::focus::FocusTarget;
use crate::processing::ProcessingMode;
use crate::providers::{AudioClip, TranscriptionRequest};
use crate::state::AppState;

/// An error in the OpenAI shape, so existing clients can read it.
pub fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    let kind = if status.is_server_error() {
        "server_error"
    } else {
        "invalid_request_error"
    };
    (
        status,
        Json(json!({ "error": { "message": message, "type": kind, "code": code } })),
    )
        .into_response()
}

pub async fn not_found() -> Response {
    error_response(StatusCode::NOT_FOUND, "not_found", "There is nothing at this path.")
}

pub async fn health(State(api): State<ApiState>) -> Response {
    let state = api.app.state::<AppState>();
    let settings = state.settings();
    Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "state": state.events.current_state(),
        "engine": settings.model_id,
        "mode": settings.mode,
    }))
    .into_response()
}

pub async fn models(State(api): State<ApiState>) -> Response {
    let state = api.app.state::<AppState>();
    // `models()` lists only what is installed, so this is what could run now.
    let data: Vec<Value> = state
        .providers
        .descriptors()
        .into_iter()
        .flat_map(|provider| {
            provider.models.into_iter().map(move |model| {
                json!({ "id": model.id, "object": "model", "owned_by": provider.id })
            })
        })
        .collect();
    Json(json!({ "object": "list", "data": data })).into_response()
}

pub async fn events(State(api): State<ApiState>) -> Response {
    let state = api.app.state::<AppState>();
    if !state.settings().local_api_events {
        return not_found().await;
    }

    let receiver = state.events.subscribe();
    let shutdown = api.shutdown.subscribe();
    // A client that connects mid-dictation should know what is happening now.
    let first = ApiEvent {
        name: "state",
        data: json!({ "state": state.events.current_state(), "ts": crate::database::now_ms() }),
    };

    Sse::new(event_stream(receiver, shutdown, first))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text("keep-alive"))
        .into_response()
}

fn event_stream(
    receiver: tokio::sync::broadcast::Receiver<ApiEvent>,
    shutdown: tokio::sync::watch::Receiver<bool>,
    first: ApiEvent,
) -> impl Stream<Item = Result<Event, Infallible>> {
    stream::unfold(
        (receiver, shutdown, Some(first)),
        |(mut receiver, mut shutdown, mut first)| async move {
            if let Some(event) = first.take() {
                return Some((Ok(to_sse(&event)), (receiver, shutdown, first)));
            }
            loop {
                tokio::select! {
                    _ = shutdown.changed() => return None,
                    message = receiver.recv() => match message {
                        Ok(event) => return Some((Ok(to_sse(&event)), (receiver, shutdown, first))),
                        // Fell behind: skip ahead instead of dropping the client.
                        Err(RecvError::Lagged(_)) => continue,
                        Err(RecvError::Closed) => return None,
                    },
                }
            }
        },
    )
}

fn to_sse(event: &ApiEvent) -> Event {
    Event::default().event(event.name).data(event.data.to_string())
}

/// A file in the uploads directory, removed when dropped.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Releases the "one upload at a time" flag however the request ends.
struct BusyGuard(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct Upload {
    file: Option<(Vec<u8>, Option<String>)>,
    model: Option<String>,
    language: Option<String>,
    response_format: Option<String>,
    mode: Option<String>,
}

enum ReadError {
    TooLarge,
    Malformed(String),
}

async fn read_upload(mut multipart: Multipart) -> Result<Upload, ReadError> {
    let mut upload = Upload::default();

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(error) => return Err(classify(&error)),
        };
        let name = field.name().unwrap_or("").to_string();
        let file_name = field.file_name().map(str::to_string);

        match name.as_str() {
            "file" => {
                let bytes = field.bytes().await.map_err(|e| classify(&e))?;
                upload.file = Some((bytes.to_vec(), file_name));
            }
            "model" | "language" | "response_format" | "mode" => {
                let value = field.text().await.map_err(|e| classify(&e))?.trim().to_string();
                let value = (!value.is_empty()).then_some(value);
                match name.as_str() {
                    "model" => upload.model = value,
                    "language" => upload.language = value,
                    "response_format" => upload.response_format = value,
                    _ => upload.mode = value,
                }
            }
            // Fields this API does not use (`prompt`, `temperature`, …) are
            // accepted and ignored, so OpenAI clients work unchanged.
            _ => {
                let _ = field.bytes().await;
            }
        }
    }

    Ok(upload)
}

fn classify(error: &axum::extract::multipart::MultipartError) -> ReadError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ReadError::TooLarge
    } else {
        ReadError::Malformed(error.body_text())
    }
}

fn too_large() -> Response {
    error_response(
        StatusCode::PAYLOAD_TOO_LARGE,
        "file_too_large",
        &format!("The upload is larger than {} MB.", super::MAX_UPLOAD_BYTES / (1024 * 1024)),
    )
}

pub async fn transcribe(State(api): State<ApiState>, request: axum::extract::Request) -> Response {
    let state = api.app.state::<AppState>();
    let settings = state.settings();

    if !settings.local_api_transcription {
        return not_found().await;
    }

    // Refuse early rather than read a huge body first.
    let declared = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok());
    if declared.is_some_and(|length| length > super::MAX_UPLOAD_BYTES + 64 * 1024) {
        return too_large();
    }

    // The user's own dictation comes first: Whisper handles one thing at a
    // time, and a long upload must never make them wait on it.
    if state.session.state().is_busy() {
        return error_response(
            StatusCode::CONFLICT,
            "busy",
            "Clide is dictating right now. Try again in a moment.",
        );
    }
    if api
        .transcribing
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "busy",
            "Another transcription is already running.",
        );
    }
    let _busy = BusyGuard(api.transcribing.clone());

    let multipart = match <Multipart as axum::extract::FromRequest<()>>::from_request(request, &()).await {
        Ok(multipart) => multipart,
        Err(rejection) => {
            return error_response(
                rejection.status(),
                "invalid_request",
                "Send multipart/form-data with a `file` field.",
            )
        }
    };
    let upload = match read_upload(multipart).await {
        Ok(upload) => upload,
        Err(ReadError::TooLarge) => return too_large(),
        Err(ReadError::Malformed(message)) => {
            return error_response(StatusCode::BAD_REQUEST, "invalid_request", &message)
        }
    };

    let format = upload.response_format.as_deref().unwrap_or("json");
    if !matches!(format, "json" | "text") {
        return error_response(
            StatusCode::BAD_REQUEST,
            "unsupported_response_format",
            "response_format must be `json` or `text`.",
        );
    }
    let as_text = format == "text";

    let mut settings = settings;
    if let Some(mode) = upload.mode.as_deref() {
        settings.mode = match mode {
            "verbatim" => ProcessingMode::Verbatim,
            "polished" => ProcessingMode::Polished,
            "rewrite" => ProcessingMode::Rewrite,
            _ => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    "invalid_mode",
                    "mode must be verbatim, polished or rewrite.",
                )
            }
        };
    }

    let Some((bytes, file_name)) = upload.file else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "missing_file",
            "The `file` field is required.",
        );
    };

    if let Err(error) = decode::check_container(&bytes[..bytes.len().min(16)], file_name.as_deref()) {
        return unsupported(&error);
    }

    // Which engine and model: the one asked for if it is installed, otherwise
    // the one the user has selected.
    let (provider, model) = match upload.model.as_deref() {
        Some(wanted) => {
            let found = state
                .providers
                .descriptors()
                .into_iter()
                .find(|provider| provider.models.iter().any(|model| model.id == wanted))
                .and_then(|provider| state.providers.get(&provider.id));
            match found {
                Some(provider) => (provider, wanted.to_string()),
                None => {
                    return error_response(
                        StatusCode::NOT_FOUND,
                        "model_not_found",
                        &format!("No installed model is called \"{wanted}\"."),
                    )
                }
            }
        }
        None => {
            let provider = state
                .providers
                .get(&settings.provider_id)
                .filter(|provider| provider.has_model(&settings.model_id));
            match provider {
                Some(provider) => (provider, settings.model_id.clone()),
                None => {
                    return error_response(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "no_engine",
                        "No speech engine is installed. Open Clide and choose or download a model.",
                    )
                }
            }
        }
    };

    // Decode off the async workers: it is blocking AVFoundation work.
    let directory = match api.app.path().app_cache_dir() {
        Ok(dir) => dir.join("api-uploads"),
        Err(_) => std::env::temp_dir().join("clide-api-uploads"),
    };
    let decoded = tauri::async_runtime::spawn_blocking(move || prepare(bytes, file_name, directory)).await;
    let (samples, clip_file) = match decoded {
        Ok(Ok(prepared)) => prepared,
        Ok(Err(PrepareError::Decode(error))) => {
            return match error {
                DecodeError::TooLong => error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "audio_too_long",
                    &error.to_string(),
                ),
                other => unsupported(&other),
            }
        }
        Ok(Err(PrepareError::Io(message))) => {
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "io_error", &message)
        }
        Err(_) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "The audio could not be processed.",
            )
        }
    };

    let analysis = speech::analyze(&samples);
    if !analysis.has_speech() {
        return respond("", as_text);
    }

    let seconds = samples.len() as f32 / 16_000.0;
    let request = TranscriptionRequest {
        audio: AudioClip::wav(clip_file.0.clone(), seconds),
        model: model.clone(),
        language: upload.language.or_else(|| settings.language.clone()),
        prompt: crate::context::vocabulary_prompt(
            settings.technical_vocabulary,
            &FocusTarget::default(),
            provider.capabilities().prompting,
        ),
    };

    // No fallback: audio is never handed to a different engine than the one
    // the caller chose or the user selected.
    let transcription = match provider.transcribe(request).await {
        Ok(transcription) => transcription,
        Err(error) => {
            return error_response(StatusCode::BAD_GATEWAY, "engine_error", &error.to_string())
        }
    };
    drop(clip_file);

    if speech::is_phantom_transcript(&transcription.text, analysis.speech) {
        return respond("", as_text);
    }

    match text::finish_text(&state, &transcription.text, &settings, None).await {
        Ok(finished) => respond(&finished.text, as_text),
        Err(failure) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "processing_error",
            &failure.message,
        ),
    }
}

fn unsupported(error: &DecodeError) -> Response {
    error_response(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_audio",
        &error.to_string(),
    )
}

fn respond(text: &str, as_text: bool) -> Response {
    if as_text {
        let mut response = text.to_string().into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        response
    } else {
        Json(json!({ "text": text })).into_response()
    }
}

enum PrepareError {
    Decode(DecodeError),
    Io(String),
}

/// Save the upload, decode it, and leave a 16 kHz WAV for the engine.
fn prepare(
    bytes: Vec<u8>,
    file_name: Option<String>,
    directory: PathBuf,
) -> Result<(Vec<i16>, TempFile), PrepareError> {
    std::fs::create_dir_all(&directory).map_err(|e| PrepareError::Io(e.to_string()))?;

    let id = uuid::Uuid::new_v4();
    let extension = file_name
        .as_deref()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .filter(|extension| extension.len() <= 5 && extension.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or_else(|| "audio".into());

    let upload = TempFile(directory.join(format!("{id}.{extension}")));
    std::fs::write(&upload.0, &bytes).map_err(|e| PrepareError::Io(e.to_string()))?;
    drop(bytes);

    let samples = decode::decode_to_16k_mono(&upload.0).map_err(PrepareError::Decode)?;
    drop(upload);

    let wav = TempFile(directory.join(format!("{id}.wav")));
    decode::write_wav(&wav.0, &samples).map_err(PrepareError::Io)?;
    Ok((samples, wav))
}
