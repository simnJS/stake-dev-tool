use anyhow::{Context, Result, anyhow};
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
    /// The team this profile was pulled from, if any. `None` for profiles the
    /// user created locally. Enables the UI to group/filter by origin and
    /// show a "from team X" badge.
    #[serde(default, rename = "teamId")]
    pub team_id: Option<String>,
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
    let parsed: ProfilesFile = serde_json::from_slice(&bytes).context("parse profiles.json")?;
    Ok(parsed)
}

async fn save(file: &ProfilesFile) -> Result<()> {
    let path = profiles_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .context("create profiles dir")?;
    }
    let bytes = serde_json::to_vec_pretty(file).context("serialize profiles")?;
    fs::write(&path, bytes)
        .await
        .context("write profiles.json")?;
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
    f.profiles.sort_by_key(|p| std::cmp::Reverse(p.updated_at));
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

    let existing = id
        .as_ref()
        .and_then(|id| f.profiles.iter_mut().find(|p| &p.id == id));
    if let Some(p) = existing {
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

    let new = Profile {
        id: Uuid::new_v4().to_string(),
        name,
        game_path,
        game_url,
        game_slug,
        resolutions,
        created_at: now,
        updated_at: now,
        team_id: None,
    };
    f.profiles.push(new.clone());
    save(&f).await?;
    Ok(new)
}

/// Insert or replace a full profile record. Used by team sync to apply
/// remote changes without losing timestamps or IDs.
pub async fn upsert_raw(profile: Profile) -> Result<Profile> {
    let mut f = load().await?;
    if let Some(existing) = f.profiles.iter_mut().find(|p| p.id == profile.id) {
        *existing = profile.clone();
    } else {
        f.profiles.push(profile.clone());
    }
    save(&f).await?;
    Ok(profile)
}

/// Stamp a profile with its team of origin. Used right after pushing to a
/// team so the UI moves the profile from the "Mine" group to that team's
/// group and exposes the "Pull latest" action.
pub async fn set_team(profile_id: &str, team_id: Option<&str>) -> Result<()> {
    let mut f = load().await?;
    let Some(p) = f.profiles.iter_mut().find(|p| p.id == profile_id) else {
        return Err(anyhow!("profile not found"));
    };
    p.team_id = team_id.map(|s| s.to_string());
    save(&f).await
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
