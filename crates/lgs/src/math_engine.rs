use crate::config::ServerConfig;
use crate::error::{AppError, AppResult};
use crate::types::{GameConfig, GameMode, WeightEntry};
use dashmap::DashMap;
use rand::RngCore;
use serde::Serialize;
use serde_json::value::RawValue;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::OnceCell;

pub struct BooksIndex {
    pub buffer: Vec<u8>,
    pub offsets: Vec<u32>,
}

pub struct WeightSampler {
    pub entries: Vec<WeightEntry>,
    pub cum_weights: Vec<u64>,
    pub total_weight: u64,
}

pub struct ModeAssets {
    pub sampler: Arc<WeightSampler>,
    pub books: Arc<BooksIndex>,
}

pub struct MathEngine {
    cfg: ServerConfig,
    configs: DashMap<String, Arc<OnceCell<Arc<GameConfig>>>>,
    modes: DashMap<String, Arc<OnceCell<Arc<ModeAssets>>>>,
}

impl MathEngine {
    pub fn new(cfg: ServerConfig) -> Self {
        Self {
            cfg,
            configs: DashMap::new(),
            modes: DashMap::new(),
        }
    }

    fn file_path(&self, game: &str, file: &str) -> PathBuf {
        PathBuf::from(&self.cfg.math_dir).join(game).join(file)
    }

    async fn read_file(&self, game: &str, file: &str) -> AppResult<Vec<u8>> {
        let path = self.file_path(game, file);
        fs::read(&path)
            .await
            .map_err(|e| AppError::Parse(format!("read {}: {e}", path.display())))
    }

    pub async fn load_config(&self, game: &str) -> AppResult<Arc<GameConfig>> {
        let cell = self
            .configs
            .entry(game.to_string())
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone();
        cell.get_or_try_init(|| async {
            let bytes = self.read_file(game, "index.json").await?;
            let cfg: GameConfig = sonic_rs::from_slice(&bytes)
                .map_err(|e| AppError::Parse(format!("index.json: {e}")))?;
            Ok::<Arc<GameConfig>, AppError>(Arc::new(cfg))
        })
        .await
        .cloned()
    }

    pub async fn get_mode(&self, game: &str, mode_name: &str) -> AppResult<GameMode> {
        let cfg = self.load_config(game).await?;
        cfg.modes
            .iter()
            .find(|m| m.name == mode_name)
            .cloned()
            .ok_or_else(|| AppError::ModeNotFound {
                game: game.to_string(),
                mode: mode_name.to_string(),
            })
    }

    pub async fn get_mode_cost(&self, game: &str, mode_name: &str) -> AppResult<u64> {
        Ok(self
            .get_mode(game, mode_name)
            .await
            .map(|m| m.cost)
            .unwrap_or(1))
    }

    pub async fn load_assets(&self, game: &str, mode: &GameMode) -> AppResult<Arc<ModeAssets>> {
        let key = format!("{game}:{}", mode.name);
        let cell = self
            .modes
            .entry(key)
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone();
        let game = game.to_string();
        let mode = mode.clone();
        cell.get_or_try_init(|| async move {
            let weights_bytes = self.read_file(&game, &mode.weights).await?;
            let books_bytes = self.read_file(&game, &mode.events).await?;

            let weights_text = String::from_utf8(weights_bytes)
                .map_err(|e| AppError::Parse(format!("weights utf8: {e}")))?;
            let sampler = parse_weights(&weights_text)?;

            let books = decompress_and_index(&books_bytes)?;

            Ok::<Arc<ModeAssets>, AppError>(Arc::new(ModeAssets {
                sampler: Arc::new(sampler),
                books: Arc::new(books),
            }))
        })
        .await
        .cloned()
    }

    pub async fn preload(&self, game: &str) -> AppResult<()> {
        let cfg = self.load_config(game).await?;
        if let Some(base) = cfg.modes.iter().find(|m| m.name == "base") {
            self.load_assets(game, base).await?;
        }
        Ok(())
    }

    pub async fn play_spin(
        &self,
        game: &str,
        mode_name: &str,
        bet_amount: u64,
    ) -> AppResult<SpinResult> {
        let mode = self.get_mode(game, mode_name).await?;
        let assets = self.load_assets(game, &mode).await?;

        let pick = weighted_pick(&assets.sampler);
        self.build_result(
            &mode,
            &assets,
            pick.event_id,
            pick.payout_multiplier,
            bet_amount,
        )
    }

