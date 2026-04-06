use blisk::backend;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fetch_dep_sources = args.iter().any(|a| a == "--fetch-dep-sources");

    // Standalone mode: --fetch-dep-sources <path>
    // Runs the dependency fetch directly and exits, without starting the LSP server.
    if fetch_dep_sources {
        if let Some(path) = args.iter().skip_while(|a| *a != "--fetch-dep-sources").nth(1) {
            let root = std::path::PathBuf::from(path);
            let index = std::sync::Arc::new(blisk::symbols::index::WorkspaceIndex::new());
            blisk::deps::fetch_dep_sources(&root, index).await;
            return;
        }
    }

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) =
        LspService::new(move |client| backend::Backend::new(client, fetch_dep_sources));
    Server::new(stdin, stdout, socket).serve(service).await;
}
