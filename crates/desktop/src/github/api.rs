use anyhow::{Context, Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::auth::load_token;

const API_BASE: &str = "https://api.github.com";

#[derive(Clone)]
pub struct GithubClient {
    http: reqwest::Client,
    token: String,
}

impl GithubClient {
    pub fn from_stored_token() -> Result<Self> {
        let token = load_token()?.ok_or_else(|| anyhow!("not signed in to GitHub"))?;
        let http = reqwest::Client::builder()
            .user_agent("stake-dev-tool")
            .timeout(Duration::from_secs(60))
            .build()
            .context("build http client")?;
        Ok(Self { http, token })
    }

    fn json_request(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
    }

    /// Create a private repo. If `org` is provided, the repo is created under
    /// that organization; otherwise under the authenticated user's account.
    pub async fn create_private_repo(
        &self,
        org: Option<&str>,
        name: &str,
        description: &str,
    ) -> Result<RepoInfo> {
        let url = match org {
            Some(o) => format!("{API_BASE}/orgs/{o}/repos"),
            None => format!("{API_BASE}/user/repos"),
        };
        let res = self
            .json_request(reqwest::Method::POST, &url)
            .json(&serde_json::json!({
                "name": name,
                "description": description,
                "private": true,
                "auto_init": true,
                "has_issues": false,
                "has_projects": false,
                "has_wiki": false
            }))
            .send()
            .await
            .context("create repo")?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("create repo: {status} {body}"));
        }
        let repo: RepoInfo = res.json().await.context("parse repo")?;
        Ok(repo)
    }

    pub async fn list_user_orgs(&self) -> Result<Vec<OrgInfo>> {
        let url = format!("{API_BASE}/user/orgs?per_page=100");
        let res = self
            .json_request(reqwest::Method::GET, &url)
            .send()
            .await
            .context("list orgs")?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("list orgs: {status} {body}"));
        }
        res.json().await.context("parse orgs")
    }

    pub async fn get_repo(&self, owner: &str, name: &str) -> Result<RepoInfo> {
        let url = format!("{API_BASE}/repos/{owner}/{name}");
        let res = self
            .json_request(reqwest::Method::GET, &url)
            .send()
            .await
            .context("get repo")?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("get repo {owner}/{name}: {status} {body}"));
        }
        res.json().await.context("parse repo")
    }

    // ============================================================
    // Git Data API — used to push many files as a single commit.
    // The Contents API forces one commit per file, which is dog slow for
    // bundle uploads (each commit is a network round-trip). Switching to
    // Git Data lets us parallelise blob uploads and stitch them into a
    // single tree + commit, matching `git push` semantics.
    // ============================================================

    pub async fn get_branch_head(
        &self,
        owner: &str,
        name: &str,
        branch: &str,
    ) -> Result<BranchHead> {
        let url = format!("{API_BASE}/repos/{owner}/{name}/branches/{branch}");
        let res = self
            .json_request(reqwest::Method::GET, &url)
            .send()
            .await
            .context("get branch head")?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("get branch {branch}: {status} {body}"));
        }
        #[derive(Deserialize)]
        struct CommitInner {
            sha: String,
            commit: CommitBody,
        }
        #[derive(Deserialize)]
        struct CommitBody {
            tree: TreeRef,
        }
        #[derive(Deserialize)]
        struct TreeRef {
            sha: String,
        }
        #[derive(Deserialize)]
        struct Resp {
            commit: CommitInner,
        }
        let r: Resp = res.json().await.context("parse branch")?;
        Ok(BranchHead {
            commit_sha: r.commit.sha,
            tree_sha: r.commit.commit.tree.sha,
        })
    }

    /// Upload `bytes` as a blob, returns its SHA. Always base64-encoded so
    /// binary files (.wasm, fonts, images, …) round-trip cleanly.
    pub async fn create_blob(
        &self,
        owner: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<String> {
        let url = format!("{API_BASE}/repos/{owner}/{name}/git/blobs");
        let res = self
            .json_request(reqwest::Method::POST, &url)
            .json(&serde_json::json!({
                "content": B64.encode(bytes),
                "encoding": "base64",
            }))
            .send()
            .await
            .context("create blob")?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("create blob: {status} {body}"));
        }
        #[derive(Deserialize)]
        struct Resp {
            sha: String,
        }
        let r: Resp = res.json().await.context("parse blob")?;
        Ok(r.sha)
    }

    /// Build a new tree on top of `base_tree`, overlaying `entries`. Each
    /// entry's `path` is relative to the repo root.
    pub async fn create_tree(
        &self,
        owner: &str,
        name: &str,
        base_tree: &str,
        entries: &[GitTreeEntry],
    ) -> Result<String> {
        let url = format!("{API_BASE}/repos/{owner}/{name}/git/trees");
        let res = self
            .json_request(reqwest::Method::POST, &url)
            .json(&serde_json::json!({
                "base_tree": base_tree,
                "tree": entries,
            }))
            .send()
            .await
            .context("create tree")?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("create tree: {status} {body}"));
        }
        #[derive(Deserialize)]
        struct Resp {
            sha: String,
        }
        let r: Resp = res.json().await.context("parse tree")?;
        Ok(r.sha)
    }

    pub async fn create_commit(
        &self,
        owner: &str,
        name: &str,
        message: &str,
        tree_sha: &str,
        parents: &[&str],
    ) -> Result<String> {
        let url = format!("{API_BASE}/repos/{owner}/{name}/git/commits");
        let res = self
            .json_request(reqwest::Method::POST, &url)
            .json(&serde_json::json!({
                "message": message,
                "tree": tree_sha,
                "parents": parents,
            }))
            .send()
            .await
            .context("create commit")?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("create commit: {status} {body}"));
        }
        #[derive(Deserialize)]
        struct Resp {
            sha: String,
        }
        let r: Resp = res.json().await.context("parse commit")?;
        Ok(r.sha)
    }

    pub async fn update_ref(
        &self,
        owner: &str,
        name: &str,
        branch: &str,
        commit_sha: &str,
    ) -> Result<()> {
        let url = format!("{API_BASE}/repos/{owner}/{name}/git/refs/heads/{branch}");
        let res = self
            .json_request(reqwest::Method::PATCH, &url)
            .json(&serde_json::json!({ "sha": commit_sha, "force": false }))
            .send()
            .await
            .context("update ref")?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("update ref {branch}: {status} {body}"));
        }
        Ok(())
    }

    /// Permanently delete a repo. Caller must have admin rights. Irreversible.
    pub async fn delete_repo(&self, owner: &str, name: &str) -> Result<()> {
        let url = format!("{API_BASE}/repos/{owner}/{name}");
        let res = self
            .json_request(reqwest::Method::DELETE, &url)
            .send()
            .await
            .context("delete repo")?;
        let status = res.status();
        if status == reqwest::StatusCode::NO_CONTENT {
            return Ok(());
        }
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("delete repo {owner}/{name}: {status} {body}"));
        }
        Ok(())
    }

    pub async fn invite_collaborator(
        &self,
        owner: &str,
        name: &str,
        username: &str,
    ) -> Result<()> {
        let url = format!("{API_BASE}/repos/{owner}/{name}/collaborators/{username}");
        let res = self
            .json_request(reqwest::Method::PUT, &url)
            .json(&serde_json::json!({ "permission": "push" }))
            .send()
            .await
            .context("invite collaborator")?;
        let status = res.status();
        // 201 = new invite, 204 = already collaborator
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("invite {username}: {status} {body}"));
        }
        Ok(())
    }

    pub async fn list_team_repos(&self, marker_topic: &str) -> Result<Vec<RepoInfo>> {
        // GitHub's search API restricted to repos the authenticated user can
        // access that carry a specific topic. We stamp "stake-dev-tool-team"
        // on every repo we create so users only see the relevant ones.
        //
        // Pagination is required: per_page maxes out at 100 and search caps at
        // 1000 results / 10 pages. Sorting by `updated` keeps the truncation
        // (if any) deterministic across requests instead of relying on the
        // default relevance ordering.
        #[derive(Deserialize)]
        struct SearchResponse {
            #[serde(default)]
            total_count: u64,
            items: Vec<RepoInfo>,
        }
        let mut all = Vec::new();
        let mut page: u32 = 1;
        loop {
            let url = format!(
                "{API_BASE}/search/repositories?q=topic:{marker_topic}+is:private+user:@me&sort=updated&per_page=100&page={page}"
            );
            let res = self
                .json_request(reqwest::Method::GET, &url)
                .send()
                .await
                .context("search team repos")?;
            let status = res.status();
            if !status.is_success() {
                let body = res.text().await.unwrap_or_default();
                return Err(anyhow!("search repos: {status} {body}"));
            }
            let parsed: SearchResponse = res.json().await.context("parse search")?;
            let returned = parsed.items.len();
            all.extend(parsed.items);
            // Stop when the page is short (no more results) or we've hit the
            // search-API hard cap of 1000 results / 10 pages.
            if returned < 100 || page >= 10 {
                if parsed.total_count as usize > all.len() {
                    tracing::warn!(
                        total = parsed.total_count,
                        returned = all.len(),
                        "list_team_repos: results exceed search-API cap; some teams will not appear"
                    );
                }
                break;
            }
            page += 1;
        }
        Ok(all)
    }

    pub async fn set_repo_topics(
        &self,
        owner: &str,
        name: &str,
        topics: &[&str],
    ) -> Result<()> {
        let url = format!("{API_BASE}/repos/{owner}/{name}/topics");
        let res = self
            .json_request(reqwest::Method::PUT, &url)
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({ "names": topics }))
            .send()
            .await
            .context("set topics")?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("set topics: {status} {body}"));
        }
        Ok(())
    }

    /// Fetch a file's content + SHA. Returns None if the file doesn't exist.
    pub async fn get_file(
        &self,
        owner: &str,
        name: &str,
        path: &str,
    ) -> Result<Option<RepoFile>> {
        let url = format!("{API_BASE}/repos/{owner}/{name}/contents/{path}");
        let res = self
            .json_request(reqwest::Method::GET, &url)
            .send()
            .await
            .context("get file")?;
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("get file {path}: {status} {body}"));
        }
        let raw: RepoFileRaw = res.json().await.context("parse file")?;
        let content = B64
            .decode(raw.content.replace('\n', ""))
            .context("decode file content")?;
        Ok(Some(RepoFile {
            sha: raw.sha,
            content,
        }))
    }

    /// List directory entries. Returns empty vec if the directory doesn't exist.
    pub async fn list_dir(
        &self,
        owner: &str,
        name: &str,
        path: &str,
    ) -> Result<Vec<RepoEntry>> {
        let url = format!("{API_BASE}/repos/{owner}/{name}/contents/{path}");
        let res = self
            .json_request(reqwest::Method::GET, &url)
            .send()
            .await
            .context("list dir")?;
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("list dir {path}: {status} {body}"));
        }
        // Can be a single object (file) or array (dir)
        let v: serde_json::Value = res.json().await.context("parse dir listing")?;
        let items = if v.is_array() {
            serde_json::from_value::<Vec<RepoEntry>>(v).context("parse entries")?
        } else {
            Vec::new()
        };
        Ok(items)
    }

    /// Create or update a file at `path`. Pass `prev_sha` when updating.
    /// Returns the SHA of the newly-written file.
    pub async fn put_file(
        &self,
        owner: &str,
        name: &str,
        path: &str,
        content: &[u8],
        message: &str,
        prev_sha: Option<&str>,
    ) -> Result<String> {
        let url = format!("{API_BASE}/repos/{owner}/{name}/contents/{path}");
        let mut body = serde_json::json!({
            "message": message,
            "content": B64.encode(content),
        });
        if let Some(sha) = prev_sha {
            body["sha"] = serde_json::Value::String(sha.to_string());
        }
        let res = self
            .json_request(reqwest::Method::PUT, &url)
            .json(&body)
            .send()
            .await
            .context("put file")?;
        let status = res.status();
        if !status.is_success() {
            let body_text = res.text().await.unwrap_or_default();
            return Err(anyhow!("put file {path}: {status} {body_text}"));
        }
        // The PUT /contents response has a different shape than the GET
        // response: `content` here is file metadata (sha, size, url, …) with
        // no `content` field because we just sent the body.
        #[derive(Deserialize)]
        struct PutContent {
            sha: String,
        }
        #[derive(Deserialize)]
        struct PutResponse {
            content: PutContent,
        }
        let parsed: PutResponse = res.json().await.context("parse put response")?;
        Ok(parsed.content.sha)
    }

    pub async fn delete_file(
        &self,
        owner: &str,
        name: &str,
        path: &str,
        sha: &str,
        message: &str,
    ) -> Result<()> {
        let url = format!("{API_BASE}/repos/{owner}/{name}/contents/{path}");
        let res = self
            .json_request(reqwest::Method::DELETE, &url)
            .json(&serde_json::json!({ "message": message, "sha": sha }))
            .send()
            .await
            .context("delete file")?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("delete file {path}: {status} {body}"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub html_url: String,
    pub owner: RepoOwner,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub default_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoOwner {
    pub login: String,
    pub id: u64,
}

#[derive(Debug, Clone)]
pub struct BranchHead {
    pub commit_sha: String,
    pub tree_sha: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GitTreeEntry {
    pub path: String,
    pub mode: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub sha: String,
}

impl GitTreeEntry {
    pub fn blob(path: String, sha: String) -> Self {
        Self {
            path,
            mode: "100644".to_string(),
            kind: "blob".to_string(),
            sha,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgInfo {
    pub login: String,
    pub id: u64,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RepoFile {
    #[allow(dead_code)]
    pub sha: String,
    pub content: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
struct RepoFileRaw {
    sha: String,
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoEntry {
    pub name: String,
    pub path: String,
    pub sha: String,
    pub size: u64,
    #[serde(rename = "type")]
    pub kind: String,
}

// ============================================================
// Releases + asset uploads (used for math file sync)
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub id: u64,
    pub tag_name: String,
    pub name: String,
    pub draft: bool,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
    pub upload_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub url: String,
    #[serde(default)]
    pub browser_download_url: String,
}

impl GithubClient {
    pub async fn find_release_by_tag(
        &self,
        owner: &str,
        name: &str,
        tag: &str,
    ) -> Result<Option<Release>> {
        let url = format!("{API_BASE}/repos/{owner}/{name}/releases/tags/{tag}");
        let res = self
            .json_request(reqwest::Method::GET, &url)
            .send()
            .await
            .context("get release by tag")?;
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("get release: {status} {body}"));
        }
        let release: Release = res.json().await.context("parse release")?;
        Ok(Some(release))
    }

    pub async fn create_release(
        &self,
        owner: &str,
        name: &str,
        tag: &str,
        title: &str,
        body: &str,
    ) -> Result<Release> {
        let url = format!("{API_BASE}/repos/{owner}/{name}/releases");
        let res = self
            .json_request(reqwest::Method::POST, &url)
            .json(&serde_json::json!({
                "tag_name": tag,
                "name": title,
                "body": body,
                "draft": false,
                "prerelease": false
            }))
            .send()
            .await
            .context("create release")?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("create release: {status} {body}"));
        }
        res.json().await.context("parse release")
    }

    pub async fn delete_release_asset(&self, owner: &str, name: &str, asset_id: u64) -> Result<()> {
        let url = format!("{API_BASE}/repos/{owner}/{name}/releases/assets/{asset_id}");
        let res = self
            .json_request(reqwest::Method::DELETE, &url)
            .send()
            .await
            .context("delete asset")?;
        let status = res.status();
        if !status.is_success() && status != reqwest::StatusCode::NOT_FOUND {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("delete asset: {status} {body}"));
        }
        Ok(())
    }

    /// Upload a single file as a release asset. `upload_url` is the `upload_url`
    /// field from the Release object (with the `{?name,label}` template
    /// trimmed). The Content-Type is `application/octet-stream`.
    pub async fn upload_release_asset(
        &self,
        upload_url: &str,
        asset_name: &str,
        bytes: Vec<u8>,
    ) -> Result<ReleaseAsset> {
        // Strip the `{?name,label}` template suffix added by GitHub.
        let base = match upload_url.find('{') {
            Some(i) => &upload_url[..i],
            None => upload_url,
        };
        let url = format!("{base}?name={}", urlencoding_encode(asset_name));
        let res = self
            .http
            .post(&url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("Content-Type", "application/octet-stream")
            .body(bytes)
            .send()
            .await
            .context("upload asset")?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("upload {asset_name}: {status} {body}"));
        }
        res.json().await.context("parse uploaded asset")
    }

    /// Download a release asset by its API url. Uses `Accept: application/octet-stream`
    /// which causes GitHub to redirect to the signed S3 URL — reqwest follows
    /// redirects by default.
    pub async fn download_release_asset(&self, asset_url: &str) -> Result<Vec<u8>> {
        let res = self
            .http
            .get(asset_url)
            .bearer_auth(&self.token)
            .header("Accept", "application/octet-stream")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .context("download asset")?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("download asset: {status} {body}"));
        }
        let bytes = res.bytes().await.context("read asset bytes")?;
        Ok(bytes.to_vec())
    }
}

/// Minimal URL component encoder (we only need it for asset names which
/// typically contain `.` and maybe `-` / alphanum).
fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' || ch == '~' {
            out.push(ch);
        } else {
            for b in ch.to_string().as_bytes() {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}
