use crate::error::{AppError, AppResult};
use crate::saved_rounds;
use crate::session::SessionInit;
use crate::settings;
use crate::state::{AppState, ForcedEvent};
use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/devtool/sessions/prepare", post(prepare_session))
        .route("/api/devtool/status", get(status))
        .route("/api/devtool/settings", get(get_settings_handler))
        .route(
            "/api/devtool/settings/toggle",
            post(toggle_resolution_handler),
        )
        .route(
            "/api/devtool/settings/custom",
            post(add_custom_resolution_handler),
        )
        .route(
            "/api/devtool/settings/custom/:id",
            delete(delete_custom_resolution_handler),
        )
        .route(
            "/api/devtool/force-event",
            get(get_forced_event)
                .post(set_forced_event)
                .delete(clear_forced_event),
        )
        .route("/api/devtool/sessions/:sid/last-event", get(get_last_event))
        .route("/api/devtool/sessions/:sid/events", get(get_events_history))
        .route(
            "/api/devtool/saved-rounds",
            get(list_saved_rounds).post(create_saved_round),
        )
        .route(
            "/api/devtool/saved-rounds/:id",
            patch(update_saved_round).delete(delete_saved_round),
        )
        .with_state(state)
}

const SUPPORTED_CURRENCIES: &[&str] = &[
    "USD", "CAD", "JPY", "EUR", "RUB", "CNY", "PHP", "INR", "IDR", "KRW", "BRL", "MXN", "DKK",
    "PLN", "VND", "TRY", "CLP", "ARS", "PEN", "NGN", "SAR", "ILS", "AED", "TWD", "NOK", "KWD",
    "JOD", "CRC", "TND", "SGD", "MYR", "OMR", "QAR", "BHD", "XGC", "XSC",
];

fn intern_currency(c: &str) -> &'static str {
    SUPPORTED_CURRENCIES
        .iter()
        .copied()
        .find(|s| s.eq_ignore_ascii_case(c))
        .unwrap_or("USD")
}

#[derive(Deserialize)]
pub struct PrepareSessionBody {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "gameSlug")]
    pub game_slug: String,
    #[serde(default)]
    pub balance: Option<u64>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
}

#[derive(Serialize)]
pub struct PrepareSessionResponse {
    pub ok: bool,
}

async fn prepare_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PrepareSessionBody>,
) -> AppResult<Json<PrepareSessionResponse>> {
    let init = SessionInit {
        game: body.game_slug,
        language: body.language,
        balance: body.balance,
        currency: body.currency.as_deref().map(intern_currency),
    };
    state.sessions.upsert(&body.session_id, init);
    Ok(Json(PrepareSessionResponse { ok: true }))
}

#[derive(Serialize)]
struct StatusResponse {
    ok: bool,
    version: &'static str,
}

async fn status() -> Json<StatusResponse> {
    Json(StatusResponse {
        ok: true,
        version: env!("CARGO_PKG_VERSION"),
    })
}

// ========== Settings (resolutions) ==========

async fn get_settings_handler() -> AppResult<Json<settings::Settings>> {
    let s = settings::load().await?;
    Ok(Json(s))
}

#[derive(Deserialize)]
pub struct ToggleResolutionBody {
    pub id: String,
    pub enabled: bool,
}

async fn toggle_resolution_handler(
    Json(body): Json<ToggleResolutionBody>,
) -> AppResult<Json<settings::Settings>> {
    let s = settings::toggle(&body.id, body.enabled).await?;
    Ok(Json(s))
}

#[derive(Deserialize)]
pub struct AddCustomResolutionBody {
    pub label: String,
    pub width: u32,
    pub height: u32,
}

async fn add_custom_resolution_handler(
    Json(body): Json<AddCustomResolutionBody>,
) -> AppResult<Json<settings::Settings>> {
    let s = settings::add_custom(body.label, body.width, body.height).await?;
    Ok(Json(s))
}

async fn delete_custom_resolution_handler(
    Path(id): Path<String>,
) -> AppResult<Json<settings::Settings>> {
    let s = settings::delete_custom(&id).await?;
    Ok(Json(s))
}

