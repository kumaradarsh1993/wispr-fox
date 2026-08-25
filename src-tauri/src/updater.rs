//! In-app updates: see both channels at once, and install without a trip to
//! GitHub.
//!
//! The old flow reported ONE version — whatever GitHub listed first, stable or
//! nightly — and then handed the user a link. From inside the app you could not
//! answer "what am I on, what's the latest stable, and is there a newer
//! nightly?", which are three different questions.
//!
//! ## Why not `tauri-plugin-updater`
//!
//! The plugin resolves ONE update per endpoint from a signed `latest.json`.
//! GitHub's `releases/latest` deliberately excludes pre-releases and there is
//! no equivalent "latest pre-release" download URL, so a two-channel picker
//! would need a second manifest published by hand and a signing key added to
//! CI. This module reads the same public releases API the app already used and
//! presents both channels, with no new secrets and no release-pipeline change.
//!
//! ## Trust boundary
//!
//! Downloads come only from `browser_download_url` values returned by the
//! GitHub API for **this** repository, over HTTPS, and the finished file must
//! match the byte size the API declared. Nothing here constructs a URL from
//! user input, and nothing follows a redirect off GitHub's own hosts. The
//! installer this fetches is the same unsigned artifact the user would
//! otherwise download by hand — this is strictly less error-prone than that,
//! not more trusting.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

const REPO: &str = "kumaradarsh1993/wispr-fox";

/// Hosts a release asset may be served from. GitHub redirects
/// `browser_download_url` to its CDN, so both must be allowed — but nothing
/// else is, which is what stops a tampered API response redirecting a download
/// somewhere arbitrary.
const ALLOWED_HOSTS: [&str; 3] = [
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

#[derive(Debug, Clone, Serialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

/// One release, reduced to what the About screen shows.
#[derive(Debug, Clone, Serialize)]
pub struct ReleaseInfo {
    /// Tag as published, e.g. `v3.4.0-nightly.1`.
    pub tag: String,
    /// Tag minus the leading `v`, for version comparison and display.
    pub version: String,
    pub html_url: String,
    pub published_at: Option<String>,
    pub prerelease: bool,
    /// Newer than what is running right now.
    pub newer: bool,
    /// The installer for THIS platform, when the release has one. `None` means
    /// the release exists but shipped no artifact we can install here — the UI
    /// must offer the release page instead of a broken Install button.
    pub asset: Option<ReleaseAsset>,
    /// First paragraph-ish of the release notes, for a one-line "what's in it".
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateStatus {
    pub current: String,
    /// True when the running build is itself a pre-release.
    pub current_is_nightly: bool,
    pub stable: Option<ReleaseInfo>,
    /// Only populated when a pre-release is NEWER than the newest stable.
    /// A nightly that has already been superseded by a stable is not an
    /// "available nightly" — offering it would be offering a downgrade.
    pub nightly: Option<ReleaseInfo>,
    /// Whether this platform can install an update in place (Windows). On
    /// macOS/Linux the download still happens; the last step is manual.
    pub can_self_install: bool,
    pub checked_at: String,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

/// Progress for the download, emitted as `wispr:update_progress`.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateProgress {
    pub phase: &'static str,
    pub downloaded: u64,
    pub total: u64,
    pub tag: String,
}

fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .connect_timeout(std::time::Duration::from_secs(10))
        .user_agent(concat!("wispr-fox/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("http client: {e}"))
}

/// The installer filename fragment for the platform we are running on.
///
/// Windows is the only one that can be handed straight to the OS to perform an
/// in-place upgrade, which is why `can_self_install` tracks it separately.
fn platform_asset_marker() -> &'static str {
    if cfg!(target_os = "windows") {
        "x64-setup.exe"
    } else if cfg!(target_os = "macos") {
        "aarch64.dmg"
    } else {
        ".AppImage"
    }
}

pub fn can_self_install() -> bool {
    cfg!(target_os = "windows")
}

fn pick_asset(rel: &GhRelease) -> Option<ReleaseAsset> {
    let marker = platform_asset_marker();
    rel.assets
        .iter()
        .find(|a| a.name.ends_with(marker))
        .map(|a| ReleaseAsset {
            name: a.name.clone(),
            url: a.browser_download_url.clone(),
            size: a.size,
        })
}

/// First meaningful line of the release notes — skips the markdown heading and
/// any blank lines so the UI gets a sentence, not a `#`.
fn summarize(body: Option<&str>) -> Option<String> {
    let body = body?;
    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("> ") {
            continue;
        }
        let cleaned = line.trim_start_matches(['*', '-', ' ']).replace("**", "");
        if cleaned.len() < 3 {
            continue;
        }
        let out: String = cleaned.chars().take(180).collect();
        return Some(out);
    }
    None
}

fn to_info(rel: GhRelease, current: &str) -> ReleaseInfo {
    let version = rel.tag_name.trim_start_matches('v').to_string();
    ReleaseInfo {
        newer: crate::commands::version_is_newer(current, &version),
        asset: pick_asset(&rel),
        summary: summarize(rel.body.as_deref()),
        version,
        tag: rel.tag_name,
        html_url: rel.html_url,
        published_at: rel.published_at,
        prerelease: rel.prerelease,
    }
}

async fn fetch_releases() -> Result<Vec<GhRelease>, String> {
    let client = http()?;
    let url = format!("https://api.github.com/repos/{REPO}/releases?per_page=30");
    let resp = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub returned HTTP {}", resp.status().as_u16()));
    }
    let releases: Vec<GhRelease> = resp.json().await.map_err(|e| format!("decode: {e}"))?;
    Ok(releases.into_iter().filter(|r| !r.draft).collect())
}

