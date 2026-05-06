use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use anyhow::Context;
use cgx_core::GraphDb;
use futures::stream::{self, Stream, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncBufReadExt;
use tokio_util::io::StreamReader;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub history: Vec<ChatMessage>,
    #[serde(default)]
    pub selected_node: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Clone)]
struct SourceNode {
    id: String,
    name: String,
    kind: String,
    path: String,
    churn: f64,
    community: i64,
}

type SseStream = Pin<Box<dyn Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>> + Send>>;

/// POST /api/chat handler — streams LLM output as SSE
pub async fn chat_stream(
    repo_path: PathBuf,
    body: ChatRequest,
) -> axum::response::Sse<SseStream>
{
    let db = match GraphDb::open(&repo_path) {
        Ok(db) => db,
        Err(e) => {
            let err = serde_json::json!({"type":"error","message":e.to_string()}).to_string();
            let stream: SseStream = Box::pin(stream::once(async move {
                Ok(axum::response::sse::Event::default().data(err))
            }));
            return axum::response::Sse::new(stream);
        }
    };

    // Load config (falls back to defaults if no config file exists)
    let config = cgx_core::CgxConfig::load(&repo_path).unwrap_or_default();

    // Build graph context
    let (context, sources) = build_graph_context(&db, &body.message, body.selected_node.as_deref());

    // Build full prompt
    let history_text = body.history.iter()
        .fold(String::new(), |mut acc, m| {
            let role = if m.role == "assistant" { "Assistant" } else { "Human" };
            acc.push_str(&format!("{}: {}\n", role, m.content));
            acc
        });

    let prompt = format!(
        "{}\n---\n{}User question: {}\n\nAnswer concisely using the codebase information above. For code references, use backtick-quoted `file_path:line_number` format.",
        context, history_text, body.message
    );

    match stream_llm_response(&config.chat, &prompt, &sources).await {
        Ok(stream) => axum::response::Sse::new(stream),
        Err(e) => {
            let err = serde_json::json!({
                "type": "error",
                "message": format!("Chat error: {}", e)
            }).to_string();
            let s: SseStream = Box::pin(stream::once(async move {
                Ok(axum::response::sse::Event::default().data(err))
            }));
            axum::response::Sse::new(s)
        }
    }
}

