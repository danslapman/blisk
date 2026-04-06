use tower_lsp::lsp_types::SymbolKind;
use tree_sitter::{Language, Node};

use crate::parsing::{java, kotlin, scala};

/// Source language supported by blisk for indexing.
#[derive(Clone, Copy, Debug)]
pub enum SourceLanguage {
    Scala,
    Java,
    Kotlin,
}

impl SourceLanguage {
    /// Detect language from a file extension. Returns `None` for unsupported extensions.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "scala" | "sc" => Some(Self::Scala),
            "java"         => Some(Self::Java),
            "kt"           => Some(Self::Kotlin),
            _              => None,
        }
    }

    /// Return the tree-sitter `Language` for this source language.
    pub fn tree_sitter_language(self) -> Language {
        match self {
            Self::Scala  => tree_sitter_scala::LANGUAGE.into(),
            Self::Java   => tree_sitter_java::LANGUAGE.into(),
            Self::Kotlin => tree_sitter_kotlin::LANGUAGE.into(),
        }
    }

    /// Return the definition node kind strings for this language.
    pub fn definition_kinds(self) -> &'static [&'static str] {
        match self {
            Self::Scala  => scala::DEFINITION_KINDS,
            Self::Java   => java::DEFINITION_KINDS,
            Self::Kotlin => kotlin::DEFINITION_KINDS,
        }
    }

    /// Return the comment node kind strings that can contain doc comments.
    pub fn doc_comment_kinds(self) -> &'static [&'static str] {
        match self {
            Self::Scala  => scala::DOC_COMMENT_KINDS,
            Self::Java   => java::DOC_COMMENT_KINDS,
            Self::Kotlin => kotlin::DOC_COMMENT_KINDS,
        }
    }

    /// Map a definition node kind to an LSP `SymbolKind`.
    pub fn symbol_kind(self, node_kind: &str) -> Option<SymbolKind> {
        match self {
            Self::Scala  => scala_kind_for(node_kind),
            Self::Java   => java_kind_for(node_kind),
            Self::Kotlin => kotlin_kind_for(node_kind),
        }
    }

    /// Extract the name text from a definition node in this language.
    pub fn node_name<'a>(self, node: Node<'a>, source: &'a str) -> Option<&'a str> {
        match self {
            Self::Scala | Self::Java => scala::node_name(node, source),
            Self::Kotlin             => kotlin::node_name(node, source),
        }
    }
}

fn scala_kind_for(k: &str) -> Option<SymbolKind> {
    crate::symbols::extract::scala_kind_for_node(k)
}

fn java_kind_for(k: &str) -> Option<SymbolKind> {
    match k {
        java::CLASS_DECL       => Some(SymbolKind::CLASS),
        java::INTERFACE_DECL   => Some(SymbolKind::INTERFACE),
        java::ENUM_DECL        => Some(SymbolKind::ENUM),
        java::METHOD_DECL      => Some(SymbolKind::METHOD),
        java::FIELD_DECL       => Some(SymbolKind::FIELD),
        java::CONSTRUCTOR_DECL => Some(SymbolKind::CONSTRUCTOR),
        java::ANNOTATION_DECL  => Some(SymbolKind::INTERFACE),
        _                      => None,
    }
}

fn kotlin_kind_for(k: &str) -> Option<SymbolKind> {
    match k {
        kotlin::CLASS_DECL     => Some(SymbolKind::CLASS),
        kotlin::INTERFACE_DECL => Some(SymbolKind::INTERFACE),
        kotlin::FUN_DECL       => Some(SymbolKind::FUNCTION),
        kotlin::PROP_DECL      => Some(SymbolKind::FIELD),
        kotlin::OBJECT_DECL    => Some(SymbolKind::MODULE),
        _                      => None,
    }
}
