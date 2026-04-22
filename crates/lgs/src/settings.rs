use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionPreset {
    pub id: String,
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub enabled: bool,
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub resolutions: Vec<ResolutionPreset>,
}

// Default-enabled set kept minimal (one desktop + one mobile) so a first
// launch doesn't spin up 7 WebGL contexts in parallel. Users enable the
// rest on demand via the sidebar. Adjusting `enabled_by_default` for a
// builtin only affects brand-new installs — existing users' settings are
// preserved by `load()`'s self-heal path.
const BUILTIN: &[(&str, &str, u32, u32, bool)] = &[
    ("desktop", "Desktop", 1200, 675, true),
    ("laptop", "Laptop", 1024, 576, false),
    ("popout-l", "Popout L", 800, 450, false),
    ("popout-s", "Popout S", 400, 225, false),
    ("mobile-l", "Mobile L", 425, 821, false),
    ("mobile-m", "Mobile M", 375, 667, true),
    ("mobile-s", "Mobile S", 320, 568, false),
];

impl Default for Settings {
    fn default() -> Self {
        Self {
            resolutions: BUILTIN
                .iter()
                .map(|(id, label, w, h, enabled)| ResolutionPreset {
                    id: (*id).to_string(),
                    label: (*label).to_string(),
                    width: *w,
                    height: *h,
                    enabled: *enabled,
                    builtin: true,
                })
                .collect(),
        }
    }
}

fn settings_path() -> Result<PathBuf> {
    let dir = dirs::data_local_dir()
        .ok_or_else(|| anyhow!("could not resolve local data dir"))?
        .join("stake-dev-tool");
    Ok(dir.join("settings.json"))
}

pub async fn load() -> Result<Settings> {
    let path = settings_path()?;
    if !fs::try_exists(&path).await.unwrap_or(false) {
        return Ok(Settings::default());
    }
    let bytes = fs::read(&path).await.context("read settings.json")?;
    let mut parsed: Settings = serde_json::from_slice(&bytes).context("parse settings.json")?;

    // Self-heal: ensure all builtins exist (if a new release adds new defaults).
    // New builtins inherit their default `enabled_by_default`; existing ones
    // keep whatever the user set.
    for (id, label, w, h, enabled) in BUILTIN {
        if !parsed.resolutions.iter().any(|r| r.id == *id) {
            parsed.resolutions.push(ResolutionPreset {
                id: (*id).to_string(),
                label: (*label).to_string(),
                width: *w,
                height: *h,
                enabled: *enabled,
                builtin: true,
            });
        }
    }
    Ok(parsed)
}

async fn save(settings: &Settings) -> Result<()> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let bytes = serde_json::to_vec_pretty(settings).context("serialize settings")?;
    fs::write(&path, bytes)
        .await
        .context("write settings.json")?;
    Ok(())
}

pub async fn toggle(id: &str, enabled: bool) -> Result<Settings> {
    let mut s = load().await?;
    let r = s
        .resolutions
        .iter_mut()
        .find(|r| r.id == id)
        .ok_or_else(|| anyhow!("resolution not found"))?;
    r.enabled = enabled;
    save(&s).await?;
    Ok(s)
}

pub async fn add_custom(label: String, width: u32, height: u32) -> Result<Settings> {
    if width == 0 || height == 0 {
        return Err(anyhow!("width and height must be > 0"));
    }
    if width > 4096 || height > 4096 {
        return Err(anyhow!("width and height must be ≤ 4096"));
    }
    let mut s = load().await?;
    let id = format!("custom-{}", uuid::Uuid::new_v4());
    s.resolutions.push(ResolutionPreset {
        id,
        label,
        width,
        height,
        enabled: true,
        builtin: false,
    });
    save(&s).await?;
    Ok(s)
}

pub async fn replace_all(resolutions: Vec<ResolutionPreset>) -> Result<Settings> {
    let s = Settings { resolutions };
    save(&s).await?;
    Ok(s)
}

pub async fn delete_custom(id: &str) -> Result<Settings> {
    let mut s = load().await?;
    let before = s.resolutions.len();
    s.resolutions.retain(|r| r.id != id || r.builtin);
    if s.resolutions.len() == before {
        return Err(anyhow!(
            "custom resolution not found (cannot delete builtins)"
        ));
    }
    save(&s).await?;
    Ok(s)
}
