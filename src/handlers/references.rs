use tower_lsp::lsp_types::{Location, Position, ReferenceContext, Url};
use tree_sitter::Tree;

use crate::parsing::scala::position_to_point;
use crate::symbols::extract::find_identifiers;
use crate::symbols::index::WorkspaceIndex;

pub fn find_references(
    tree: &Tree,
    source: &str,
    uri: &Url,
    pos: Position,
    _context: ReferenceContext,
    index: &WorkspaceIndex,
    get_file: &dyn Fn(&Url) -> Option<(String, Tree)>,
) -> Vec<Location> {
    let point = position_to_point(pos);
    let root = tree.root_node();
    let Some(node) = root.named_descendant_for_point_range(point, point) else {
        return vec![];
    };

    let name = match node.utf8_text(source.as_bytes()).ok() {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => return vec![],
    };

    let mut locations = Vec::new();

    // 1. Same-file references
    for range in find_identifiers(tree, source, &name) {
        locations.push(Location { uri: uri.clone(), range });
    }

    // 2. Cross-file references via workspace index
    for file_uri in index.all_uris() {
        if &file_uri == uri {
            continue; // already done above
        }

        if let Some((file_text, file_tree)) = get_file(&file_uri) {
            // Pre-filter: skip files not containing the name as a substring
            if !file_text.contains(name.as_str()) {
                continue;
            }
            for range in find_identifiers(&file_tree, &file_text, &name) {
                locations.push(Location { uri: file_uri.clone(), range });
            }
        }
    }

    locations
}
