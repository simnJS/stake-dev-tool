use anyhow::{Context, Result, anyhow};
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, KeyUsagePurpose,
    SanType,
};
use std::path::{Path, PathBuf};
use time::{Duration, OffsetDateTime};
use tokio::fs;

pub const ROOT_CA_NAME: &str = "Stake Dev Tool Local CA";
pub const ROOT_CA_ORG: &str = "Stake Dev Tool";

pub struct CertBundle {
    pub cert_pem: String,
    pub key_pem: String,
}

pub struct LocalCa {
    pub dir: PathBuf,
    pub ca_cert_pem: String,
    pub leaf_cert_pem: String,
    pub leaf_key_pem: String,
}

impl LocalCa {
    pub async fn load_or_create() -> Result<Self> {
        let dir = data_dir()?;
        fs::create_dir_all(&dir).await.context("create cert dir")?;

        let ca_cert_path = dir.join("rootCA.pem");
        let ca_key_path = dir.join("rootCA.key.pem");
        let leaf_cert_path = dir.join("localhost.pem");
        let leaf_key_path = dir.join("localhost.key.pem");

        let all_exist =
            futures_all_exist(&[&ca_cert_path, &ca_key_path, &leaf_cert_path, &leaf_key_path])
                .await;

        if all_exist {
            return Ok(Self {
                dir,
                ca_cert_pem: fs::read_to_string(&ca_cert_path).await?,
                leaf_cert_pem: fs::read_to_string(&leaf_cert_path).await?,
                leaf_key_pem: fs::read_to_string(&leaf_key_path).await?,
            });
        }

        let (ca_cert_pem, ca_key_pem, leaf_cert_pem, leaf_key_pem) = generate_ca_and_leaf()?;

        fs::write(&ca_cert_path, &ca_cert_pem).await?;
        fs::write(&ca_key_path, &ca_key_pem).await?;
        fs::write(&leaf_cert_path, &leaf_cert_pem).await?;
        fs::write(&leaf_key_path, &leaf_key_pem).await?;

        Ok(Self {
            dir,
            ca_cert_pem,
            leaf_cert_pem,
            leaf_key_pem,
        })
    }

    pub fn ca_cert_path(&self) -> PathBuf {
        self.dir.join("rootCA.pem")
    }

    pub fn leaf_bundle(&self) -> CertBundle {
        CertBundle {
            cert_pem: self.leaf_cert_pem.clone(),
            key_pem: self.leaf_key_pem.clone(),
        }
    }
}

async fn futures_all_exist(paths: &[&PathBuf]) -> bool {
    for p in paths {
        if !fs::try_exists(p).await.unwrap_or(false) {
            return false;
        }
    }
    true
}

fn data_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir().ok_or_else(|| anyhow!("could not resolve local data dir"))?;
    Ok(base.join("stake-dev-tool").join("certs"))
}

fn generate_ca_and_leaf() -> Result<(String, String, String, String)> {
    // ----- Root CA -----
    let mut ca_params = CertificateParams::new(Vec::<String>::new())?;
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    ca_params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
    ];
    let mut ca_dn = DistinguishedName::new();
    ca_dn.push(DnType::CommonName, ROOT_CA_NAME);
    ca_dn.push(DnType::OrganizationName, ROOT_CA_ORG);
    ca_params.distinguished_name = ca_dn;
    ca_params.not_before = OffsetDateTime::now_utc() - Duration::days(1);
    ca_params.not_after = OffsetDateTime::now_utc() + Duration::days(365 * 10);

    let ca_key = KeyPair::generate()?;
    let ca_cert = ca_params.self_signed(&ca_key)?;

    // ----- Leaf cert for localhost -----
    let mut leaf_params =
        CertificateParams::new(vec!["localhost".to_string(), "127.0.0.1".to_string()])?;
    leaf_params.subject_alt_names = vec![
        SanType::DnsName("localhost".try_into()?),
        SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))),
    ];
    let mut leaf_dn = DistinguishedName::new();
    leaf_dn.push(DnType::CommonName, "localhost");
    leaf_params.distinguished_name = leaf_dn;
    leaf_params.not_before = OffsetDateTime::now_utc() - Duration::days(1);
    leaf_params.not_after = OffsetDateTime::now_utc() + Duration::days(365 * 5);

    let leaf_key = KeyPair::generate()?;
    let leaf_cert = leaf_params.signed_by(&leaf_key, &ca_cert, &ca_key)?;

    Ok((
        ca_cert.pem(),
        ca_key.serialize_pem(),
        leaf_cert.pem(),
        leaf_key.serialize_pem(),
    ))
}

// ============================================================
// Per-OS trust store integration — user-level, no sudo/UAC.
// ============================================================

// ---------- Windows (user "Root" via certutil) ----------

#[cfg(windows)]
pub fn is_ca_installed_user_store() -> bool {
    use std::process::Command;
    let out = Command::new("certutil")
        .args(["-user", "-store", "Root", ROOT_CA_NAME])
        .output();
    match out {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            o.status.success() && !stdout.contains("No certificates")
        }
        Err(_) => false,
    }
}

