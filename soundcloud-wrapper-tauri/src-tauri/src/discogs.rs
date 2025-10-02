use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::async_runtime;
use tauri::AppHandle;
use tauri::Emitter;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::library::LibraryStore;
use crate::SoundcloudTrackPayload;

const SEARCH_URL: &str = "https://api.discogs.com/database/search";
const DISCOGS_AMBIGUITY_EVENT: &str = "app://discogs/lookup-ambiguous";
const USER_AGENT: &str = "SoundCloudWrapper/0.1 (+https://github.com/your-org/desktop-soundcloud)";

#[derive(Clone)]
pub struct DiscogsService {
    sender: mpsc::Sender<SoundcloudTrackPayload>,
}

impl DiscogsService {
    pub fn new(app: &AppHandle, library: Arc<Mutex<LibraryStore>>) -> Self {
        let (sender, mut receiver) = mpsc::channel::<SoundcloudTrackPayload>(32);
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .expect("failed to build Discogs client");
        let app_handle = app.clone();
        async_runtime::spawn(async move {
            let mut rate_limiter = RateLimiter::new(Duration::from_millis(1100));
            while let Some(payload) = receiver.recv().await {
                if payload.track_id.is_empty() {
                    continue;
                }
                process_job(
                    &app_handle,
                    Arc::clone(&library),
                    &client,
                    &mut rate_limiter,
                    payload,
                )
                .await;
            }
        });

        Self { sender }
    }

    pub fn queue_lookup(&self, payload: SoundcloudTrackPayload) {
        let mut sender = self.sender.clone();
        async_runtime::spawn(async move {
            if let Err(error) = sender.send(payload).await {
                eprintln!("[discogs] failed to enqueue lookup: {error}");
            }
        });
    }
}

struct RateLimiter {
    last: Option<Instant>,
    interval: Duration,
}

impl RateLimiter {
    fn new(interval: Duration) -> Self {
        Self {
            last: None,
            interval,
        }
    }

    async fn wait(&mut self) {
        if let Some(last) = self.last {
            let elapsed = last.elapsed();
            if elapsed < self.interval {
                sleep(self.interval - elapsed).await;
            }
        }
        self.last = Some(Instant::now());
    }
}

async fn process_job(
    app: &AppHandle,
    library: Arc<Mutex<LibraryStore>>,
    client: &Client,
    rate_limiter: &mut RateLimiter,
    payload: SoundcloudTrackPayload,
) {
    let track_id = payload.track_id.clone();
    let query = build_search_term(&payload);

    if query.trim().is_empty() {
        if let Ok(mut store) = library.lock() {
            if let Err(error) =
                store.record_discogs_failure(&track_id, &query, "missing title or artist")
            {
                eprintln!("[discogs] failed to persist lookup failure for {track_id}: {error}");
            }
        }
        return;
    }

    match perform_lookup(client, rate_limiter, &payload, &query).await {
        Ok(LookupResult::Success {
            release,
            confidence,
        }) => {
            if let Ok(mut store) = library.lock() {
                if let Err(error) =
                    store.record_discogs_success(&track_id, &query, &release, confidence)
                {
                    eprintln!("[discogs] failed to persist lookup success for {track_id}: {error}");
                }
            }
        }
        Ok(LookupResult::Ambiguous { candidates }) => {
            if let Ok(mut store) = library.lock() {
                if let Err(error) = store.record_discogs_ambiguity(&track_id, &query, &candidates) {
                    eprintln!(
                        "[discogs] failed to persist lookup ambiguity for {track_id}: {error}"
                    );
                }
            }

            if let Err(error) = app.emit(
                DISCOGS_AMBIGUITY_EVENT,
                json!({
                    "trackId": track_id,
                    "query": query,
                    "candidates": candidates,
                }),
            ) {
                eprintln!("[discogs] failed to emit ambiguity event: {error}");
            }
        }
        Err(failure) => {
            if let Ok(mut store) = library.lock() {
                if let Err(error) =
                    store.record_discogs_failure(&track_id, &query, &failure.into_message())
                {
                    eprintln!("[discogs] failed to persist lookup failure for {track_id}: {error}");
                }
            }
        }
    }
}

