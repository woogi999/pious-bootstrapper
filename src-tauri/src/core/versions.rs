//! Roblox client version discovery, installation and removal.

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use futures::SinkExt;
use futures::StreamExt;
use futures::channel::mpsc::Sender;

use crate::core::model::{VersionRecord, VersionSource};
use crate::core::roblox;

const CDN: &str = "https://setup.rbxcdn.com";

/// Directories that may contain Roblox client builds, with their source.
fn search_roots(pious_dir: &Path) -> Vec<(PathBuf, VersionSource)> {
    let mut roots = vec![(pious_dir.to_path_buf(), VersionSource::Pious)];
    if let Some(local) = dirs::data_local_dir() {
        roots.push((local.join("Roblox").join("Versions"), VersionSource::Roblox));
        for strap in ["Bloxstrap", "Fishstrap", "Voidstrap"] {
            roots.push((local.join(strap).join("Versions"), VersionSource::Bootstrapper));
        }
    }
    if let Some(pf86) = std::env::var_os("ProgramFiles(x86)") {
        roots.push((
            PathBuf::from(pf86).join("Roblox").join("Versions"),
            VersionSource::Roblox,
        ));
    }
    roots
}

/// Scans the known install locations for Roblox player builds.
pub async fn detect(pious_dir: PathBuf) -> Vec<VersionRecord> {
    tokio::task::spawn_blocking(move || detect_blocking(&pious_dir))
        .await
        .unwrap_or_default()
}

fn detect_blocking(pious_dir: &Path) -> Vec<VersionRecord> {
    let mut found: Vec<VersionRecord> = Vec::new();

    for (root, source) in search_roots(pious_dir) {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !path.is_dir() || !name.starts_with("version-") {
                continue;
            }
            // Studio builds live alongside player builds; only keep players.
            if path.join("RobloxStudioBeta.exe").exists() {
                continue;
            }
            if found.iter().any(|v| v.hash == name) {
                continue;
            }

            let installed_at: DateTime<Utc> = entry
                .metadata()
                .and_then(|m| m.modified().or_else(|_| m.created()))
                .map(DateTime::<Utc>::from)
                .unwrap_or_else(|_| Utc::now());

            found.push(VersionRecord {
                hash: name.to_owned(),
                version: None,
                valid: path.join("RobloxPlayerBeta.exe").is_file(),
                path,
                source,
                installed_at,
                label: None,
            });
        }
    }

    found
}

#[derive(Debug, Clone)]
pub enum InstallEvent {
    Progress {
        downloaded: u64,
        total: u64,
        stage: String,
    },
    Finished(Result<VersionRecord, String>),
}

struct Package {
    name: String,
    checksum: String,
    size: u64,
}

fn parse_manifest(manifest: &str) -> Result<Vec<Package>, String> {
    let mut lines = manifest.lines().map(str::trim).filter(|l| !l.is_empty());
    if lines.next() != Some("v0") {
        return Err("Unrecognized package manifest".into());
    }
    let lines: Vec<&str> = lines.collect();
    lines
        .chunks(4)
        .filter(|c| c.len() == 4)
        .map(|c| {
            Ok(Package {
                name: c[0].to_owned(),
                checksum: c[1].to_ascii_lowercase(),
                size: c[2].parse().map_err(|_| "Malformed package manifest")?,
            })
        })
        .collect()
}

/// Where each player package is extracted, relative to the version folder.
fn package_destination(name: &str) -> Option<&'static str> {
    Some(match name {
        "RobloxApp.zip" | "redist.zip" | "WebView2.zip" => "",
        "shaders.zip" => "shaders/",
        "ssl.zip" => "ssl/",
        "WebView2RuntimeInstaller.zip" => "WebView2RuntimeInstaller/",
        "content-avatar.zip" => "content/avatar/",
        "content-configs.zip" => "content/configs/",
        "content-fonts.zip" => "content/fonts/",
        "content-sky.zip" => "content/sky/",
        "content-sounds.zip" => "content/sounds/",
        "content-textures2.zip" => "content/textures/",
        "content-models.zip" => "content/models/",
        "content-platform-fonts.zip" => "PlatformContent/pc/fonts/",
        "content-platform-dictionaries.zip" => "PlatformContent/pc/shared_compression_dictionaries/",
        "content-terrain.zip" => "PlatformContent/pc/terrain/",
        "content-textures3.zip" => "PlatformContent/pc/textures/",
        "extracontent-luapackages.zip" => "ExtraContent/LuaPackages/",
        "extracontent-translations.zip" => "ExtraContent/translations/",
        "extracontent-models.zip" => "ExtraContent/models/",
        "extracontent-textures.zip" => "ExtraContent/textures/",
        "extracontent-places.zip" => "ExtraContent/places/",
        _ => return None,
    })
}

const APP_SETTINGS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Settings>\r\n\t<ContentFolder>content</ContentFolder>\r\n\t<BaseUrl>http://www.roblox.com</BaseUrl>\r\n</Settings>\r\n";

