use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedRound {
    pub id: String,
    #[serde(rename = "gameSlug")]
    pub game_slug: String,
    pub mode: String,
    #[serde(rename = "eventId")]
    pub event_id: u32,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "createdAt")]
    pub created_at: u64,
    #[serde(rename = "updatedAt")]
    pub updated_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SavedRoundsFile {
    #[serde(default)]
    rounds: Vec<SavedRound>,
}

fn file_path() -> Result<PathBuf> {
    let dir = dirs::data_local_dir()
        .ok_or_else(|| anyhow!("could not resolve local data dir"))?
        .join("stake-dev-tool");
    Ok(dir.join("saved-rounds.json"))
}

async fn load() -> Result<SavedRoundsFile> {
    let path = file_path()?;
    if !fs::try_exists(&path).await.unwrap_or(false) {
        return Ok(SavedRoundsFile::default());
    }
    let bytes = fs::read(&path).await.context("read saved-rounds.json")?;
    let parsed: SavedRoundsFile =
        serde_json::from_slice(&bytes).context("parse saved-rounds.json")?;
    Ok(parsed)
}

async fn save(file: &SavedRoundsFile) -> Result<()> {
    let path = file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .context("create saved-rounds dir")?;
    }
    let bytes = serde_json::to_vec_pretty(file).context("serialize saved-rounds")?;
    fs::write(&path, bytes)
        .await
        .context("write saved-rounds.json")?;
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub async fn list(game_slug: Option<&str>) -> Result<Vec<SavedRound>> {
    let mut f = load().await?;
    if let Some(slug) = game_slug {
        f.rounds.retain(|r| r.game_slug == slug);
    }
    f.rounds.sort_by_key(|r| std::cmp::Reverse(r.updated_at));
    Ok(f.rounds)
}

pub async fn create(
    game_slug: String,
    mode: String,
    event_id: u32,
    description: String,
) -> Result<SavedRound> {
    if game_slug.is_empty() {
        return Err(anyhow!("gameSlug is required"));
    }
    if mode.is_empty() {
        return Err(anyhow!("mode is required"));
    }
    if event_id == 0 {
        return Err(anyhow!("eventId must be > 0"));
    }
    let mut f = load().await?;
    let now = now_ms();
    let round = SavedRound {
        id: Uuid::new_v4().to_string(),
        game_slug,
        mode,
        event_id,
        description,
        created_at: now,
        updated_at: now,
    };
    f.rounds.push(round.clone());
    save(&f).await?;
    Ok(round)
}

pub async fn update_description(id: &str, description: String) -> Result<SavedRound> {
    let mut f = load().await?;
    let round = f
        .rounds
        .iter_mut()
        .find(|r| r.id == id)
        .ok_or_else(|| anyhow!("saved round not found"))?;
    round.description = description;
    round.updated_at = now_ms();
    let updated = round.clone();
    save(&f).await?;
    Ok(updated)
}

pub async fn delete(id: &str) -> Result<()> {
    let mut f = load().await?;
    let before = f.rounds.len();
    f.rounds.retain(|r| r.id != id);
    if f.rounds.len() == before {
        return Err(anyhow!("saved round not found"));
    }
    save(&f).await
}
