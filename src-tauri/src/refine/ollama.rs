//! Text refinement through a local Ollama server.
//!
//! Ollama runs models on this Mac and listens only on `127.0.0.1`, so the
//! transcript never leaves the machine — this is the same "100% local" bar as
//! Apple Intelligence, with a choice of model. Clide does not install or
//! manage Ollama; it uses whatever the user already has pulled.
//!
//! Availability is a quick local socket check, repeated before every use,
//! because the user can quit Ollama at any time.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::traits::{strip_wrapping_quotes, RefineError, RefineRequest, Refiner};

const ENGINE_ID: &str = "ollama";
const ADDRESS: &str = "127.0.0.1:11434";
const CONNECT_TIMEOUT: Duration = Duration::from_millis(300);
const READ_TIMEOUT: Duration = Duration::from_secs(2);
/// Cleaning up a dictation should take a second or two; past this the user is
/// better served by the polished transcript than by waiting.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Keeps the model resident between dictations so each one does not pay the
/// load.
const KEEP_ALIVE: &str = "30m";

/// Preferred models, best first, matched as substrings of the model name.
const PREFERRED: &[&str] = &["qwen3", "llama3", "gemma", "phi", "mistral"];

pub struct OllamaRefiner {
    client: reqwest::Client,
}

impl OllamaRefiner {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            // Loopback only. A system proxy must never see a transcript.
            .no_proxy()
            .build()
            .unwrap_or_default();
        Self { client }
    }
}

impl Default for OllamaRefiner {
    fn default() -> Self {
        Self::new()
    }
}

/// The models Ollama has pulled, or `None` when it is not reachable.
///
/// A deliberately tiny HTTP/1.1 GET over a loopback socket: it keeps the check
/// synchronous (the descriptor and availability paths are) without pulling in
/// a blocking HTTP client.
fn installed_models() -> Option<Vec<String>> {
    let address: SocketAddr = ADDRESS.parse().ok()?;
    let mut stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT).ok()?;
    stream.set_read_timeout(Some(READ_TIMEOUT)).ok()?;
    stream
        .write_all(b"GET /api/tags HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .ok()?;

    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    parse_models(&response)
}

/// Pull model names out of a raw `/api/tags` response.
fn parse_models(response: &str) -> Option<Vec<String>> {
    // Tolerates chunked framing: only the JSON object itself is parsed.
    let start = response.find('{')?;
    let end = response.rfind('}')?;
    let body: Value = serde_json::from_str(response.get(start..=end)?).ok()?;

    let names = body
        .get("models")?
        .as_array()?
        .iter()
        .filter_map(|model| model.get("name")?.as_str())
        // Embedding models cannot rewrite text.
        .filter(|name| !name.to_lowercase().contains("embed"))
        .map(str::to_string)
        .collect();
    Some(names)
}

/// The model to use: the one the user chose if it is installed, otherwise the
/// best installed match for dictation cleanup.
pub fn pick_model(requested: Option<&str>, installed: &[String]) -> Option<String> {
    if let Some(requested) = requested {
        if installed.iter().any(|name| name == requested) {
            return Some(requested.to_string());
        }
    }

    PREFERRED
        .iter()
        .find_map(|family| {
            // Instruct builds follow "reply with only the text" far better
            // than reasoning builds, so they win within a family.
            installed
                .iter()
                .find(|name| name.contains(family) && name.contains("instruct"))
                .or_else(|| installed.iter().find(|name| name.contains(family)))
        })
        .or_else(|| installed.first())
        .cloned()
}

/// Remove a `<think>…</think>` block some reasoning models prepend.
fn strip_thinking(text: &str) -> &str {
    match text.find("</think>") {
        Some(end) => text[end + "</think>".len()..].trim(),
        None => text.trim(),
    }
}

