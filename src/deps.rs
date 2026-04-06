use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use log::{debug, error, info, warn};
use tokio::process::Command;

use crate::symbols::index::WorkspaceIndex;
use crate::workspace::scanner::index_file;

/// Fetch, extract and index dependency sources into `<root>/target/.dep-srcs/`.
/// Unpacking is incremental: already-resolved coordinates recorded in
/// `target/.dep-srcs/.resolved.list` are skipped.
pub async fn fetch_dep_sources(root: &Path, index: Arc<WorkspaceIndex>) {
    info!("fetching dep sources for {}", root.display());

    // Run sbt dependencyList
    let output = match Command::new("sbt")
        .arg("dependencyList")
        .current_dir(root)
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            error!("failed to run sbt: {e}");
            return;
        }
    };
    if !output.status.success() {
        error!(
            "sbt dependencyList failed (exit {:?}):\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut deps = parse_dependency_list(&stdout);
    deps.sort();
    deps.dedup();

    let subprojects = fetch_subproject_names(root).await;
    info!("subprojects: {subprojects:?}");
    let deps: Vec<String> = deps
        .into_iter()
        .filter(|coord| {
            let artifact = coord.split(':').nth(1).unwrap_or("");
            let base = strip_scala_suffix(artifact);
            !subprojects.contains(base)
        })
        .collect();
    info!("found {} unique external dependencies", deps.len());

    // Create target/.dep-srcs
    let dep_srcs = root.join(".dep-srcs");
    if let Err(e) = tokio::fs::create_dir_all(&dep_srcs).await {
        error!("failed to create {}: {e}", dep_srcs.display());
        return;
    }

    // Load the manifest of previously resolved deps
    let resolved_file = dep_srcs.join(".resolved.list");
    let already_resolved: HashSet<String> = tokio::fs::read_to_string(&resolved_file)
        .await
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();

    let new_deps: Vec<String> = deps
        .into_iter()
        .filter(|d| !already_resolved.contains(d))
        .collect();

    info!("{} new deps to fetch", new_deps.len());

    // Fetch and extract only new deps
    for dep in &new_deps {
        info!("fetching sources for {dep}");
        let jars = fetch_source_jars(dep).await;
        debug!("  -> {} source jars", jars.len());
        for jar in jars {
            extract_jar_to(&jar, &dep_srcs).await;
        }
    }

    // Persist updated manifest
    if !new_deps.is_empty() {
        let mut manifest: Vec<String> = already_resolved.into_iter().collect();
        manifest.extend(new_deps);
        manifest.sort();
        let _ = tokio::fs::write(&resolved_file, manifest.join("\n") + "\n").await;
    }

    // Index all .scala files under dep_srcs (fast for already-indexed files)
    scan_dep_sources(&dep_srcs, index).await;
}

/// Run `sbt projects` and return the set of local subproject names (e.g. "oolong-core").
async fn fetch_subproject_names(root: &Path) -> HashSet<String> {
    let output = match Command::new("sbt")
        .arg("projects")
        .current_dir(root)
        .output()
        .await
    {
        Ok(o) => o,
        Err(_) => return HashSet::new(),
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("[info]")?;
            let name = rest.trim().trim_start_matches("* ").trim();
            // Only lines that are just a plain identifier (no spaces, non-empty)
            if !name.is_empty() && !name.contains(' ') && !name.starts_with("In ") {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Strip Scala binary version suffix from an artifact ID.
/// e.g. "cats-core_3" -> "cats-core", "mongo-scala-bson_2.13" -> "mongo-scala-bson"
fn strip_scala_suffix(artifact: &str) -> &str {
    if let Some(idx) = artifact.rfind('_') {
        let suffix = &artifact[idx + 1..];
        // Suffix is a Scala version if it starts with a digit
        if suffix.starts_with(|c: char| c.is_ascii_digit()) {
            return &artifact[..idx];
        }
    }
    artifact
}

/// Parse `[info] groupId:artifactId:version` lines from sbt dependencyList output.
/// Lines that don't match the exact 3-part colon-separated form are ignored,
/// which silently filters out local subprojects and sbt log noise.
fn parse_dependency_list(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("[info]")?;
            let coord = rest.trim();
            let parts: Vec<&str> = coord.split(':').collect();
            if parts.len() == 3 && parts.iter().all(|p| !p.is_empty()) {
                Some(coord.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Run `cs fetch --sources <coord>` and return paths to source jars.
/// Returns an empty vec on any failure (e.g. subproject not in Maven).
async fn fetch_source_jars(coord: &str) -> Vec<PathBuf> {
    let output = match Command::new("cs")
        .args(["fetch", "--sources", coord])
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            warn!("cs fetch failed: {e}");
            return vec![];
        }
    };
    if !output.status.success() {
        warn!(
            "cs fetch non-zero exit for {coord}: {}",
            String::from_utf8_lossy(&output.stderr).lines().next().unwrap_or("")
        );
        return vec![];
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| PathBuf::from(l.trim()))
        .filter(|p| p.extension().map_or(false, |e| e == "jar"))
        .collect()
}

/// Extract `.scala` files from a source jar into `dest`.
async fn extract_jar_to(jar: &Path, dest: &Path) {
    let jar = jar.to_path_buf();
    let dest = dest.to_path_buf();
    let _ = tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&jar).ok()?;
        let mut archive = zip::ZipArchive::new(file).ok()?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).ok()?;
            let name = entry.name().to_string();
            if !name.ends_with(".scala") {
                continue;
            }
            let out_path = dest.join(&name);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).ok()?;
            }
            let mut out = std::fs::File::create(&out_path).ok()?;
            std::io::copy(&mut entry, &mut out).ok()?;
        }
        Some(())
    })
    .await;
}

/// Index all `.scala` files found under `dep_srcs`.
async fn scan_dep_sources(dep_srcs: &Path, index: Arc<WorkspaceIndex>) {
    let files = {
        let dep_srcs = dep_srcs.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let mut out = vec![];
            collect_scala_recursive(&dep_srcs, &mut out);
            out
        })
        .await
        .unwrap_or_default()
    };

    let sem = Arc::new(tokio::sync::Semaphore::new(64));
    let mut handles = vec![];
    for f in files {
        let index = index.clone();
        let permit = sem.clone().acquire_owned().await.unwrap();
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            index_file(&f, &index).await;
        }));
    }
    for h in handles {
        let _ = h.await;
    }
}

fn collect_scala_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_scala_recursive(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("scala") {
            out.push(path);
        }
    }
}
