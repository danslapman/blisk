use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Range};
use tree_sitter::{Node, Tree};

use crate::parsing::scala::node_to_range;

pub fn get_diagnostics(tree: &Tree, source: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    collect_errors(tree.root_node(), source, &mut diags);
    diags
}

fn collect_errors(node: Node<'_>, source: &str, out: &mut Vec<Diagnostic>) {
    if node.is_error() {
        let range = node_to_range(node);
        let message = if node.child_count() == 0 {
            // Leaf error — show the unexpected token text
            let text = node.utf8_text(source.as_bytes()).unwrap_or("?");
            format!("Unexpected token: `{text}`")
        } else {
            "Syntax error".to_string()
        };
        out.push(Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::ERROR),
            message,
            source: Some("blisk".to_string()),
            ..Default::default()
        });
        // Don't recurse into error nodes to avoid flooding with sub-errors
        return;
    }

    if node.is_missing() {
        let range = Range {
            start: crate::parsing::scala::point_to_position(node.start_position()),
            end: crate::parsing::scala::point_to_position(node.start_position()),
        };
        let text = node.kind();
        out.push(Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::ERROR),
            message: format!("Missing `{text}`"),
            source: Some("blisk".to_string()),
            ..Default::default()
        });
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_errors(child, source, out);
    }
}