/// Build context from DuckDB graph data
fn build_graph_context(db: &GraphDb, message: &str, selected_node: Option<&str>) -> (String, Vec<SourceNode>) {
    let mut context = String::from(
        "You are a codebase expert assistant. Below is structured information about the codebase.\n\n"
    );

    // Repo overview
    let node_count = db.node_count().unwrap_or(0);
    let edge_count = db.edge_count().unwrap_or(0);
    let languages = db.get_language_breakdown().unwrap_or_default();

    let lang_summary = if languages.is_empty() {
        "unknown".to_string()
    } else {
        let mut entries: Vec<_> = languages.iter().collect();
        entries.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
        entries.iter().take(3)
            .map(|(l, p)| format!("{} {:.0}%", l, *p * 100.0))
            .collect::<Vec<_>>().join(", ")
    };

    context.push_str(&format!(
        "REPOSITORY OVERVIEW\nNodes: {}  Edges: {}  Languages: {}\n\n",
        node_count, edge_count, lang_summary
    ));

    // Top communities
    if let Ok(communities) = db.get_communities() {
        if !communities.is_empty() {
            context.push_str("TOP COMMUNITIES\n");
            for (id, label, count, top_nodes) in communities.iter().take(5) {
                let tops = top_nodes.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
                context.push_str(&format!("  #{id} {label} — {count} nodes (top: {tops})\n"));
            }
            context.push('\n');
        }
    }

    // Hotspots
    if let Ok(hotspots) = db.get_hotspots(5) {
        if !hotspots.is_empty() {
            context.push_str("TOP HOTSPOTS (high churn × coupling)\n");
            for (path, churn, coupling, callers) in &hotspots {
                context.push_str(&format!(
                    "  {} churn={:.2} coupling={:.2} callers={}\n",
                    path, churn, coupling, callers
                ));
            }
            context.push('\n');
        }
    }

    // Extract keywords and search graph
    let keywords = extract_keywords(message);
    let all_nodes = db.get_all_nodes().unwrap_or_default();

    let mut seen_ids = std::collections::HashSet::new();
    let mut relevant_nodes: Vec<SourceNode> = Vec::new();

    for kw in &keywords {
        for node in &all_nodes {
            if (node.name.to_lowercase().contains(kw) || node.id.to_lowercase().contains(kw))
                && seen_ids.insert(node.id.clone())
            {
                relevant_nodes.push(SourceNode {
                    id: node.id.clone(),
                    name: node.name.clone(),
                    kind: node.kind.clone(),
                    path: node.path.clone(),
                    churn: node.churn,
                    community: node.community,
                });
            }
            if relevant_nodes.len() >= 15 {
                break;
            }
        }
        if relevant_nodes.len() >= 15 {
            break;
        }
    }

    // Include selected node's neighbors
    if let Some(node_id) = selected_node {
        if let Ok(neighbors) = db.get_neighbors(node_id, 1) {
            for n in &neighbors {
                if seen_ids.insert(n.id.clone()) && relevant_nodes.len() < 20 {
                    relevant_nodes.push(SourceNode {
                        id: n.id.clone(),
                        name: n.name.clone(),
                        kind: n.kind.clone(),
                        path: n.path.clone(),
                        churn: n.churn,
                        community: n.community,
                    });
                }
            }
        }
    }

    if !relevant_nodes.is_empty() {
        context.push_str("RELEVANT NODES\n");
        for n in &relevant_nodes {
            context.push_str(&format!(
                "  {} [{}] churn={:.2} community=#{}\n",
                n.name, n.kind, n.churn, n.community
            ));
            if !n.path.is_empty() {
                context.push_str(&format!("    path: {}\n", n.path));
            }
        }
        context.push('\n');
    }

    let mut sources = relevant_nodes;
    sources.sort_by(|a, b| b.churn.partial_cmp(&a.churn).unwrap_or(std::cmp::Ordering::Equal));

    (context, sources)
}

/// Simple keyword extraction from user message
fn extract_keywords(message: &str) -> Vec<String> {
    let stop_words: &[&str] = &[
        "the", "is", "are", "a", "an", "what", "who", "how", "where", "why",
        "when", "can", "does", "do", "did", "was", "were", "be", "has", "have",
        "had", "will", "would", "could", "should", "may", "might", "this", "that",
        "these", "those", "it", "its", "in", "on", "at", "to", "for", "of", "with",
        "about", "from", "by", "or", "and", "not", "but", "if", "then", "else",
        "tell", "show", "find", "get", "give", "make", "explain", "describe",
        "help", "me", "my", "our", "your", "their", "codebase", "file",
        "files", "code", "any", "some", "all", "just", "like",
    ];
    let words: Vec<String> = message
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1)
        .map(|w| w.to_lowercase())
        .filter(|w| !stop_words.contains(&w.as_str()))
        .collect();

    let mut seen = std::collections::HashSet::new();
    let mut unique: Vec<String> = Vec::new();
    for w in words {
        if seen.insert(w.clone()) {
            unique.push(w);
        }
    }
    unique
}

/// Dispatch to the configured LLM provider
async fn stream_llm_response(
    chat_config: &cgx_core::ChatConfig,
    prompt: &str,
    sources: &[SourceNode],
) -> anyhow::Result<SseStream> {
    let provider = std::env::var("CGX_CHAT_PROVIDER")
        .unwrap_or_else(|_| chat_config.provider.clone());

    let sources_json = serde_json::to_string(&serde_json::json!({
        "type": "sources",
        "nodes": sources,
    })).unwrap_or_else(|_| r#"{"type":"sources","nodes":[]}"#.to_string());

    let sources_event = axum::response::sse::Event::default().data(sources_json);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(chat_config.timeout as u64))
        .build()
        .context("Failed to build HTTP client")?;

    match provider.as_str() {
        "openai" => stream_openai(&client, chat_config, prompt, sources_event).await,
        "anthropic" => stream_anthropic(&client, chat_config, prompt, sources_event).await,
        "ollama" => stream_ollama(&client, chat_config, prompt, sources_event).await,
        "openai-compatible" => stream_openai_compatible(&client, chat_config, prompt, sources_event).await,
        _ => anyhow::bail!(
            "Unknown chat provider: '{}'. Supported: openai, anthropic, ollama, openai-compatible. \
             Set CGX_CHAT_PROVIDER or run `cgx init` to configure.",
            provider
        ),
    }
}

