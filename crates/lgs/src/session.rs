use crate::config;
use crate::types::{Round, Session};
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SessionInit {
    pub game: String,
    pub language: Option<String>,
    pub balance: Option<u64>,
    pub currency: Option<&'static str>,
}

pub struct SessionStore {
    sessions: DashMap<String, Session>,
    bet_counter: AtomicU64,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
            bet_counter: AtomicU64::new(0),
        }
    }

    pub fn create(&self, session_id: &str, game: &str, language: Option<String>) -> Session {
        self.upsert(
            session_id,
            SessionInit {
                game: game.to_string(),
                language,
                balance: None,
                currency: None,
            },
        )
    }

    /// Insert or replace a session with the given init params (admin-style).
    pub fn upsert(&self, session_id: &str, init: SessionInit) -> Session {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let session = Session {
            id: session_id.to_string(),
            game: init.game,
            balance: init.balance.unwrap_or(config::INITIAL_BALANCE),
            currency: init.currency.unwrap_or(config::CURRENCY),
            language: init.language.unwrap_or_else(|| config::LANGUAGE.to_string()),
            active_round: None,
            created_at: now,
        };
        self.sessions.insert(session_id.to_string(), session.clone());
        session
    }

    /// Fetch existing session, or create with defaults. Used by authenticate
    /// so a pre-configured session (set via Tauri prepare_session) is preserved.
    pub fn get_or_create(
        &self,
        session_id: &str,
        game: &str,
        language: Option<String>,
    ) -> Session {
        if let Some(s) = self.sessions.get(session_id) {
            return s.clone();
        }
        self.create(session_id, game, language)
    }

    pub fn get(&self, session_id: &str) -> Option<Session> {
        self.sessions.get(session_id).map(|s| s.clone())
    }

    pub fn set_active_round(&self, session_id: &str, round: Option<Round>) -> Option<Session> {
        let mut entry = self.sessions.get_mut(session_id)?;
        entry.active_round = round;
        Some(entry.clone())
    }

    pub fn deduct_bet(&self, session_id: &str, amount: u64) -> Option<Session> {
        let mut entry = self.sessions.get_mut(session_id)?;
        if entry.balance < amount {
            return None;
        }
        entry.balance -= amount;
        Some(entry.clone())
    }

    pub fn add_winnings(&self, session_id: &str, amount: u64) -> Option<Session> {
        let mut entry = self.sessions.get_mut(session_id)?;
        entry.balance = entry.balance.saturating_add(amount);
        Some(entry.clone())
    }

    pub fn next_bet_id(&self) -> u64 {
        self.bet_counter.fetch_add(1, Ordering::Relaxed) + 1
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}
