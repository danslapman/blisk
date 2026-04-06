use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens,
    SemanticTokensLegend,
};
use tree_sitter::{Node, Tree};

use crate::parsing::scala::*;

// Token type indices — must match `token_legend()` order exactly
pub const TT_NAMESPACE: u32 = 0;
pub const TT_TYPE: u32 = 1;
pub const TT_CLASS: u32 = 2;
pub const TT_INTERFACE: u32 = 3;
pub const TT_FUNCTION: u32 = 4;
pub const TT_VARIABLE: u32 = 5;
pub const TT_PARAMETER: u32 = 6;
pub const TT_STRING: u32 = 7;
pub const TT_NUMBER: u32 = 8;
pub const TT_COMMENT: u32 = 9;
pub const TT_KEYWORD: u32 = 10;
pub const TT_DECORATOR: u32 = 11;

pub const MOD_DEFINITION: u32 = 1 << 0;
pub const MOD_READONLY: u32 = 1 << 1;

pub fn token_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::NAMESPACE,
            SemanticTokenType::TYPE,
            SemanticTokenType::CLASS,
            SemanticTokenType::INTERFACE,
            SemanticTokenType::FUNCTION,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::PARAMETER,
            SemanticTokenType::STRING,
            SemanticTokenType::NUMBER,
            SemanticTokenType::COMMENT,
            SemanticTokenType::KEYWORD,
            SemanticTokenType::DECORATOR,
        ],
        token_modifiers: vec![
            SemanticTokenModifier::DEFINITION,
            SemanticTokenModifier::READONLY,
        ],
    }
}

struct RawToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
    modifiers: u32,
}

pub fn semantic_tokens_full(tree: &Tree, source: &str) -> SemanticTokens {
    let mut raw: Vec<RawToken> = Vec::new();
    collect_tokens(tree.root_node(), source, &mut raw);
    raw.sort_by(|a, b| a.line.cmp(&b.line).then(a.start.cmp(&b.start)));

    let mut tokens = Vec::with_capacity(raw.len());
    let mut prev_line = 0u32;
    let mut prev_start = 0u32;
    for t in &raw {
        let delta_line = t.line - prev_line;
        let delta_start = if delta_line == 0 { t.start - prev_start } else { t.start };
        tokens.push(SemanticToken {
            delta_line,
            delta_start,
            length: t.length,
            token_type: t.token_type,
            token_modifiers_bitset: t.modifiers,
        });
        prev_line = t.line;
        prev_start = t.start;
    }

    SemanticTokens { result_id: None, data: tokens }
}

fn collect_tokens(node: Node<'_>, source: &str, out: &mut Vec<RawToken>) {
    let kind = node.kind();

    // Leaf token types: emit and stop recursing
    match kind {
        k if k == COMMENT || k == BLOCK_COMMENT || k == MULTILINE_COMMENT => {
            emit(node, TT_COMMENT, 0, out);
            return;
        }
        k if k == STRING_NODE || k == INTERPOLATED_STRING || k == CHARACTER_LITERAL => {
            emit(node, TT_STRING, 0, out);
            return;
        }
        k if k == INTEGER_LITERAL || k == FLOATING_LITERAL => {
            emit(node, TT_NUMBER, 0, out);
            return;
        }
        k if k == ANNOTATION => {
            emit(node, TT_DECORATOR, 0, out);
            return;
        }
        k if k == TYPE_IDENTIFIER => {
            emit(node, TT_TYPE, 0, out);
            return;
        }
        k if !node.is_named() && is_keyword(k) => {
            emit(node, TT_KEYWORD, 0, out);
            return;
        }
        _ => {}
    }

    // Definition nodes: emit name child with definition type, skip name when recursing
    if let Some((tt, mods)) = definition_token(kind) {
        let name_id = node.child_by_field_name("name").map(|n| {
            emit(n, tt, mods, out);
            n.id()
        });
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if Some(child.id()) != name_id {
                collect_tokens(child, source, out);
            }
        }
        return;
    }

    // Default: recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_tokens(child, source, out);
    }
}

fn emit(node: Node<'_>, token_type: u32, modifiers: u32, out: &mut Vec<RawToken>) {
    let start = node.start_position();
    let end = node.end_position();
    // Only emit single-line tokens; for multi-line (comments/strings), emit first line
    let length = if start.row == end.row {
        (end.column - start.column) as u32
    } else {
        // emit to end of start line
        let line_text = ""; // approximate — we don't have source here easily
        let _ = line_text;
        // Use a fallback: just emit a reasonable length
        80
    };
    if length == 0 {
        return;
    }
    out.push(RawToken {
        line: start.row as u32,
        start: start.column as u32,
        length,
        token_type,
        modifiers,
    });
}

fn definition_token(kind: &str) -> Option<(u32, u32)> {
    match kind {
        CLASS_DEF => Some((TT_CLASS, MOD_DEFINITION)),
        TRAIT_DEF => Some((TT_INTERFACE, MOD_DEFINITION)),
        OBJECT_DEF => Some((TT_NAMESPACE, MOD_DEFINITION)),
        FUNCTION_DEF => Some((TT_FUNCTION, MOD_DEFINITION)),
        VAL_DEF => Some((TT_VARIABLE, MOD_DEFINITION | MOD_READONLY)),
        VAR_DEF => Some((TT_VARIABLE, MOD_DEFINITION)),
        TYPE_DEF => Some((TT_TYPE, MOD_DEFINITION)),
        GIVEN_DEF => Some((TT_VARIABLE, MOD_DEFINITION | MOD_READONLY)),
        ENUM_DEF => Some((TT_CLASS, MOD_DEFINITION)),
        EXTENSION_DEF => Some((TT_NAMESPACE, MOD_DEFINITION)),
        PARAMETER => Some((TT_PARAMETER, MOD_DEFINITION)),
        _ => None,
    }
}

fn is_keyword(s: &str) -> bool {
    matches!(
        s,
        "abstract" | "case" | "catch" | "class" | "def" | "do" | "else"
            | "enum" | "export" | "extends" | "final" | "finally" | "for"
            | "given" | "if" | "implicit" | "import" | "lazy" | "match"
            | "new" | "object" | "override" | "package" | "private"
            | "protected" | "return" | "sealed" | "super" | "then" | "this"
            | "throw" | "trait" | "try" | "type" | "using" | "val" | "var"
            | "while" | "with" | "yield" | "true" | "false" | "null"
            | "inline" | "opaque" | "open" | "transparent" | "end"
    )
}
