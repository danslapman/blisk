use tree_sitter::Node;

/// Walk backwards through `prev_named_sibling()` from `node`, collecting
/// contiguous comment nodes that begin with `/**`.
/// Only nodes whose kind is in `doc_comment_kinds` are considered.
/// Returns the cleaned doc comment text, or `None` if none is found.
pub fn extract_doc_comment(
    node: Node<'_>,
    source: &str,
    doc_comment_kinds: &[&str],
) -> Option<String> {
    let mut comments: Vec<String> = Vec::new();
    let mut sib = node.prev_named_sibling();
    while let Some(s) = sib {
        if doc_comment_kinds.contains(&s.kind()) {
            let raw = &source[s.byte_range()];
            if raw.trim_start().starts_with("/**") {
                comments.push(strip_scaladoc(raw));
                sib = s.prev_named_sibling();
            } else {
                break;
            }
        } else {
            break;
        }
    }
    if comments.is_empty() {
        return None;
    }
    comments.reverse();
    let joined = comments.join("\n").trim().to_string();
    if joined.is_empty() { None } else { Some(joined) }
}

/// Remove `/**`, `*/`, and leading `* ` or `*` from each line.
/// Tags such as `@param`, `@return` are preserved verbatim.
/// Works for Scaladoc, Javadoc, and KDoc, which all use the same `/** ... */` format.
pub fn strip_scaladoc(raw: &str) -> String {
    raw.lines()
        .map(|line| {
            let t = line.trim();
            // Skip pure delimiter lines
            if t == "/**" || t == "*/" {
                return "";
            }
            // Strip opening /** (possibly with text on same line)
            let t = t.strip_prefix("/**").unwrap_or(t);
            // Strip closing */
            let t = t.strip_suffix("*/").unwrap_or(t);
            // Strip leading " * " or " *"
            let t = t
                .strip_prefix("* ")
                .unwrap_or_else(|| t.strip_prefix('*').unwrap_or(t));
            t.trim_end()
        })
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
