//! Split a file into fixed-size chunks on disk. Used by the preview
//! publisher because GitHub Pages serves any file under 100 MB, but
//! Release assets — even though they accept multi-GB files — go through a
//! redirect chain that strips CORS headers, so a browser fetch from
//! `*.github.io` is blocked. Same-origin Pages serving sidesteps the whole
//! mess.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

/// Max bytes per emitted chunk file. The blob API documents a 100 MB cap
/// but rejects requests well before that with `422 too large to process` —
/// observed cutoff is around 30-40 MB raw. 10 MB raw → ~14 MB base64 body
/// in a JSON wrapper, comfortably under whatever GitHub's actual ceiling
/// is. Smaller means more API calls but each one finishes in a couple
/// seconds, and the 5000-req/hr quota is still huge headroom.
pub const CHUNK_SIZE: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedFile {
    /// Path relative to the original math root (e.g. `books_base.jsonl.zst`).
    pub name: String,
    pub size: u64,
    pub sha256: String,
    pub chunks: Vec<StagedChunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedChunk {
    /// File name written under `dst_chunks_dir`. Has the original file name
    /// as prefix + a `.partNNN.<short-sha>` suffix so the chunk is uniquely
    /// addressable and idempotent across re-publishes.
    #[serde(rename = "fileName")]
    pub file_name: String,
    /// URL the runtime should fetch — relative to the bundle root so the
    /// browser stays same-origin.
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

/// For each file in `srcs`, hash + split into `CHUNK_SIZE`-byte parts and
/// write the parts to `dst_chunks_dir` (created if missing). Returns a
/// manifest the caller bakes into the bundle. Each chunk's `url` is the
/// path the runtime will fetch, prefixed with `url_base_rel` (a path
/// relative to the bundle root, e.g. `./math/chunks`).
pub async fn split_to_dir(
    src_dir: &Path,
    dst_chunks_dir: &Path,
    files_rel: &[String],
    url_base_rel: &str,
) -> Result<Vec<StagedFile>> {
    fs::create_dir_all(dst_chunks_dir).await?;
    let mut out = Vec::with_capacity(files_rel.len());
    for rel in files_rel {
        let src = src_dir.join(rel);
        let staged = split_one(&src, dst_chunks_dir, rel, url_base_rel)
            .await
            .with_context(|| format!("split {rel}"))?;
        out.push(staged);
    }
    Ok(out)
}

async fn split_one(
    src: &Path,
    dst_chunks_dir: &Path,
    rel_name: &str,
    url_base_rel: &str,
) -> Result<StagedFile> {
    let meta = fs::metadata(src).await?;
    let size = meta.len();
    let total_parts = if size == 0 { 1 } else { size.div_ceil(CHUNK_SIZE) };

    // First pass: compute whole-file SHA + per-chunk SHA + name.
    let mut file = fs::File::open(src).await?;
    let mut whole = Sha256::new();
    let mut chunk_metas: Vec<(String, u64)> = Vec::with_capacity(total_parts as usize);
    let mut buf = vec![0u8; 1024 * 1024];

    for part_ix in 0..total_parts {
        let mut hasher = Sha256::new();
        let mut read_total: u64 = 0;
        let target = if part_ix + 1 == total_parts {
            size - part_ix * CHUNK_SIZE
        } else {
            CHUNK_SIZE
        };
        while read_total < target {
            let to_read = ((target - read_total) as usize).min(buf.len());
            let n = file.read(&mut buf[..to_read]).await?;
            if n == 0 {
                break;
            }
            whole.update(&buf[..n]);
            hasher.update(&buf[..n]);
            read_total += n as u64;
        }
        chunk_metas.push((hex(&hasher.finalize()), read_total));
    }
    let whole_sha = hex(&whole.finalize());

    // Second pass: write chunks.
    let mut chunks: Vec<StagedChunk> = Vec::with_capacity(chunk_metas.len());
    let safe = sanitize(rel_name);
    file.seek(std::io::SeekFrom::Start(0)).await?;
    for (ix, (chunk_sha, chunk_size)) in chunk_metas.iter().enumerate() {
        let short = &chunk_sha[..8.min(chunk_sha.len())];
        let chunk_file_name = if total_parts == 1 {
            format!("{safe}.{short}")
        } else {
            format!("{safe}.part{:03}.{short}", ix + 1)
        };
        let chunk_path = dst_chunks_dir.join(&chunk_file_name);
        let mut out = fs::File::create(&chunk_path).await?;
        let mut left = *chunk_size;
        while left > 0 {
            let to_read = (left as usize).min(buf.len());
            let n = file.read(&mut buf[..to_read]).await?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).await?;
            left -= n as u64;
        }
        out.flush().await?;
        chunks.push(StagedChunk {
            file_name: chunk_file_name.clone(),
            url: format!("{}/{}", url_base_rel.trim_end_matches('/'), chunk_file_name),
            size: *chunk_size,
            sha256: chunk_sha.clone(),
        });
    }

    Ok(StagedFile {
        name: rel_name.to_string(),
        size,
        sha256: whole_sha,
        chunks,
    })
}

fn sanitize(rel: &str) -> String {
    rel.chars()
        .map(|c| if c == '/' || c == '\\' { '.' } else { c })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}