    /// Like `play_spin` but forces a specific event id (bypasses the RNG).
    /// Used for replay / debug "force next event" flows.
    pub async fn play_forced(
        &self,
        game: &str,
        mode_name: &str,
        bet_amount: u64,
        event_id: u32,
    ) -> AppResult<SpinResult> {
        let mode = self.get_mode(game, mode_name).await?;
        let assets = self.load_assets(game, &mode).await?;

        // Find the weight entry for this event to get the authoritative payout
        // multiplier. Weights table is small (~1k entries), a linear search is fine.
        let entry = assets
            .sampler
            .entries
            .iter()
            .find(|e| e.event_id == event_id)
            .ok_or_else(|| AppError::Parse(format!("event {event_id} not found in weights")))?;

        self.build_result(
            &mode,
            &assets,
            entry.event_id,
            entry.payout_multiplier,
            bet_amount,
        )
    }

    fn build_result(
        &self,
        mode: &GameMode,
        assets: &Arc<ModeAssets>,
        event_id: u32,
        payout_multiplier: u32,
        bet_amount: u64,
    ) -> AppResult<SpinResult> {
        let state = read_event(&assets.books, event_id)?;
        let base_bet = bet_amount / mode.cost.max(1);
        let payout = (base_bet.saturating_mul(payout_multiplier as u64)) / 100;
        Ok(SpinResult {
            event_id,
            payout_multiplier,
            payout,
            state,
        })
    }

    /// Compute notable bet ids per mode (lowest-payout / "average" winning hit
    /// / max-payout). Loads each mode's sampler — already cached after the
    /// first call — so a second call is essentially free. Used by the test
    /// view's "Notable rounds" panel.
    pub async fn game_bet_stats(&self, game: &str) -> AppResult<Vec<ModeBetStats>> {
        let cfg = self.load_config(game).await?;
        let mut out = Vec::with_capacity(cfg.modes.len());
        for mode in &cfg.modes {
            let assets = self.load_assets(game, mode).await?;
            if let Some(stats) = compute_bet_stats(&assets.sampler) {
                out.push(ModeBetStats {
                    mode: mode.name.clone(),
                    stats,
                });
            }
        }
        Ok(out)
    }