// ─────────────────────────────────────────────────────────────
// OpenAI
// ─────────────────────────────────────────────────────────────

async fn stream_openai(
    client: &reqwest::Client,
    config: &cgx_core::ChatConfig,
    prompt: &str,
    sources_event: axum::response::sse::Event,
) -> anyhow::Result<SseStream> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .context("OPENAI_API_KEY environment variable not set")?;

    let model = std::env::var("CGX_CHAT_MODEL")
        .unwrap_or_else(|_| config.model.clone());

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "You are a codebase expert assistant."},
            {"role": "user", "content": prompt}
        ],
        "stream": true,
    });

    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("Failed to connect to OpenAI")?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        anyhow::bail!("OpenAI API error: {}", err_text);
    }

    Ok(stream_sse(resp, sources_event, |line| {
        if line == "[DONE]" {
            return Some(("__done__".to_string(), true));
        }
        let json: serde_json::Value = serde_json::from_str(line).ok()?;
        let text = json
            .get("choices")?
            .get(0)?
            .get("delta")?
            .get("content")?
            .as_str()?;
        Some((text.to_string(), false))
    }))
}

// ─────────────────────────────────────────────────────────────
// Anthropic
// ─────────────────────────────────────────────────────────────

async fn stream_anthropic(
    client: &reqwest::Client,
    config: &cgx_core::ChatConfig,
    prompt: &str,
    sources_event: axum::response::sse::Event,
) -> anyhow::Result<SseStream> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .context("ANTHROPIC_API_KEY environment variable not set")?;

    let model = std::env::var("CGX_CHAT_MODEL")
        .unwrap_or_else(|_| config.model.clone());

    let body = serde_json::json!({
        "model": model,
        "system": "You are a codebase expert assistant.",
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": 4096,
        "stream": true,
    });

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("Failed to connect to Anthropic")?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Anthropic API error: {}", err_text);
    }

    Ok(stream_sse(resp, sources_event, |line| {
        let json: serde_json::Value = serde_json::from_str(line).ok()?;
        // Check for message_stop
        if json.get("type")?.as_str()? == "message_stop" {
            return Some(("__done__".to_string(), true));
        }
        let text = json
            .get("delta")?
            .get("text")?
            .as_str()?;
        Some((text.to_string(), false))
    }))
}

// ─────────────────────────────────────────────────────────────
// Ollama
// ─────────────────────────────────────────────────────────────

async fn stream_ollama(
    client: &reqwest::Client,
    config: &cgx_core::ChatConfig,
    prompt: &str,
    sources_event: axum::response::sse::Event,
) -> anyhow::Result<SseStream> {
    let host = std::env::var("CGX_OLLAMA_HOST")
        .unwrap_or_else(|_| config.ollama_host.clone());

    let model = std::env::var("CGX_CHAT_MODEL")
        .unwrap_or_else(|_| config.model.clone());

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "You are a codebase expert assistant."},
            {"role": "user", "content": prompt}
        ],
        "stream": true,
    });

    let url = format!("{}/api/chat", host.trim_end_matches('/'));

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context(format!("Failed to connect to Ollama at {}", url))?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Ollama API error: {}", err_text);
    }

    Ok(stream_ndjson(resp, sources_event, |line| {
        let json: serde_json::Value = serde_json::from_str(line).ok()?;
        // Check for done
        if json.get("done")?.as_bool()? {
            return Some(("__done__".to_string(), true));
        }
        let text = json
            .get("message")?
            .get("content")?
            .as_str()?;
        Some((text.to_string(), false))
    }))
}

