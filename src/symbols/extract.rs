use tower_lsp::lsp_types::{DocumentSymbol, Range, SymbolKind, Url};
use tree_sitter::{Node, Tree};

use crate::parsing::doc_comments::extract_doc_comment;
use crate::parsing::scala::{
    self, CLASS_DEF, ENUM_DEF, EXTENSION_DEF, FUNCTION_DEF, GIVEN_DEF, OBJECT_DEF, TEMPLATE_BODY,
    TRAIT_DEF, TYPE_DEF, VAL_DEF, VAR_DEF,
};
use crate::symbols::lang::SourceLanguage;
use crate::symbols::types::SymbolInfo;

/// Map a Scala definition node kind to an LSP SymbolKind.
pub fn scala_kind_for_node(kind: &str) -> Option<SymbolKind> {
    match kind {
        CLASS_DEF     => Some(SymbolKind::CLASS),
        TRAIT_DEF     => Some(SymbolKind::INTERFACE),
        OBJECT_DEF    => Some(SymbolKind::MODULE),
        FUNCTION_DEF  => Some(SymbolKind::FUNCTION),
        VAL_DEF       => Some(SymbolKind::FIELD),
        VAR_DEF       => Some(SymbolKind::FIELD),
        TYPE_DEF      => Some(SymbolKind::TYPE_PARAMETER),
        GIVEN_DEF     => Some(SymbolKind::CONSTANT),
        ENUM_DEF      => Some(SymbolKind::ENUM),
        EXTENSION_DEF => Some(SymbolKind::MODULE),
        _             => None,
    }
}

/// Extract a hierarchical list of DocumentSymbols from the whole tree (Scala only).
pub fn document_symbols(tree: &Tree, source: &str) -> Vec<DocumentSymbol> {
    extract_children(tree.root_node(), source)
}

fn extract_children(node: Node<'_>, source: &str) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if let Some(sym) = try_extract_symbol(child, source) {
            symbols.push(sym);
        } else {
            // Descend into non-definition nodes (e.g., package clauses)
            let nested = extract_children(child, source);
            symbols.extend(nested);
        }
    }
    symbols
}

fn try_extract_symbol(node: Node<'_>, source: &str) -> Option<DocumentSymbol> {
    let sym_kind = scala_kind_for_node(node.kind())?;
    let name = scala::node_name(node, source)?;
    let range = scala::node_to_range(node);

    // selection_range: just the name node
    let selection_range = node
        .child_by_field_name("name")
        .map(scala::node_to_range)
        .unwrap_or(range);

    // Recursively extract children from the body/template
    let children = extract_body_symbols(node, source);

    #[allow(deprecated)]
    Some(DocumentSymbol {
        name: name.to_string(),
        detail: None,
        kind: sym_kind,
        deprecated: None,
        range,
        selection_range,
        children: if children.is_empty() { None } else { Some(children) },
        tags: None,
    })
}

fn extract_body_symbols(node: Node<'_>, source: &str) -> Vec<DocumentSymbol> {
    let mut children = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == TEMPLATE_BODY || child.kind() == "block" {
            children.extend(extract_children(child, source));
        }
    }
    children
}

/// Extract a flat list of SymbolInfos for the workspace index (Scala).
pub fn workspace_symbols(tree: &Tree, source: &str, uri: &Url) -> Vec<SymbolInfo> {
    workspace_symbols_for_lang(tree, source, uri, SourceLanguage::Scala)
}

/// Extract a flat list of SymbolInfos for the workspace index for any supported language.
pub fn workspace_symbols_for_lang(
    tree: &Tree,
    source: &str,
    uri: &Url,
    lang: SourceLanguage,
) -> Vec<SymbolInfo> {
    let mut infos = Vec::new();
    collect_symbols_lang(tree.root_node(), source, uri, lang, None, &mut infos);
    infos
}

fn collect_symbols_lang(
    node: Node<'_>,
    source: &str,
    uri: &Url,
    lang: SourceLanguage,
    container: Option<&str>,
    out: &mut Vec<SymbolInfo>,
) {
    if let Some(sym_kind) = lang.symbol_kind(node.kind()) {
        if let Some(name) = lang.node_name(node, source) {
            let range = scala::node_to_range(node);
            let selection_range = node
                .child_by_field_name("name")
                .map(scala::node_to_range)
                .unwrap_or(range);

            let mut info = SymbolInfo::new(name.to_string(), sym_kind, uri.clone(), range, selection_range);
            if let Some(c) = container {
                info = info.with_container(c);
            }
            if let Some(doc) = extract_doc_comment(node, source, lang.doc_comment_kinds()) {
                info = info.with_doc(doc);
            }
            let name_owned = name.to_string();
            out.push(info);

            // Recurse with this symbol as the container
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_symbols_lang(child, source, uri, lang, Some(&name_owned), out);
            }
            return;
        }
    }

    // Non-definition node: just recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_symbols_lang(child, source, uri, lang, container, out);
    }
}

/// Collect all identifier nodes matching `name` in the tree (for references).
/// Handles Scala/Java (`identifier`, `type_identifier`) and Kotlin (`simple_identifier`).
pub fn find_identifiers<'a>(tree: &'a Tree, source: &str, name: &str) -> Vec<Range> {
    let mut ranges = Vec::new();
    collect_identifiers(tree.root_node(), source, name, &mut ranges);
    ranges
}

fn collect_identifiers(node: Node<'_>, source: &str, name: &str, out: &mut Vec<Range>) {
    if (node.kind() == "identifier"
        || node.kind() == "type_identifier"
        || node.kind() == "simple_identifier")
        && node.utf8_text(source.as_bytes()).ok() == Some(name)
    {
        out.push(scala::node_to_range(node));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_identifiers(child, source, name, out);
    }
}
