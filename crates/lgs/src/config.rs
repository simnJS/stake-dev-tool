use crate::types::{AuthConfig, JurisdictionFlags, API_MULTIPLIER};
use std::env;

pub const CURRENCY: &str = "USD";
pub const LANGUAGE: &str = "en";
pub const INITIAL_BALANCE: u64 = 10_000 * API_MULTIPLIER;

pub const MIN_BET: u64 = 20_000;
pub const MAX_BET: u64 = 100_000_000;
pub const STEP_BET: u64 = 20_000;
pub const DEFAULT_BET_LEVEL: u64 = 200_000;

pub const BET_LEVELS: &[u64] = &[
    20_000, 40_000, 60_000, 80_000, 100_000,
    200_000, 400_000, 600_000, 800_000, 1_000_000,
    2_000_000, 4_000_000, 6_000_000, 8_000_000, 10_000_000,
    20_000_000, 40_000_000, 60_000_000, 80_000_000, 100_000_000,
];

pub fn jurisdiction() -> JurisdictionFlags {
    JurisdictionFlags {
        social_casino: false,
        disabled_fullscreen: false,
        disabled_turbo: false,
        disabled_super_turbo: false,
        disabled_autoplay: false,
        disabled_slamstop: false,
        disabled_spacebar: false,
        disabled_buy_feature: false,
        display_net_position: false,
        display_rtp: true,
        display_session_timer: false,
        minimum_round_duration: 0,
    }
}

pub fn auth_config() -> AuthConfig {
    AuthConfig {
        game_id: String::new(),
        min_bet: MIN_BET,
        max_bet: MAX_BET,
        step_bet: STEP_BET,
        default_bet_level: DEFAULT_BET_LEVEL,
        bet_levels: BET_LEVELS,
        bet_modes: serde_json::json!({}),
        jurisdiction: jurisdiction(),
    }
}

pub struct ServerConfig {
    pub bind_addr: String,
    pub math_dir: String,
    pub ui_dir: Option<std::path::PathBuf>,
}

impl ServerConfig {
    pub fn from_env() -> Self {
        Self {
            bind_addr: env::var("LGS_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3001".to_string()),
            math_dir: env::var("LGS_MATH_DIR").unwrap_or_else(|_| "./math".to_string()),
            ui_dir: env::var("LGS_UI_DIR").ok().map(std::path::PathBuf::from),
        }
    }
}