#[cfg(windows)]
pub fn install_ca_user_store(ca_pem_path: &Path) -> Result<()> {
    use std::process::Command;
    let out = Command::new("certutil")
        .args([
            "-user",
            "-addstore",
            "Root",
            ca_pem_path
                .to_str()
                .ok_or_else(|| anyhow!("invalid path"))?,
        ])
        .output()
        .context("failed to spawn certutil")?;
    if !out.status.success() {
        return Err(anyhow!(
            "certutil failed: {}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

#[cfg(windows)]
pub fn uninstall_ca_user_store() -> Result<()> {
    use std::process::Command;
    let out = Command::new("certutil")
        .args(["-user", "-delstore", "Root", ROOT_CA_NAME])
        .output()
        .context("failed to spawn certutil")?;
    if !out.status.success() {
        return Err(anyhow!(
            "certutil failed: {}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

// ---------- macOS (login keychain via /usr/bin/security) ----------
// Covers Safari, Chrome, Edge. Firefox has its own NSS store.

#[cfg(target_os = "macos")]
fn home_login_keychain() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("no home dir"))?;
    Ok(home
        .join("Library")
        .join("Keychains")
        .join("login.keychain-db"))
}

#[cfg(target_os = "macos")]
pub fn is_ca_installed_user_store() -> bool {
    use std::process::Command;
    let Ok(kc) = home_login_keychain() else {
        return false;
    };
    Command::new("security")
        .args(["find-certificate", "-c", ROOT_CA_NAME])
        .arg(&kc)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
pub fn install_ca_user_store(ca_pem_path: &Path) -> Result<()> {
    use std::process::Command;
    let kc = home_login_keychain()?;
    // `security add-trusted-cert -r trustRoot -k <login.keychain-db> cert.pem`
    // Prompts the user for their keychain password (GUI). No sudo needed.
    let out = Command::new("security")
        .args(["add-trusted-cert", "-r", "trustRoot", "-k"])
        .arg(&kc)
        .arg(ca_pem_path)
        .output()
        .context("failed to spawn /usr/bin/security")?;
    if !out.status.success() {
        return Err(anyhow!(
            "security add-trusted-cert failed: {}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn uninstall_ca_user_store() -> Result<()> {
    use std::process::Command;
    let kc = home_login_keychain()?;
    let out = Command::new("security")
        .args(["delete-certificate", "-c", ROOT_CA_NAME])
        .arg(&kc)
        .output()
        .context("failed to spawn /usr/bin/security")?;
    if !out.status.success() {
        return Err(anyhow!(
            "security delete-certificate failed: {}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

// ---------- Linux (user NSS database via certutil) ----------
// Covers Chromium-family browsers (Chrome, Edge, Brave, Vivaldi).
// Firefox uses a separate NSS store per profile — user must trust manually.
// Requires `libnss3-tools` (Debian) / `nss-tools` (Fedora) installed.

#[cfg(target_os = "linux")]
fn nssdb_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".pki")
        .join("nssdb")
}

#[cfg(target_os = "linux")]
fn nssdb_arg() -> String {
    format!("sql:{}", nssdb_dir().display())
}

#[cfg(target_os = "linux")]
pub fn is_ca_installed_user_store() -> bool {
    use std::process::Command;
    if !nssdb_dir().exists() {
        return false;
    }
    Command::new("certutil")
        .args(["-L", "-d", &nssdb_arg(), "-n", ROOT_CA_NAME])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
pub fn install_ca_user_store(ca_pem_path: &Path) -> Result<()> {
    use std::process::Command;
    let db = nssdb_dir();
    if !db.exists() {
        std::fs::create_dir_all(&db).context("create NSS db dir")?;
        // Init the NSS DB with an empty password.
        let init = Command::new("certutil")
            .args(["-N", "-d", &nssdb_arg(), "--empty-password"])
            .output()
            .context(
                "failed to spawn certutil — install `libnss3-tools` (Debian/Ubuntu) \
                 or `nss-tools` (Fedora/RHEL)",
            )?;
        if !init.status.success() {
            return Err(anyhow!(
                "certutil -N (init NSS db) failed: {}",
                String::from_utf8_lossy(&init.stderr)
            ));
        }
    }
    let out = Command::new("certutil")
        .args([
            "-A",
            "-d",
            &nssdb_arg(),
            "-t",
            "C,,",
            "-n",
            ROOT_CA_NAME,
            "-i",
        ])
        .arg(ca_pem_path)
        .output()
        .context(
            "failed to spawn certutil — install `libnss3-tools` (Debian/Ubuntu) \
             or `nss-tools` (Fedora/RHEL)",
        )?;
    if !out.status.success() {
        return Err(anyhow!(
            "certutil -A failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn uninstall_ca_user_store() -> Result<()> {
    use std::process::Command;
    if !nssdb_dir().exists() {
        return Ok(());
    }
    let out = Command::new("certutil")
        .args(["-D", "-d", &nssdb_arg(), "-n", ROOT_CA_NAME])
        .output()
        .context("failed to spawn certutil")?;
    if !out.status.success() {
        return Err(anyhow!(
            "certutil -D failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

// ---------- Fallback for unsupported OS ----------

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn is_ca_installed_user_store() -> bool {
    false
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn install_ca_user_store(_ca_pem_path: &Path) -> Result<()> {
    Err(anyhow!("CA install not implemented for this platform"))
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn uninstall_ca_user_store() -> Result<()> {
    Err(anyhow!("CA uninstall not implemented for this platform"))
}