    /// Fetch the raw event state + payout multiplier for replay / bet-replay endpoint.
    pub async fn replay_event(
        &self,
        game: &str,
        mode_name: &str,
        event_id: u32,
    ) -> AppResult<ReplayResult> {
        let mode = self.get_mode(game, mode_name).await?;
        let assets = self.load_assets(game, &mode).await?;
        let entry = assets
            .sampler
            .entries
            .iter()
            .find(|e| e.event_id == event_id)
            .ok_or_else(|| AppError::Parse(format!("event {event_id} not found in weights")))?;
        let state = read_event(&assets.books, entry.event_id)?;
        Ok(ReplayResult {
            payout_multiplier: entry.payout_multiplier,
            cost_multiplier: mode.cost,
            state,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct NotableBet {
    #[serde(rename = "eventId")]
    pub event_id: u32,
    #[serde(rename = "payoutMultiplier")]
    pub payout_multiplier: u32,
}

/// Three "interesting" bet ids picked from a mode's lookup table:
/// - `min`: a no-win round (payoutMultiplier = 0 if any exist, else the
///   smallest payout)
/// - `avg`: the round whose payoutMultiplier is closest to the
///   weight-weighted average of *winning* multipliers (i.e. the typical
///   look of a winning spin)
/// - `max`: the highest payoutMultiplier in the table
#[derive(Debug, Clone, Copy, Serialize)]
pub struct BetStats {
    pub min: NotableBet,
    pub avg: NotableBet,
    pub max: NotableBet,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModeBetStats {
    pub mode: String,
    pub stats: BetStats,
}

fn notable_from(entry: &WeightEntry) -> NotableBet {
    NotableBet {
        event_id: entry.event_id,
        payout_multiplier: entry.payout_multiplier,
    }
}

fn compute_bet_stats(sampler: &WeightSampler) -> Option<BetStats> {
    if sampler.entries.is_empty() {
        return None;
    }

    let min_entry = sampler
        .entries
        .iter()
        .find(|e| e.payout_multiplier == 0)
        .or_else(|| sampler.entries.iter().min_by_key(|e| e.payout_multiplier))?;

    let max_entry = sampler.entries.iter().max_by_key(|e| e.payout_multiplier)?;

    // Weighted mean of winning payoutMultipliers — represents the EV of a
    // winning spin. We then pick the entry whose pm is closest to that mean.
    let winners: Vec<&WeightEntry> = sampler
        .entries
        .iter()
        .filter(|e| e.payout_multiplier > 0)
        .collect();

    let avg_entry = if winners.is_empty() {
        // No winners at all: avg falls back to min (degenerate but coherent).
        min_entry
    } else {
        let total_w: u128 = winners.iter().map(|e| e.weight as u128).sum();
        let weighted_sum: u128 = winners
            .iter()
            .map(|e| e.weight as u128 * e.payout_multiplier as u128)
            .sum();
        let avg_pm = (weighted_sum / total_w.max(1)) as u32;
        winners
            .iter()
            .min_by_key(|e| e.payout_multiplier.abs_diff(avg_pm))
            .copied()
            .unwrap_or(min_entry)
    };

    Some(BetStats {
        min: notable_from(min_entry),
        avg: notable_from(avg_entry),
        max: notable_from(max_entry),
    })
}

pub struct ReplayResult {
    pub payout_multiplier: u32,
    pub cost_multiplier: u64,
    pub state: Arc<RawValue>,
}

pub struct SpinResult {
    pub event_id: u32,
    pub payout_multiplier: u32,
    pub payout: u64,
    pub state: Arc<RawValue>,
}

fn parse_weights(text: &str) -> AppResult<WeightSampler> {
    let mut entries = Vec::with_capacity(1024);
    for (lineno, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut it = line.split(',');
        let event_id = it
            .next()
            .ok_or_else(|| AppError::Parse(format!("weights line {lineno}: missing eventId")))?
            .trim()
            .parse::<u32>()
            .map_err(|e| AppError::Parse(format!("weights line {lineno}: eventId: {e}")))?;
        let weight = it
            .next()
            .ok_or_else(|| AppError::Parse(format!("weights line {lineno}: missing weight")))?
            .trim()
            .parse::<u64>()
            .map_err(|e| AppError::Parse(format!("weights line {lineno}: weight: {e}")))?;
        let payout_multiplier = it
            .next()
            .ok_or_else(|| AppError::Parse(format!("weights line {lineno}: missing payout")))?
            .trim()
            .parse::<u32>()
            .map_err(|e| AppError::Parse(format!("weights line {lineno}: payout: {e}")))?;
        entries.push(WeightEntry {
            event_id,
            weight,
            payout_multiplier,
        });
    }

    let mut cum_weights = Vec::with_capacity(entries.len());
    let mut total: u64 = 0;
    for e in &entries {
        total = total
            .checked_add(e.weight)
            .ok_or_else(|| AppError::Parse("weights overflow u64".into()))?;
        cum_weights.push(total);
    }

    Ok(WeightSampler {
        entries,
        cum_weights,
        total_weight: total,
    })
}

fn decompress_and_index(compressed: &[u8]) -> AppResult<BooksIndex> {
    let buffer = zstd::decode_all(compressed).map_err(|e| AppError::Zstd(e.to_string()))?;

    let mut offsets = Vec::with_capacity(buffer.len() / 256 + 1);
    offsets.push(0u32);
    for (i, b) in buffer.iter().enumerate() {
        if *b == b'\n' && i + 1 < buffer.len() {
            offsets.push((i + 1) as u32);
        }
    }
    Ok(BooksIndex { buffer, offsets })
}

fn weighted_pick(sampler: &WeightSampler) -> WeightEntry {
    let mut rng = rand::thread_rng();
    let r = rng.next_u64();
    let pick = r % sampler.total_weight;
    let cw = &sampler.cum_weights;
    let mut lo = 0usize;
    let mut hi = cw.len() - 1;
    while lo < hi {
        let mid = (lo + hi) >> 1;
        if cw[mid] <= pick {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    sampler.entries[lo]
}

fn read_event(idx: &BooksIndex, event_id: u32) -> AppResult<Arc<RawValue>> {
    let i = (event_id as usize)
        .checked_sub(1)
        .ok_or_else(|| AppError::Parse("event id 0 not allowed".into()))?;
    if i >= idx.offsets.len() {
        return Err(AppError::Parse(format!(
            "event {event_id} not found ({} events)",
            idx.offsets.len()
        )));
    }
    let start = idx.offsets[i] as usize;
    let end = if i + 1 < idx.offsets.len() {
        let mut e = idx.offsets[i + 1] as usize - 1;
        while e > start && (idx.buffer[e - 1] == b'\n' || idx.buffer[e - 1] == b'\r') {
            e -= 1;
        }
        e
    } else {
        let mut e = idx.buffer.len();
        while e > start && (idx.buffer[e - 1] == b'\n' || idx.buffer[e - 1] == b'\r') {
            e -= 1;
        }
        e
    };
    let slice = &idx.buffer[start..end];

    #[derive(serde::Deserialize)]
    struct Wrapper<'a> {
        #[serde(borrow)]
        events: Option<&'a RawValue>,
    }

    let line_str =
        std::str::from_utf8(slice).map_err(|e| AppError::Parse(format!("event utf8: {e}")))?;
    let wrapper: Wrapper =
        serde_json::from_str(line_str).map_err(|e| AppError::Parse(format!("event parse: {e}")))?;
    let raw = match wrapper.events {
        Some(events) => RawValue::from_string(events.get().to_string())
            .map_err(|e| AppError::Parse(format!("event raw: {e}")))?,
        None => RawValue::from_string(line_str.to_string())
            .map_err(|e| AppError::Parse(format!("event raw: {e}")))?,
    };
    Ok(Arc::from(raw))
}
