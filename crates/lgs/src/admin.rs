use crate::error::AppResult;
use crate::session::SessionInit;
use crate::settings;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/admin/sessions/prepare", post(prepare_session))
        .route("/api/admin/status", get(status))
        .route("/api/admin/settings", get(get_settings_handler))
        .route(
            "/api/admin/settings/toggle",
            post(toggle_resolution_handler),
        )
        .route(
            "/api/admin/settings/custom",
            post(add_custom_resolution_handler),
        )
        .route(
            "/api/admin/settings/custom/:id",
            delete(delete_custom_resolution_handler),
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
