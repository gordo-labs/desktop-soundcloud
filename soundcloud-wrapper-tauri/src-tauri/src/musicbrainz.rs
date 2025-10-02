use std::cmp::Ordering;
use std::env;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use tauri::async_runtime;
use tauri::AppHandle;
use tauri::Emitter;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::library::LibraryStore;
use crate::SoundcloudTrackPayload;

const SEARCH_URL: &str = "https://musicbrainz.org/ws/2/release/";
const MUSICBRAINZ_AMBIGUITY_EVENT: &str = "app://musicbrainz/lookup-ambiguous";
const MAX_ATTEMPTS: usize = 3;

#[derive(Clone)]
pub struct MusicbrainzService {
    sender: mpsc::Sender<SoundcloudTrackPayload>,
}

impl MusicbrainzService {
    pub fn new(app: &AppHandle, library: Arc<Mutex<LibraryStore>>) -> Self {
        let (sender, mut receiver) = mpsc::channel::<SoundcloudTrackPayload>(32);
        let credentials = Arc::new(MusicbrainzCredentials::load(app));
        let client = Client::builder()
            .user_agent(credentials.user_agent.clone())
            .build()
            .expect("failed to build MusicBrainz client");
        let app_handle = app.clone();
        async_runtime::spawn(async move {
            let mut rate_limiter = RateLimiter::new(Duration::from_millis(1100));
            let worker_credentials = Arc::clone(&credentials);
            while let Some(payload) = receiver.recv().await {
                if payload.track_id.is_empty() {
                    continue;
                }
                process_job(
                    &app_handle,
                    Arc::clone(&library),
                    &client,
                    worker_credentials.as_ref(),
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
                eprintln!("[musicbrainz] failed to enqueue lookup: {error}");
            }
        });
    }
}

struct MusicbrainzCredentials {
    user_agent: String,
    token: Option<String>,
}

impl MusicbrainzCredentials {
    fn load(app: &AppHandle) -> Self {
        let package_info = app.package_info();
        let default_name = package_info.name.clone();
        let default_version = package_info.version.to_string();
        let app_name = env::var("MUSICBRAINZ_APP_NAME")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(default_name);
        let app_version = env::var("MUSICBRAINZ_APP_VERSION")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(default_version);
        let contact = env::var("MUSICBRAINZ_APP_CONTACT")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "https://github.com/your-org/desktop-soundcloud".to_string());
        let user_agent = format!("{app_name}/{app_version} ({contact})");
        let token = env::var("MUSICBRAINZ_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty());

        Self { user_agent, token }
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
    credentials: &MusicbrainzCredentials,
    rate_limiter: &mut RateLimiter,
    payload: SoundcloudTrackPayload,
) {
    let track_id = payload.track_id.clone();
    let query = build_search_query(&payload);

    if query.trim().is_empty() {
        if let Ok(mut store) = library.lock() {
            if let Err(error) =
                store.record_musicbrainz_failure(&track_id, &query, "missing title or artist")
            {
                eprintln!("[musicbrainz] failed to persist lookup failure for {track_id}: {error}");
            }
        }
        return;
    }

    match perform_lookup(client, credentials, rate_limiter, &query).await {
        Ok(LookupResult::Success {
            release,
            confidence,
        }) => {
            if let Ok(mut store) = library.lock() {
                if let Err(error) =
                    store.record_musicbrainz_success(&track_id, &query, &release, confidence)
                {
                    eprintln!(
                        "[musicbrainz] failed to persist lookup success for {track_id}: {error}"
                    );
                }
            }
        }
        Ok(LookupResult::Ambiguous { candidates }) => {
            if let Ok(mut store) = library.lock() {
                if let Err(error) =
                    store.record_musicbrainz_ambiguity(&track_id, &query, &candidates)
                {
                    eprintln!(
                        "[musicbrainz] failed to persist lookup ambiguity for {track_id}: {error}"
                    );
                }
            }

            if let Err(error) = app.emit(
                MUSICBRAINZ_AMBIGUITY_EVENT,
                json!({
                    "trackId": track_id,
                    "query": query,
                    "candidates": candidates,
                }),
            ) {
                eprintln!("[musicbrainz] failed to emit ambiguity event: {error}");
            }
        }
        Err(failure) => {
            if let Ok(mut store) = library.lock() {
                if let Err(error) =
                    store.record_musicbrainz_failure(&track_id, &query, &failure.into_message())
                {
                    eprintln!(
                        "[musicbrainz] failed to persist lookup failure for {track_id}: {error}"
                    );
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

async fn perform_lookup(
    client: &Client,
    credentials: &MusicbrainzCredentials,
    rate_limiter: &mut RateLimiter,
    query: &str,
) -> Result<LookupResult, LookupFailure> {
    let mut attempts = 0usize;
    loop {
        attempts += 1;
        rate_limiter.wait().await;
        let mut request =
            client
                .get(SEARCH_URL)
                .query(&[("fmt", "json"), ("limit", "5"), ("query", query)]);

        if let Some(token) = credentials.token.as_ref() {
            request = request.bearer_auth(token);
        }

        let response = request
            .send()
            .await
            .map_err(|error| LookupFailure::Error(format!("request failed: {error}")))?;

        match response.status() {
            StatusCode::OK => {
                let body: Value = response.json().await.map_err(|error| {
                    LookupFailure::Error(format!("failed to parse MusicBrainz response: {error}"))
                })?;
                return interpret_lookup(body);
            }
            StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE => {
                let retry_after = response
                    .headers()
                    .get("Retry-After")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .map(Duration::from_secs)
                    .unwrap_or_else(|| Duration::from_secs(5));
                sleep(retry_after).await;
                if attempts >= MAX_ATTEMPTS {
                    return Err(LookupFailure::Message(
                        "rate limited by MusicBrainz".to_string(),
                    ));
                }
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                return Err(LookupFailure::Message(
                    "unauthorized MusicBrainz request".to_string(),
                ));
            }
            StatusCode::NOT_FOUND => {
                return Err(LookupFailure::Message(
                    "no releases found for track".to_string(),
                ));
            }
            status => {
                return Err(LookupFailure::Message(format!(
                    "unexpected MusicBrainz status: {status}"
                )));
            }
        }
    }
}

fn interpret_lookup(body: Value) -> Result<LookupResult, LookupFailure> {
    let releases = body
        .get("releases")
        .and_then(|value| value.as_array())
        .ok_or_else(|| LookupFailure::Message("invalid response payload".to_string()))?;

    let mut scored: Vec<(f32, Value)> = Vec::new();
    for release in releases.iter().cloned() {
        let score = release
            .get("score")
            .and_then(|value| value.as_f64())
            .map(|value| value as f32)
            .unwrap_or(0.0);
        scored.push((score, release));
    }

    if scored.is_empty() {
        return Err(LookupFailure::Message(
            "MusicBrainz returned no releases".to_string(),
        ));
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or_else(|| Ordering::Equal));

    let mut releases: Vec<Value> = scored.iter().map(|(_, release)| release.clone()).collect();
    let (mut best_score, best_release) = scored
        .into_iter()
        .next()
        .ok_or_else(|| LookupFailure::Message("MusicBrainz returned no releases".to_string()))?;

    if best_score <= 0.0 {
        best_score = 100.0;
    }

    let second_score = releases
        .get(1)
        .and_then(|release| release.get("score"))
        .and_then(|value| value.as_f64())
        .map(|value| value as f32)
        .unwrap_or(0.0);

    let is_confident = releases.len() == 1
        || best_score >= 95.0
        || (best_score >= 85.0 && (best_score - second_score) >= 10.0);

    if is_confident {
        Ok(LookupResult::Success {
            release: best_release,
            confidence: best_score,
        })
    } else {
        releases.truncate(5);
        Ok(LookupResult::Ambiguous {
            candidates: releases,
        })
    }
}

fn build_search_query(payload: &SoundcloudTrackPayload) -> String {
    let mut components = Vec::new();

    if let Some(artist) = payload.artist.as_ref().and_then(normalize_term) {
        components.push(format!("artist:\"{artist}\""));
    }

    if let Some(title) = payload.title.as_ref().and_then(normalize_term) {
        components.push(format!("recording:\"{title}\""));
    }

    if let Some(album) = payload
        .tags
        .iter()
        .find(|tag| tag.to_lowercase().contains("album:"))
    {
        if let Some(term) = normalize_term(album) {
            components.push(format!("release:\"{term}\""));
        }
    }

    components.join(" AND ")
}

fn normalize_term(value: &String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.replace('"', "\\\""))
    }
}