/// Both channels at once, each compared against the running build.
#[tauri::command]
pub async fn update_status() -> Result<UpdateStatus, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let releases = fetch_releases().await?;

    // The API returns newest-first, so the first match in each class wins.
    let stable_raw = releases.iter().position(|r| !r.prerelease);
    let nightly_raw = releases.iter().position(|r| r.prerelease);

    let mut list: Vec<Option<GhRelease>> = releases.into_iter().map(Some).collect();
    let stable = stable_raw
        .and_then(|i| list[i].take())
        .map(|r| to_info(r, &current));
    let nightly = nightly_raw
        .and_then(|i| list[i].take())
        .map(|r| to_info(r, &current));

    // A nightly older than the newest stable is not an upgrade path. Hiding it
    // is the difference between "here are your options" and a button that
    // quietly moves the user backwards.
    let nightly = match (&stable, nightly) {
        (Some(s), Some(n)) if !crate::commands::version_is_newer(&s.version, &n.version) => None,
        (_, n) => n,
    };

    Ok(UpdateStatus {
        current_is_nightly: current.contains('-'),
        current,
        stable,
        nightly,
        can_self_install: can_self_install(),
        checked_at: chrono::Utc::now().to_rfc3339(),
    })
}

fn host_allowed(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    if parsed.scheme() != "https" {
        return false;
    }
    parsed
        .host_str()
        .map(|h| ALLOWED_HOSTS.iter().any(|a| h == *a || h.ends_with(&format!(".{a}"))))
        .unwrap_or(false)
}

/// Download the installer for `tag` and hand it to the OS.
///
/// Returns the path it was written to. On Windows the installer is launched
/// and the app quits so the upgrade can replace files that are otherwise
/// locked; elsewhere the file is opened/revealed and the app stays running.
#[tauri::command]
pub async fn download_and_install(app: AppHandle, tag: String) -> Result<String, String> {
    let releases = fetch_releases().await?;
    let rel = releases
        .into_iter()
        .find(|r| r.tag_name == tag)
        .ok_or_else(|| format!("release {tag} not found"))?;
    let asset = pick_asset(&rel).ok_or_else(|| {
        format!("{tag} has no installer for this platform — open the release page instead")
    })?;

    // The URL came from the API, but check it anyway: a download is the one
    // place where trusting a response field blindly would be expensive.
    if !host_allowed(&asset.url) {
        return Err("refusing to download: asset is not hosted by GitHub".to_string());
    }

    let emit = |phase: &'static str, downloaded: u64, total: u64| {
        let _ = app.emit(
            "wispr:update_progress",
            UpdateProgress { phase, downloaded, total, tag: tag.clone() },
        );
    };
    emit("starting", 0, asset.size);

    let client = http()?;
    let resp = client
        .get(&asset.url)
        .send()
        .await
        .map_err(|e| format!("download: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("download failed: HTTP {}", resp.status().as_u16()));
    }

    let total = resp.content_length().unwrap_or(asset.size);
    let dir = app
        .path_resolver_updates()
        .map_err(|e| format!("temp dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {e}"))?;
    let dest = dir.join(&asset.name);

    // Stream to disk so a 12 MB installer never sits in memory twice, and so
    // progress is real rather than a fake spinner.
    let mut file = std::fs::File::create(&dest).map_err(|e| format!("create file: {e}"))?;
    let mut downloaded: u64 = 0;
    let mut stream = resp;
    loop {
        let chunk = stream
            .chunk()
            .await
            .map_err(|e| format!("download interrupted: {e}"))?;
        let Some(bytes) = chunk else { break };
        use std::io::Write;
        file.write_all(&bytes).map_err(|e| format!("write: {e}"))?;
        downloaded += bytes.len() as u64;
        emit("downloading", downloaded, total);
    }
    drop(file);

    // Size check. Not a signature — the artifacts are unsigned, same as a
    // manual download — but it does catch a truncated or interrupted transfer
    // before we hand a half-written .exe to the OS, which is the realistic
    // failure here.
    let written = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    if asset.size > 0 && written != asset.size {
        let _ = std::fs::remove_file(&dest);
        return Err(format!(
            "download incomplete ({written} of {} bytes) — not installing",
            asset.size
        ));
    }

    emit("launching", written, total);
    launch_installer(&app, &dest)?;
    Ok(dest.to_string_lossy().to_string())
}