// ─────────────────────────────────────────────────────────────
// OpenAI-compatible (e.g. Together, Fireworks, Groq, LM Studio)
// ─────────────────────────────────────────────────────────────

async fn stream_openai_compatible(
    client: &reqwest::Client,
    config: &cgx_core::ChatConfig,
    prompt: &str,
    sources_event: axum::response::sse::Event,
) -> anyhow::Result<SseStream> {
    let api_key = std::env::var("CGX_CHAT_API_KEY")
        .context("CGX_CHAT_API_KEY environment variable not set (required for openai-compatible provider)")?;

    let base_url = std::env::var("CGX_CHAT_BASE_URL")
        .context("CGX_CHAT_BASE_URL environment variable not set (required for openai-compatible provider)")?;

    let model = std::env::var("CGX_CHAT_MODEL")
        .unwrap_or_else(|_| config.model.clone());

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "You are a codebase expert assistant."},
            {"role": "user", "content": prompt}
        ],
        "stream": true,
    });

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context(format!("Failed to connect to {}", url))?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        anyhow::bail!("API error: {}", err_text);
    }

    Ok(stream_sse(resp, sources_event, |line| {
        if line == "[DONE]" {
            return Some(("__done__".to_string(), true));
        }
        let json: serde_json::Value = serde_json::from_str(line).ok()?;
        let text = json
            .get("choices")?
            .get(0)?
            .get("delta")?
            .get("content")?
            .as_str()?;
        Some((text.to_string(), false))
    }))
}

// ─────────────────────────────────────────────────────────────
// Generic SSE streamer (for OpenAI, Anthropic)
// ─────────────────────────────────────────────────────────────

fn stream_sse(
    resp: reqwest::Response,
    sources_event: axum::response::sse::Event,
    mut extract: impl FnMut(&str) -> Option<(String, bool)> + Send + 'static,
) -> SseStream {
    let stream = async_stream::stream! {
        yield Ok(sources_event);

        let body_stream = resp.bytes_stream()
            .map_err(std::io::Error::other);
        let reader = StreamReader::new(body_stream);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // SSE lines start with "data: " — skip everything else
            let data = if let Some(d) = line.strip_prefix("data: ") {
                d
            } else {
                // Skip event:, id:, and comment lines
                continue;
            };

            match extract(data) {
                Some((text, true)) if text == "__done__" => {
                    yield Ok(axum::response::sse::Event::default()
                        .data(r#"{"type":"done"}"#));
                    return;
                }
                Some((text, false)) if !text.is_empty() => {
                    let delta = serde_json::json!({
                        "type": "delta",
                        "text": text,
                    });
                    yield Ok(axum::response::sse::Event::default()
                        .data(delta.to_string()));
                }
                _ => {}
            }
        }

        yield Ok(axum::response::sse::Event::default()
            .data(r#"{"type":"done"}"#));
    };

    Box::pin(stream)
}

// ─────────────────────────────────────────────────────────────
// Generic NDJSON streamer (for Ollama)
// ─────────────────────────────────────────────────────────────

fn stream_ndjson(
    resp: reqwest::Response,
    sources_event: axum::response::sse::Event,
    mut extract: impl FnMut(&str) -> Option<(String, bool)> + Send + 'static,
) -> SseStream {
    let stream = async_stream::stream! {
        yield Ok(sources_event);

        let body_stream = resp.bytes_stream()
            .map_err(std::io::Error::other);
        let reader = StreamReader::new(body_stream);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            match extract(line) {
                Some((text, true)) if text == "__done__" => {
                    yield Ok(axum::response::sse::Event::default()
                        .data(r#"{"type":"done"}"#));
                    return;
                }
                Some((text, false)) if !text.is_empty() => {
                    let delta = serde_json::json!({
                        "type": "delta",
                        "text": text,
                    });
                    yield Ok(axum::response::sse::Event::default()
                        .data(delta.to_string()));
                }
                _ => {}
            }
        }

        yield Ok(axum::response::sse::Event::default()
            .data(r#"{"type":"done"}"#));
    };

    Box::pin(stream)
}
