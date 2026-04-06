use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position, SymbolKind, Url};
use tree_sitter::{Node, Tree};

use crate::parsing::doc_comments::extract_doc_comment;
use crate::parsing::scala::{
    self, node_to_range, position_to_point, DEFINITION_KINDS, PARAMETER, VAL_DEF, VAR_DEF,
};
use crate::symbols::{extract::scala_kind_for_node, index::WorkspaceIndex};

pub fn hover(
    tree: &Tree,
    source: &str,
    _uri: &Url,
    pos: Position,
    index: &WorkspaceIndex,
) -> Option<Hover> {
    let point = position_to_point(pos);
    let root = tree.root_node();
    let node = root.named_descendant_for_point_range(point, point)?;

    // Must be on an identifier
    let name = node.utf8_text(source.as_bytes()).ok()?;
    if name.is_empty() || !node.kind().contains("identifier") {
        return None;
    }

    // 1. Same-file scope walk (Scala only — hover is only triggered on open Scala documents)
    if let Some(h) = resolve_hover_in_file(node, source, name, index) {
        return Some(h);
    }

    // 2. Cross-file index lookup (works for Java/Kotlin symbols too)
    let matches = index.lookup_by_name(name);
    if matches.is_empty() {
        return None;
    }

    let info = &matches[0];
    let markdown = format_hover(name, info.kind, info.doc_comment.as_deref(), index);
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: markdown,
        }),
        range: None,
    })
}

fn resolve_hover_in_file(
    start: Node<'_>,
    source: &str,
    name: &str,
    index: &WorkspaceIndex,
) -> Option<Hover> {
    let mut current = start.parent()?;
    loop {
        if let Some(h) = find_hover_in_scope(current, source, name, index) {
            return Some(h);
        }
        current = current.parent()?;
    }
}

fn find_hover_in_scope(
    scope: Node<'_>,
    source: &str,
    name: &str,
    index: &WorkspaceIndex,
) -> Option<Hover> {
    let mut cursor = scope.walk();
    for child in scope.children(&mut cursor) {
        let child_kind = child.kind();
        if !DEFINITION_KINDS.contains(&child_kind) && child_kind != PARAMETER {
            continue;
        }

        let def_name = match child_kind {
            PARAMETER => child
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok()),
            VAL_DEF | VAR_DEF => child
                .child_by_field_name("pattern")
                .or_else(|| child.child_by_field_name("name"))
                .and_then(|n| n.utf8_text(source.as_bytes()).ok()),
            _ => child
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok()),
        };

        if def_name == Some(name) {
            let sym_kind = scala_kind_for_node(child_kind).unwrap_or(SymbolKind::VARIABLE);
            let doc = extract_doc_comment(child, source, scala::DOC_COMMENT_KINDS);
            let markdown = format_hover(name, sym_kind, doc.as_deref(), index);
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: markdown,
                }),
                range: Some(node_to_range(child)),
            });
        }
    }
    None
}

/// Build the Markdown hover string.
///
/// Format:
/// ```scala
/// class Greeter
/// ```
///
/// ---
///
/// <doc comment, with [[links]] and {{{code}}} rendered>
fn format_hover(name: &str, kind: SymbolKind, doc: Option<&str>, index: &WorkspaceIndex) -> String {
    let kind_label = symbol_kind_label(kind);
    let mut out = format!("```scala\n{kind_label} {name}\n```");
    if let Some(d) = doc {
        out.push_str("\n\n---\n\n");
        out.push_str(&linkify_scaladoc(d, index));
    }
    out
}

/// Transform Scaladoc/Javadoc/KDoc markup into Markdown:
/// - `{{{ ... }}}` → fenced scala code block
/// - `[[URL desc]]` → `[desc](URL)`
/// - `[[URL]]` → `[URL](URL)`
/// - `[[SymbolName]]` → `[SymbolName](file_uri)` if found in index, else plain `SymbolName`
fn linkify_scaladoc(text: &str, index: &WorkspaceIndex) -> String {
    // First pass: replace {{{ ... }}} with fenced code blocks
    let text = replace_code_blocks(text);
    // Second pass: replace [[ ... ]] wiki-links
    replace_wiki_links(&text, index)
}

fn replace_code_blocks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{{") {
        out.push_str(&rest[..start]);
        rest = &rest[start + 3..];
        if let Some(end) = rest.find("}}}") {
            let code = rest[..end].trim();
            out.push_str("```scala\n");
            out.push_str(code);
            out.push_str("\n```");
            rest = &rest[end + 3..];
        } else {
            // Unclosed — emit literally
            out.push_str("{{{");
        }
    }
    out.push_str(rest);
    out
}

fn replace_wiki_links(text: &str, index: &WorkspaceIndex) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        rest = &rest[start + 2..];
        if let Some(end) = rest.find("]]") {
            let inner = &rest[..end];
            out.push_str(&render_wiki_link(inner, index));
            rest = &rest[end + 2..];
        } else {
            // Unclosed — emit literally
            out.push_str("[[");
        }
    }
    out.push_str(rest);
    out
}

fn render_wiki_link(inner: &str, index: &WorkspaceIndex) -> String {
    // External URL: [[https://... description]] or [[https://...]]
    if inner.starts_with("http://") || inner.starts_with("https://") {
        if let Some(sp) = inner.find(' ') {
            let url = &inner[..sp];
            let desc = inner[sp + 1..].trim();
            return format!("[{desc}]({url})");
        } else {
            return format!("[{inner}]({inner})");
        }
    }

    // Symbol reference — may contain disambiguation suffixes like $ or !
    // and overload signatures like (x:Int)*. Skip complex forms.
    if inner.contains('(') {
        // Overload signature — too complex, render as plain text (strip brackets)
        let simple = inner.split('(').next().unwrap_or(inner);
        return last_segment(simple).to_string();
    }

    // Strip $ / ! disambiguation suffixes before the dot
    let clean = inner.trim_end_matches('$').trim_end_matches('!');
    let simple_name = last_segment(clean);

    let matches = index.lookup_by_name(simple_name);
    if matches.len() == 1 {
        let uri = matches[0].uri.as_str();
        format!("[{simple_name}]({uri})")
    } else {
        simple_name.to_string()
    }
}

/// Return the last dot-separated segment of a qualified name.
/// `scala.Option` → `Option`, `List` → `List`
fn last_segment(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

fn symbol_kind_label(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::CLASS          => "class",
        SymbolKind::INTERFACE      => "trait",
        SymbolKind::MODULE         => "object",
        SymbolKind::FUNCTION       => "def",
        SymbolKind::FIELD          => "val",
        SymbolKind::CONSTANT       => "given",
        SymbolKind::ENUM           => "enum",
        SymbolKind::TYPE_PARAMETER => "type",
        SymbolKind::METHOD         => "method",
        SymbolKind::CONSTRUCTOR    => "constructor",
        _                          => "val",
    }
}
