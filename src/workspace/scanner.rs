use std::path::PathBuf;
use std::sync::Arc;

use tower_lsp::lsp_types::Url;
use tree_sitter::Parser;

use crate::symbols::{extract, index::WorkspaceIndex};

/// Scan all Scala files under `root` and populate the workspace index.
/// Trees are NOT retained after scanning — only SymbolInfo records are stored.
pub async fn scan_workspace(root: Url, index: Arc<WorkspaceIndex>) {
    let path = match root.to_file_path() {
        Ok(p) => p,
        Err(_) => return,
    };

    let files = tokio::task::spawn_blocking(move || collect_scala_files(&path))
        .await
        .unwrap_or_default();

    let semaphore = Arc::new(tokio::sync::Semaphore::new(64));
    let mut handles = Vec::new();

    for file_path in files {
        let index = index.clone();
        let permit = semaphore.clone().acquire_owned().await.unwrap();

        let handle = tokio::spawn(async move {
            let _permit = permit;
            index_file(&file_path, &index).await;
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await;
    }
}

pub(crate) async fn index_file(path: &std::path::Path, index: &WorkspaceIndex) {
    let text = match tokio::fs::read_to_string(path).await {
        Ok(t) => t,
        Err(_) => return,
    };
    let uri = match Url::from_file_path(path) {
        Ok(u) => u,
        Err(_) => return,
    };

    let symbols = tokio::task::spawn_blocking({
        let text = text.clone();
        let uri = uri.clone();
        move || {
            let mut parser = match create_parser() {
                Some(p) => p,
                None => return vec![],
            };
            let tree = match parser.parse(text.as_bytes(), None) {
                Some(t) => t,
                None => return vec![],
            };
            extract::workspace_symbols(&tree, &text, &uri)
        }
    })
    .await
    .unwrap_or_default();

    index.update_file(&uri, symbols);
}

fn collect_scala_files(root: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_recursive(root, &mut files);
    files
}

fn collect_recursive(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if (name.starts_with('.') && name != ".dep-srcs") || name == "target" || name == "node_modules" {
                continue;
            }
            collect_recursive(&path, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext == "scala" || ext == "sc" {
                out.push(path);
            }
        }
    }
}

pub fn create_parser() -> Option<Parser> {
    let mut parser = Parser::new();
    let language: tree_sitter::Language = tree_sitter_scala::LANGUAGE.into();
    parser.set_language(&language).ok()?;
    Some(parser)
}
