use anyhow::{anyhow, Context, Result};
use lgs::settings::ResolutionPreset;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(rename = "gamePath")]
    pub game_path: String,
    #[serde(rename = "gameUrl")]
    pub game_url: String,
    #[serde(rename = "gameSlug")]
    pub game_slug: String,
    /// Snapshot of resolutions enabled at the time of saving. Empty = use global
    /// settings (back-compat with profiles saved before this field existed).
    #[serde(default)]
    pub resolutions: Vec<ResolutionPreset>,
    #[serde(rename = "createdAt")]
    pub created_at: u64,
    #[serde(rename = "updatedAt")]
    pub updated_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProfilesFile {
    #[serde(default)]
    profiles: Vec<Profile>,
}

fn profiles_path() -> Result<PathBuf> {
    let dir = dirs::data_local_dir()
        .ok_or_else(|| anyhow!("could not resolve local data dir"))?
        .join("stake-dev-tool");
    Ok(dir.join("profiles.json"))
}

async fn load() -> Result<ProfilesFile> {
    let path = profiles_path()?;
    if !fs::try_exists(&path).await.unwrap_or(false) {
        return Ok(ProfilesFile::default());
    }
    let bytes = fs::read(&path).await.context("read profiles.json")?;
    let parsed: ProfilesFile =
        serde_json::from_slice(&bytes).context("parse profiles.json")?;
    Ok(parsed)
}

async fn save(file: &ProfilesFile) -> Result<()> {
    let path = profiles_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await.context("create profiles dir")?;
    }
    let bytes = serde_json::to_vec_pretty(file).context("serialize profiles")?;
    fs::write(&path, bytes).await.context("write profiles.json")?;
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub async fn list() -> Result<Vec<Profile>> {
    let mut f = load().await?;
    f.profiles.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(f.profiles)
}

pub async fn upsert(
    id: Option<String>,
    name: String,
    game_path: String,
    game_url: String,
    game_slug: String,
    resolutions: Vec<ResolutionPreset>,
) -> Result<Profile> {
    let mut f = load().await?;
    let now = now_ms();

    if let Some(id) = id {
        if let Some(p) = f.profiles.iter_mut().find(|p| p.id == id) {
            p.name = name;
            p.game_path = game_path;
            p.game_url = game_url;
            p.game_slug = game_slug;
            p.resolutions = resolutions;
            p.updated_at = now;
            let updated = p.clone();
            save(&f).await?;
            return Ok(updated);
        }
    }

    let new = Profile {
        id: Uuid::new_v4().to_string(),
        name,
        game_path,
        game_url,
        game_slug,
        resolutions,
        created_at: now,
        updated_at: now,
    };
    f.profiles.push(new.clone());
    save(&f).await?;
    Ok(new)
}

pub async fn delete(id: &str) -> Result<()> {
    let mut f = load().await?;
    let before = f.profiles.len();
    f.profiles.retain(|p| p.id != id);
    if f.profiles.len() == before {
        return Err(anyhow!("profile not found"));
    }
    save(&f).await
}