#[async_trait]
impl Refiner for OllamaRefiner {
    fn id(&self) -> &'static str {
        ENGINE_ID
    }

    fn name(&self) -> &'static str {
        "Local model (Ollama)"
    }

    fn description(&self) -> &'static str {
        "A model running in Ollama on this Mac. Nothing leaves it."
    }

    fn local(&self) -> bool {
        true
    }

    fn availability(&self) -> Result<(), RefineError> {
        match installed_models() {
            Some(models) if !models.is_empty() => Ok(()),
            _ => Err(RefineError::Unavailable { engine: ENGINE_ID }),
        }
    }

    fn models(&self) -> Vec<String> {
        installed_models().unwrap_or_default()
    }

    async fn refine(&self, request: RefineRequest) -> Result<String, RefineError> {
        let failed = |detail: String| RefineError::Failed {
            engine: ENGINE_ID,
            detail,
        };

        let installed = tauri::async_runtime::spawn_blocking(installed_models)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        let model = pick_model(request.model.as_deref(), &installed)
            .ok_or(RefineError::Unavailable { engine: ENGINE_ID })?;

        let words = request.text.split_whitespace().count();
        let body = json!({
            "model": model,
            "stream": false,
            // Ignored by models that do not reason; stops ones that do from
            // spending seconds thinking about a tidy-up.
            "think": false,
            "keep_alive": KEEP_ALIVE,
            "options": {
                "temperature": 0,
                "num_predict": (words * 3).clamp(128, 2048),
            },
            "messages": [
                { "role": "system", "content": request.style.instruction() },
                { "role": "user", "content": request.prompt() },
            ],
        });

        let response = self
            .client
            .post(format!("http://{ADDRESS}/api/chat"))
            .json(&body)
            .send()
            .await
            .map_err(|error| failed(error.to_string()))?;

        if !response.status().is_success() {
            return Err(failed(format!("Ollama answered {}", response.status())));
        }

        let payload: Value = response
            .json()
            .await
            .map_err(|error| failed(error.to_string()))?;
        let content = payload
            .pointer("/message/content")
            .and_then(Value::as_str)
            .unwrap_or_default();

        let refined = strip_wrapping_quotes(strip_thinking(content)).to_string();
        if refined.is_empty() {
            return Err(RefineError::Declined {
                engine: ENGINE_ID,
                detail: "the model returned nothing".into(),
            });
        }

        Ok(refined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn model_names_are_read_from_a_raw_response() {
        let raw = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n\
                   {\"models\":[{\"name\":\"qwen3:4b\"},{\"name\":\"nomic-embed-text\"}]}";
        assert_eq!(parse_models(raw), Some(names(&["qwen3:4b"])));
    }

    #[test]
    fn chunked_framing_does_not_break_parsing() {
        let raw = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1f\r\n\
                   {\"models\":[{\"name\":\"llama3.2\"}]}\r\n0\r\n\r\n";
        assert_eq!(parse_models(raw), Some(names(&["llama3.2"])));
    }

    #[test]
    fn a_garbled_response_is_unavailable_not_a_panic() {
        assert_eq!(parse_models("nonsense"), None);
        assert_eq!(parse_models("{\"error\":\"x\"}"), None);
    }

    #[test]
    fn the_users_choice_wins_when_it_is_installed() {
        let installed = names(&["qwen3:4b-instruct", "llama3.2"]);
        assert_eq!(
            pick_model(Some("llama3.2"), &installed).as_deref(),
            Some("llama3.2")
        );
    }

    #[test]
    fn a_missing_choice_falls_back_to_the_best_match() {
        let installed = names(&["gemma4:e4b", "qwen3:4b", "qwen3:4b-instruct-2507"]);
        assert_eq!(
            pick_model(Some("gone"), &installed).as_deref(),
            Some("qwen3:4b-instruct-2507")
        );
        assert_eq!(
            pick_model(None, &names(&["something-else"])).as_deref(),
            Some("something-else")
        );
        assert_eq!(pick_model(None, &[]), None);
    }

    #[test]
    fn a_reasoning_block_is_removed() {
        assert_eq!(strip_thinking("<think>hmm</think>\nHello."), "Hello.");
        assert_eq!(strip_thinking("Hello."), "Hello.");
    }
}