// ========== Force event / replay debug helpers ==========

#[derive(Serialize)]
pub struct ForcedEventResponse {
    pub forced: Option<ForcedEvent>,
}

async fn get_forced_event(State(state): State<Arc<AppState>>) -> Json<ForcedEventResponse> {
    Json(ForcedEventResponse {
        forced: state.forced_event.lock().clone(),
    })
}

async fn set_forced_event(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ForcedEvent>,
) -> Json<ForcedEventResponse> {
    *state.forced_event.lock() = Some(body);
    Json(ForcedEventResponse {
        forced: state.forced_event.lock().clone(),
    })
}

async fn clear_forced_event(State(state): State<Arc<AppState>>) -> Json<ForcedEventResponse> {
    *state.forced_event.lock() = None;
    Json(ForcedEventResponse { forced: None })
}

#[derive(Serialize)]
pub struct LastEventResponse {
    #[serde(rename = "eventId")]
    pub event_id: Option<u32>,
    #[serde(rename = "payoutMultiplier")]
    pub payout_multiplier: Option<u32>,
}

async fn get_last_event(
    State(state): State<Arc<AppState>>,
    Path(sid): Path<String>,
) -> AppResult<Json<LastEventResponse>> {
    let s = state.sessions.get(&sid).ok_or(AppError::SessionNotFound)?;
    Ok(Json(LastEventResponse {
        event_id: s.last_event_id,
        payout_multiplier: s.last_payout_multiplier,
    }))
}

#[derive(Serialize)]
pub struct EventsHistoryResponse {
    pub count: usize,
    pub events: Vec<crate::types::EventEntry>,
}

async fn get_events_history(
    State(state): State<Arc<AppState>>,
    Path(sid): Path<String>,
) -> AppResult<Json<EventsHistoryResponse>> {
    let s = state.sessions.get(&sid).ok_or(AppError::SessionNotFound)?;
    Ok(Json(EventsHistoryResponse {
        count: s.event_history.len(),
        events: s.event_history,
    }))
}

// ========== Saved rounds ==========

#[derive(Deserialize)]
pub struct SavedRoundsQuery {
    #[serde(default, rename = "gameSlug")]
    pub game_slug: Option<String>,
}

#[derive(Serialize)]
pub struct SavedRoundsResponse {
    pub rounds: Vec<saved_rounds::SavedRound>,
}

async fn list_saved_rounds(
    Query(q): Query<SavedRoundsQuery>,
) -> AppResult<Json<SavedRoundsResponse>> {
    let rounds = saved_rounds::list(q.game_slug.as_deref()).await?;
    Ok(Json(SavedRoundsResponse { rounds }))
}

#[derive(Deserialize)]
pub struct CreateSavedRoundBody {
    #[serde(rename = "gameSlug")]
    pub game_slug: String,
    pub mode: String,
    #[serde(rename = "eventId")]
    pub event_id: u32,
    #[serde(default)]
    pub description: String,
}

async fn create_saved_round(
    Json(body): Json<CreateSavedRoundBody>,
) -> AppResult<Json<saved_rounds::SavedRound>> {
    let r = saved_rounds::create(body.game_slug, body.mode, body.event_id, body.description)
        .await?;
    Ok(Json(r))
}

#[derive(Deserialize)]
pub struct UpdateSavedRoundBody {
    pub description: String,
}

async fn update_saved_round(
    Path(id): Path<String>,
    Json(body): Json<UpdateSavedRoundBody>,
) -> AppResult<Json<saved_rounds::SavedRound>> {
    let r = saved_rounds::update_description(&id, body.description).await?;
    Ok(Json(r))
}

#[derive(Serialize)]
pub struct OkResponse {
    pub ok: bool,
}

async fn delete_saved_round(Path(id): Path<String>) -> AppResult<Json<OkResponse>> {
    saved_rounds::delete(&id).await?;
    Ok(Json(OkResponse { ok: true }))
}
