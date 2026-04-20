use crate::config;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::types::{
    AuthenticateResponse, Balance, BalanceResponse, BetEventResponse, EndRoundResponse,
    PlayResponse, Round,
};
use axum::extract::{Path, State};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/rgs/:game/wallet/authenticate", post(authenticate))
        .route("/api/rgs/:game/wallet/balance", post(balance))
        .route("/api/rgs/:game/wallet/play", post(play))
        .route("/api/rgs/:game/wallet/end-round", post(end_round))
        .route("/api/rgs/:game/bet/event", post(bet_event))
        .with_state(state)
}

#[derive(Deserialize)]
struct AuthBody {
    #[serde(rename = "sessionID")]
    session_id: Option<String>,
    language: Option<String>,
}

async fn authenticate(
    State(state): State<Arc<AppState>>,
    Path(game): Path<String>,
    Json(body): Json<AuthBody>,
) -> AppResult<Json<AuthenticateResponse>> {
    let session_id = body.session_id.ok_or(AppError::MissingField("sessionID"))?;
    let session = state
        .sessions
        .get_or_create(&session_id, &game, body.language);

    let engine = state.engine.clone();
    let game_clone = game.clone();
    tokio::spawn(async move {
        if let Err(e) = engine.preload(&game_clone).await {
            tracing::warn!(?e, game = %game_clone, "preload failed");
        }
    });

    Ok(Json(AuthenticateResponse {
        balance: Balance {
            amount: session.balance,
            currency: session.currency,
        },
        round: None,
        config: config::auth_config(),
        meta: None,
    }))
}

#[derive(Deserialize)]
struct BalanceBody {
    #[serde(rename = "sessionID")]
    session_id: Option<String>,
}

async fn balance(
    State(state): State<Arc<AppState>>,
    Path(_game): Path<String>,
    Json(body): Json<BalanceBody>,
) -> AppResult<Json<BalanceResponse>> {
    let session_id = body.session_id.ok_or(AppError::MissingField("sessionID"))?;
    let session = state
        .sessions
        .get(&session_id)
        .ok_or(AppError::SessionNotFound)?;
    Ok(Json(BalanceResponse {
        balance: Balance {
            amount: session.balance,
            currency: session.currency,
        },
    }))
}

#[derive(Deserialize)]
struct PlayBody {
    #[serde(rename = "sessionID")]
    session_id: Option<String>,
    mode: Option<String>,
    amount: Option<u64>,
}

async fn play(
    State(state): State<Arc<AppState>>,
    Path(game): Path<String>,
    Json(body): Json<PlayBody>,
) -> AppResult<Json<PlayResponse>> {
    let session_id = body.session_id.ok_or(AppError::MissingField("sessionID"))?;
    let mode = body.mode.ok_or(AppError::MissingField("mode"))?;
    let amount = body.amount.ok_or(AppError::MissingField("amount"))?;

    state
        .sessions
        .get(&session_id)
        .ok_or(AppError::SessionNotFound)?;

    let mode_cost = state.engine.get_mode_cost(&game, &mode).await?;
    let total_cost = amount.saturating_mul(mode_cost);

    state
        .sessions
        .deduct_bet(&session_id, total_cost)
        .ok_or(AppError::InsufficientBalance)?;

    let result = state.engine.play_spin(&game, &mode, total_cost).await?;

    if result.payout > 0 {
        state.sessions.add_winnings(&session_id, result.payout);
    }

    let round = Round {
        bet_id: state.sessions.next_bet_id(),
        amount: total_cost,
        payout: result.payout,
        payout_multiplier: result.payout_multiplier as f64 / 100.0,
        active: false,
        mode,
        event: None,
        state: result.state,
    };

    state
        .sessions
        .set_active_round(&session_id, Some(round.clone()));

    let final_session = state
        .sessions
        .get(&session_id)
        .ok_or(AppError::SessionNotFound)?;

    Ok(Json(PlayResponse {
        balance: Balance {
            amount: final_session.balance,
            currency: final_session.currency,
        },
        round,
    }))
}

#[derive(Deserialize)]
struct EndRoundBody {
    #[serde(rename = "sessionID")]
    session_id: Option<String>,
}

async fn end_round(
    State(state): State<Arc<AppState>>,
    Path(_game): Path<String>,
    Json(body): Json<EndRoundBody>,
) -> AppResult<Json<EndRoundResponse>> {
    let session_id = body.session_id.ok_or(AppError::MissingField("sessionID"))?;
    state
        .sessions
        .get(&session_id)
        .ok_or(AppError::SessionNotFound)?;

    state.sessions.set_active_round(&session_id, None);

    let session = state
        .sessions
        .get(&session_id)
        .ok_or(AppError::SessionNotFound)?;
    Ok(Json(EndRoundResponse {
        balance: Balance {
            amount: session.balance,
            currency: session.currency,
        },
        round: None,
        config: config::auth_config(),
        meta: None,
    }))
}

#[derive(Deserialize)]
struct BetEventBody {
    #[serde(rename = "sessionID")]
    session_id: Option<String>,
    event: Option<String>,
}

async fn bet_event(
    State(state): State<Arc<AppState>>,
    Path(_game): Path<String>,
    Json(body): Json<BetEventBody>,
) -> AppResult<Json<BetEventResponse>> {
    let session_id = body.session_id.ok_or(AppError::MissingField("sessionID"))?;
    state
        .sessions
        .get(&session_id)
        .ok_or(AppError::SessionNotFound)?;
    Ok(Json(BetEventResponse { event: body.event }))
}