/// Downloads and installs a Roblox player build into `versions_dir`,
/// reporting progress through `events`.
pub async fn install(
    hash: String,
    version: Option<String>,
    versions_dir: PathBuf,
    mut events: Sender<InstallEvent>,
) {
    let result = install_inner(&hash, version, &versions_dir, &mut events).await;
    if result.is_err() {
        let _ = tokio::fs::remove_dir_all(versions_dir.join(format!(".{hash}.partial"))).await;
    }
    let _ = events.send(InstallEvent::Finished(result)).await;
}

async fn install_inner(
    hash: &str,
    version: Option<String>,
    versions_dir: &Path,
    events: &mut Sender<InstallEvent>,
) -> Result<VersionRecord, String> {
    let http = roblox::http();

    let _ = events
        .send(InstallEvent::Progress {
            downloaded: 0,
            total: 0,
            stage: "Reading package list".into(),
        })
        .await;

    let response = http
        .get(format!("{CDN}/{hash}-rbxPkgManifest.txt"))
        .send()
        .await
        .map_err(roblox::network_error)?;
    if !response.status().is_success() {
        return Err(format!(
            "Roblox doesn't offer {hash} for download. It may be too old or mistyped."
        ));
    }
    let manifest = response.text().await.map_err(roblox::network_error)?;
    let packages: Vec<Package> = parse_manifest(&manifest)?
        .into_iter()
        .filter(|p| package_destination(&p.name).is_some())
        .collect();

    if packages.is_empty() {
        return Err("This build has no player packages.".into());
    }

    let total: u64 = packages.iter().map(|p| p.size).sum();
    let staging = versions_dir.join(format!(".{hash}.partial"));
    let _ = tokio::fs::remove_dir_all(&staging).await;
    tokio::fs::create_dir_all(&staging)
        .await
        .map_err(|e| format!("Couldn't create the install folder: {e}"))?;

    let mut downloaded = 0u64;
    for package in &packages {
        let stage = format!("Downloading {}", package.name.trim_end_matches(".zip"));
        let response = http
            .get(format!("{CDN}/{hash}-{}", package.name))
            .send()
            .await
            .map_err(roblox::network_error)?;
        if !response.status().is_success() {
            return Err(format!("Couldn't download {} ({})", package.name, response.status()));
        }

        let mut bytes = Vec::with_capacity(package.size as usize);
        let mut stream = response.bytes_stream();
        let mut last_report = 0u64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(roblox::network_error)?;
            downloaded += chunk.len() as u64;
            bytes.extend_from_slice(&chunk);
            if downloaded - last_report > 256 * 1024 {
                last_report = downloaded;
                let _ = events
                    .send(InstallEvent::Progress {
                        downloaded,
                        total,
                        stage: stage.clone(),
                    })
                    .await;
            }
        }

        if format!("{:x}", md5::compute(&bytes)) != package.checksum {
            return Err(format!("{} failed its integrity check. Try again.", package.name));
        }

        let destination = staging.join(package_destination(&package.name).unwrap_or_default());
        tokio::task::spawn_blocking(move || extract(&bytes, &destination))
            .await
            .map_err(|e| e.to_string())??;
    }

    let _ = events
        .send(InstallEvent::Progress {
            downloaded: total,
            total,
            stage: "Finishing up".into(),
        })
        .await;

    tokio::fs::write(staging.join("AppSettings.xml"), APP_SETTINGS)
        .await
        .map_err(|e| e.to_string())?;

    let final_dir = versions_dir.join(hash);
    let _ = tokio::fs::remove_dir_all(&final_dir).await;
    tokio::fs::rename(&staging, &final_dir)
        .await
        .map_err(|e| format!("Couldn't finalize the install: {e}"))?;

    Ok(VersionRecord {
        hash: hash.to_owned(),
        version,
        valid: final_dir.join("RobloxPlayerBeta.exe").is_file(),
        path: final_dir,
        source: VersionSource::Pious,
        installed_at: Utc::now(),
        label: None,
    })
}

fn extract(bytes: &[u8], destination: &Path) -> Result<(), String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("Corrupt package: {e}"))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().replace('\\', "/");
        let relative: PathBuf = name
            .split('/')
            .filter(|part| !part.is_empty() && *part != "." && *part != "..")
            .collect();
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = destination.join(relative);

        if file.is_dir() || name.ends_with('/') {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut contents = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut contents).map_err(|e| e.to_string())?;
        std::fs::write(&target, contents).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Deletes a version's folder from disk.
pub async fn remove(path: PathBuf) -> Result<(), String> {
    tokio::fs::remove_dir_all(&path)
        .await
        .map_err(|e| format!("Couldn't remove {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest() {
        let manifest = "v0\nRobloxApp.zip\nABCDEF\n100\n200\nshaders.zip\n012345\n10\n20\n";
        let packages = parse_manifest(manifest).unwrap();
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name, "RobloxApp.zip");
        assert_eq!(packages[0].checksum, "abcdef");
        assert_eq!(packages[1].size, 10);
    }
}
