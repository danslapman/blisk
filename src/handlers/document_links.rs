use tower_lsp::lsp_types::{DocumentLink, Position, Range, Url};
use tree_sitter::{Node, Tree};

use crate::parsing::scala::{BLOCK_COMMENT, COMMENT, INTERPOLATED_STRING, MULTILINE_COMMENT, STRING_NODE};

/// Regex-free URL detection: look for "http://" or "https://" spans inside
/// comment and string nodes, then extract the URL by scanning for whitespace.
pub fn document_links(tree: &Tree, source: &str) -> Vec<DocumentLink> {
    let mut links = Vec::new();
    collect_links(tree.root_node(), source, &mut links);
    links
}

fn collect_links(node: Node<'_>, source: &str, out: &mut Vec<DocumentLink>) {
    let is_text_node = matches!(
        node.kind(),
        _ if node.kind() == COMMENT
            || node.kind() == BLOCK_COMMENT
            || node.kind() == MULTILINE_COMMENT
            || node.kind() == STRING_NODE
            || node.kind() == INTERPOLATED_STRING
    );

    if is_text_node {
        if let Ok(text) = node.utf8_text(source.as_bytes()) {
            let base_byte = node.start_byte();
            for link in extract_urls(text, node.start_position().row, node.start_position().column, base_byte, source) {
                out.push(link);
            }
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_links(child, source, out);
    }
}

fn extract_urls(
    text: &str,
    start_row: usize,
    start_col: usize,
    _base_byte: usize,
    _source: &str,
) -> Vec<DocumentLink> {
    let mut links = Vec::new();
    let prefixes = ["https://", "http://"];

    let mut search_from = 0usize;
    while search_from < text.len() {
        let Some((prefix_idx, found_at)) = prefixes
            .iter()
            .enumerate()
            .filter_map(|(i, p)| text[search_from..].find(p).map(|pos| (i, search_from + pos)))
            .min_by_key(|&(_, pos)| pos)
        else {
            break;
        };

        let _ = prefix_idx;
        let url_start = found_at;

        // Scan until whitespace or end
        let url_end = text[url_start..]
            .char_indices()
            .find(|(_, c)| c.is_whitespace() || *c == '"' || *c == '\'' || *c == '>' || *c == ')')
            .map(|(i, _)| url_start + i)
            .unwrap_or(text.len());

        let url_str = &text[url_start..url_end];
        if url_str.len() > 8 {
            if let Ok(target) = Url::parse(url_str) {
                // Convert byte offsets within text to positions
                let range_start = byte_offset_to_position(text, url_start, start_row, start_col);
                let range_end = byte_offset_to_position(text, url_end, start_row, start_col);
                links.push(DocumentLink {
                    range: Range { start: range_start, end: range_end },
                    target: Some(target),
                    tooltip: None,
                    data: None,
                });
            }
        }

        search_from = url_end + 1;
    }
    links
}

fn byte_offset_to_position(text: &str, byte_offset: usize, base_row: usize, base_col: usize) -> Position {
    let before = &text[..byte_offset.min(text.len())];
    let newlines = before.bytes().filter(|&b| b == b'\n').count();
    if newlines == 0 {
        Position {
            line: base_row as u32,
            character: (base_col + byte_offset) as u32,
        }
    } else {
        let last_nl = before.rfind('\n').unwrap();
        Position {
            line: (base_row + newlines) as u32,
            character: (byte_offset - last_nl - 1) as u32,
        }
    }
}
