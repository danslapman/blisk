use tower_lsp::lsp_types::{FoldingRange, FoldingRangeKind};
use tree_sitter::{Node, Tree};

use crate::parsing::scala::{BLOCK, COMMENT, IMPORT_DECL, MULTILINE_COMMENT, TEMPLATE_BODY};

pub fn folding_ranges(tree: &Tree, _source: &str) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    collect_folds(tree.root_node(), &mut ranges);
    ranges
}

fn collect_folds(node: Node<'_>, out: &mut Vec<FoldingRange>) {
    let start_line = node.start_position().row as u32;
    let end_line = node.end_position().row as u32;

    if end_line <= start_line {
        // Single-line: still recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_folds(child, out);
        }
        return;
    }

    match node.kind() {
        k if k == TEMPLATE_BODY || k == BLOCK => {
            out.push(FoldingRange {
                start_line,
                start_character: None,
                end_line,
                end_character: None,
                kind: Some(FoldingRangeKind::Region),
                collapsed_text: None,
            });
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_folds(child, out);
            }
        }
        k if k == COMMENT || k == MULTILINE_COMMENT || k == "block_comment" => {
            out.push(FoldingRange {
                start_line,
                start_character: None,
                end_line,
                end_character: None,
                kind: Some(FoldingRangeKind::Comment),
                collapsed_text: None,
            });
            // Don't recurse into comments
        }
        k if k == IMPORT_DECL => {
            // Emit import group fold only for the first import in a consecutive run
            if !is_import_group_start(node) {
                // This import is a continuation; the group was handled by the first one
                return;
            }
            let group_end = import_group_end(node);
            if group_end > start_line {
                out.push(FoldingRange {
                    start_line,
                    start_character: None,
                    end_line: group_end,
                    end_character: None,
                    kind: Some(FoldingRangeKind::Imports),
                    collapsed_text: None,
                });
            }
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_folds(child, out);
            }
        }
    }
}

fn is_import_group_start(node: Node<'_>) -> bool {
    node.kind() == IMPORT_DECL
        && node
            .prev_named_sibling()
            .map(|p| p.kind() != IMPORT_DECL)
            .unwrap_or(true)
}

fn import_group_end(first: Node<'_>) -> u32 {
    let mut last_line = first.end_position().row as u32;
    let mut cur = first.next_named_sibling();
    while let Some(n) = cur {
        if n.kind() == IMPORT_DECL {
            last_line = n.end_position().row as u32;
            cur = n.next_named_sibling();
        } else {
            break;
        }
    }
    last_line
}