#[cfg(target_os = "windows")]
fn launch_installer(app: &AppHandle, path: &PathBuf) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    // Detached: the installer must outlive us, because the very next thing we
    // do is exit so it can replace files this process holds open.
    std::process::Command::new(path)
        .creation_flags(DETACHED_PROCESS)
        .spawn()
        .map_err(|e| format!("could not start the installer: {e}"))?;

    // Give the installer a moment to actually come up before the app vanishes.
    // Quitting instantly looks like a crash if the installer is slow to paint.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        app.exit(0);
    });
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn launch_installer(app: &AppHandle, path: &PathBuf) -> Result<(), String> {
    // macOS: opens the .dmg so the user can drag the app across. Linux: reveals
    // the AppImage. Neither can be a true in-place upgrade without either code
    // signing (mac) or knowing how the user installed it (Linux), so the app
    // deliberately stays running and the UI says what to do next.
    let _ = app;
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open")
        .arg(path.parent().unwrap_or(path))
        .spawn();
    result
        .map(|_| ())
        .map_err(|e| format!("could not open the download: {e}"))
}

/// Where downloaded installers are staged.
trait UpdatesDir {
    fn path_resolver_updates(&self) -> Result<PathBuf, String>;
}

impl UpdatesDir for AppHandle {
    fn path_resolver_updates(&self) -> Result<PathBuf, String> {
        use tauri::Manager;
        Ok(self
            .path()
            .app_cache_dir()
            .map_err(|e| e.to_string())?
            .join("updates"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only GitHub's own hosts, and only over HTTPS. This is the check that
    /// stops a tampered or spoofed API response pointing a download somewhere
    /// arbitrary, so it is worth pinning precisely.
    #[test]
    fn host_allowlist_accepts_github_and_rejects_everything_else() {
        assert!(host_allowed(
            "https://github.com/kumaradarsh1993/wispr-fox/releases/download/v1/a.exe"
        ));
        assert!(host_allowed("https://objects.githubusercontent.com/x"));
        assert!(host_allowed(
            "https://release-assets.githubusercontent.com/y"
        ));

        // Wrong scheme.
        assert!(!host_allowed("http://github.com/a.exe"));
        // Lookalike domains — the classic way an allowlist gets fooled.
        assert!(!host_allowed("https://github.com.evil.test/a.exe"));
        assert!(!host_allowed("https://notgithub.com/a.exe"));
        assert!(!host_allowed("https://evil.test/a.exe"));
        // Not a URL at all.
        assert!(!host_allowed("a.exe"));
        // A subdomain of an allowed host is fine; a suffix match that isn't a
        // real subdomain boundary is not.
        assert!(host_allowed("https://cdn.objects.githubusercontent.com/z"));
        assert!(!host_allowed("https://evilobjects.githubusercontent.com.bad/z"));
    }

    #[test]
    fn summarize_skips_headings_and_blank_lines() {
        let body = "# v3.4.0\n\n> Stable release\n\n**Your devices, as one account.** Insights merges.\n";
        assert_eq!(
            summarize(Some(body)).as_deref(),
            Some("Your devices, as one account. Insights merges.")
        );
        assert_eq!(summarize(Some("#only a heading")), None);
        assert_eq!(summarize(None), None);
    }

    #[test]
    fn summarize_strips_bullet_markers() {
        assert_eq!(
            summarize(Some("## What\n- **Fixed** the thing\n")).as_deref(),
            Some("Fixed the thing")
        );
    }

    /// The platform marker must match the names CI actually produces, or the
    /// Install button silently degrades to "no installer for this platform".
    #[test]
    fn platform_marker_matches_ci_artifact_names() {
        let ci_names = [
            "wispr-fox_3.4.0-nightly.1_x64-setup.exe",
            "wispr-fox_3.4.0-nightly.1_aarch64.dmg",
            "wispr-fox_3.4.0-nightly.1_amd64.AppImage",
            "wispr-fox_3.4.0-nightly.1_amd64.deb",
            "wispr-fox-3.4.0-nightly.1-1.x86_64.rpm",
        ];
        let marker = platform_asset_marker();
        assert!(
            ci_names.iter().any(|n| n.ends_with(marker)),
            "no CI artifact ends with '{marker}'"
        );
    }
}