enum LookupResult {
    Success { release: Value, confidence: f32 },
    Ambiguous { candidates: Vec<Value> },
}

enum LookupFailure {
    Message(String),
    Error(String),
}

impl LookupFailure {
    fn into_message(self) -> String {
        match self {
            LookupFailure::Message(message) => message,
            LookupFailure::Error(error) => error,
        }
    }
}

#[derive(Deserialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
}

#[derive(Clone, Deserialize)]
struct SearchResult {
    id: Option<u64>,
    title: Option<String>,
    #[serde(rename = "type")]
    result_type: Option<String>,
    resource_url: Option<String>,
    score: Option<f32>,
    year: Option<u32>,
    country: Option<String>,
    thumb: Option<String>,
}

fn build_search_term(payload: &SoundcloudTrackPayload) -> String {
    let mut terms = Vec::new();
    if let Some(artist) = payload.artist.as_ref() {
        if !artist.trim().is_empty() {
            terms.push(artist.trim().to_string());
        }
    }
    if let Some(title) = payload.title.as_ref() {
        if !title.trim().is_empty() {
            terms.push(title.trim().to_string());
        }
    }
    terms.join(" ")
}

async fn perform_lookup(
    client: &Client,
    rate_limiter: &mut RateLimiter,
    payload: &SoundcloudTrackPayload,
    query: &str,
) -> Result<LookupResult, LookupFailure> {
    let mut params = vec![
        ("type", "release".to_string()),
        ("per_page", "5".to_string()),
    ];

    if let Some(artist) = payload.artist.as_ref() {
        params.push(("artist", artist.clone()));
    }
    if let Some(title) = payload.title.as_ref() {
        params.push(("release_title", title.clone()));
    }
    if !query.is_empty() {
        params.push(("q", query.to_string()));
    }

    rate_limiter.wait().await;
    let response = client
        .get(SEARCH_URL)
        .query(&params)
        .send()
        .await
        .map_err(|error| LookupFailure::Error(error.to_string()))?;

    if !response.status().is_success() {
        return Err(LookupFailure::Message(format!(
            "search returned status {}",
            response.status()
        )));
    }

    let body = response
        .json::<SearchResponse>()
        .await
        .map_err(|error| LookupFailure::Error(error.to_string()))?;

    let mut results: Vec<SearchResult> = body
        .results
        .into_iter()
        .filter(|result| {
            matches!(result.result_type.as_deref(), Some("release"))
                && result.resource_url.is_some()
        })
        .collect();

    if results.is_empty() {
        return Err(LookupFailure::Message("no releases found".to_string()));
    }

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let top_score = results
        .first()
        .and_then(|result| result.score)
        .unwrap_or(0.0);
    let second_score = results
        .get(1)
        .and_then(|result| result.score)
        .unwrap_or(0.0);

    if results.len() == 1 || (top_score >= 85.0 && top_score - second_score >= 10.0) {
        let top = results.first().cloned().unwrap();
        let release_url = top.resource_url.unwrap_or_else(|| {
            top.id
                .map(|id| format!("https://api.discogs.com/releases/{id}"))
                .unwrap_or_default()
        });

        if release_url.is_empty() {
            return Err(LookupFailure::Message(
                "top result missing release URL".to_string(),
            ));
        }

        rate_limiter.wait().await;
        let release = client
            .get(release_url)
            .send()
            .await
            .map_err(|error| LookupFailure::Error(error.to_string()))?
            .json::<Value>()
            .await
            .map_err(|error| LookupFailure::Error(error.to_string()))?;

        return Ok(LookupResult::Success {
            release,
            confidence: top_score,
        });
    }

    let candidates = results
        .into_iter()
        .take(5)
        .map(|result| {
            json!({
                "id": result.id,
                "title": result.title,
                "score": result.score,
                "year": result.year,
                "country": result.country,
                "resourceUrl": result.resource_url,
                "thumb": result.thumb,
            })
        })
        .collect();

    Ok(LookupResult::Ambiguous { candidates })
}
