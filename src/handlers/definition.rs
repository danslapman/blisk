use tower_lsp::lsp_types::{GotoDefinitionResponse, Location, Position, Url};
use tree_sitter::{Node, Tree};

use crate::parsing::scala::{node_to_range, position_to_point, DEFINITION_KINDS, PARAMETER, VAL_DEF, VAR_DEF};
use crate::symbols::index::WorkspaceIndex;

pub fn goto_definition(
    tree: &Tree,
    source: &str,
    uri: &Url,
    pos: Position,
    index: &WorkspaceIndex,
) -> Option<GotoDefinitionResponse> {
    let point = position_to_point(pos);
    let root = tree.root_node();
    let node = root.named_descendant_for_point_range(point, point)?;

    // Must be on an identifier
    let name = node.utf8_text(source.as_bytes()).ok()?;
    if name.is_empty() || !node.kind().contains("identifier") {
        return None;
    }

    // 1. Same-file scope walk
    if let Some(loc) = resolve_in_file(node, source, uri, name) {
        return Some(GotoDefinitionResponse::Scalar(loc));
    }

    // 2. Cross-file index lookup
    let matches = index.lookup_by_name(name);
    if matches.is_empty() {
        return None;
    }

    let locations: Vec<Location> = matches
        .into_iter()
        .map(|info| Location { uri: info.uri, range: info.range })
        .collect();

    Some(if locations.len() == 1 {
        GotoDefinitionResponse::Scalar(locations.into_iter().next().unwrap())
    } else {
        GotoDefinitionResponse::Array(locations)
    })
}

/// Walk up the scope tree from `node` looking for a binding of `name`.
fn resolve_in_file(
    start: Node<'_>,
    source: &str,
    uri: &Url,
    name: &str,
) -> Option<Location> {
    let mut current = start.parent()?;
    loop {
        // Search named children of the current scope for a definition with this name
        if let Some(loc) = find_definition_in_scope(current, source, uri, name) {
            return Some(loc);
        }
        current = current.parent()?;
    }
}

fn find_definition_in_scope(scope: Node<'_>, source: &str, uri: &Url, name: &str) -> Option<Location> {
    let mut cursor = scope.walk();
    for child in scope.children(&mut cursor) {
        let child_kind = child.kind();
        if !DEFINITION_KINDS.contains(&child_kind) && child_kind != PARAMETER {
            continue;
        }

        let def_name = match child_kind {
            PARAMETER => {
                // parameters can have a pattern child
                child
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
            }
            VAL_DEF | VAR_DEF => {
                // pattern might be an identifier directly
                child
                    .child_by_field_name("pattern")
                    .or_else(|| child.child_by_field_name("name"))
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
            }
            _ => child
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok()),
        };

        if def_name == Some(name) {
            let name_node = child
                .child_by_field_name("name")
                .or_else(|| child.child_by_field_name("pattern"))
                .unwrap_or(child);
            return Some(Location {
                uri: uri.clone(),
                range: node_to_range(name_node),
            });
        }
    }
    None
}
