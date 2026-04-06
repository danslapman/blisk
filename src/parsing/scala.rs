use tower_lsp::lsp_types::{Position, Range};
use tree_sitter::{Node, Point};

// Definition node kinds
pub const CLASS_DEF: &str = "class_definition";
pub const TRAIT_DEF: &str = "trait_definition";
pub const OBJECT_DEF: &str = "object_definition";
pub const FUNCTION_DEF: &str = "function_definition";
pub const VAL_DEF: &str = "val_definition";
pub const VAR_DEF: &str = "var_definition";
pub const TYPE_DEF: &str = "type_definition";
pub const GIVEN_DEF: &str = "given_definition";
pub const ENUM_DEF: &str = "enum_definition";
pub const EXTENSION_DEF: &str = "extension_definition";

// All definition kinds
pub const DEFINITION_KINDS: &[&str] = &[
    CLASS_DEF, TRAIT_DEF, OBJECT_DEF, FUNCTION_DEF,
    VAL_DEF, VAR_DEF, TYPE_DEF, GIVEN_DEF, ENUM_DEF, EXTENSION_DEF,
];

// Identifier node kinds
pub const IDENTIFIER: &str = "identifier";
pub const TYPE_IDENTIFIER: &str = "type_identifier";
pub const OPERATOR_IDENTIFIER: &str = "operator_identifier";

// Structural node kinds
pub const TEMPLATE_BODY: &str = "template_body";
pub const BLOCK: &str = "block";
pub const IMPORT_DECL: &str = "import_declaration";
pub const PACKAGE_CLAUSE: &str = "package_clause";
pub const PACKAGE_OBJECT: &str = "package_object";
pub const COMPILATION_UNIT: &str = "compilation_unit";

// Literal node kinds
pub const INTEGER_LITERAL: &str = "integer_literal";
pub const FLOATING_LITERAL: &str = "floating_point_literal";
pub const STRING_NODE: &str = "string";
pub const INTERPOLATED_STRING: &str = "interpolated_string_expression";
pub const BOOLEAN_LITERAL: &str = "boolean_literal";
pub const CHARACTER_LITERAL: &str = "character_literal";

// Comment node kinds
pub const COMMENT: &str = "comment";
pub const BLOCK_COMMENT: &str = "block_comment";
pub const MULTILINE_COMMENT: &str = "multiline_comment";

/// Comment node kinds that can contain Scaladoc (`/** ... */`).
pub const DOC_COMMENT_KINDS: &[&str] = &[COMMENT, BLOCK_COMMENT, MULTILINE_COMMENT];

// Parameter node kinds
pub const PARAMETERS: &str = "parameters";
pub const PARAMETER: &str = "parameter";
pub const CLASS_PARAMETERS: &str = "class_parameters";

// Annotation
pub const ANNOTATION: &str = "annotation";

/// Convert a tree-sitter Node's span to an LSP Range.
/// Note: tree-sitter columns are in bytes; for ASCII (common in Scala) this matches UTF-16.
pub fn node_to_range(node: Node<'_>) -> Range {
    Range {
        start: point_to_position(node.start_position()),
        end: point_to_position(node.end_position()),
    }
}

pub fn point_to_position(p: Point) -> Position {
    Position {
        line: p.row as u32,
        character: p.column as u32,
    }
}

pub fn position_to_point(p: Position) -> Point {
    Point {
        row: p.line as usize,
        column: p.character as usize,
    }
}

/// Convert an LSP Position to a byte offset within `text`.
/// Handles UTF-16 character encoding.
pub fn pos_to_byte(text: &str, pos: Position) -> usize {
    let mut byte_offset = 0usize;
    for (line_idx, line) in text.split('\n').enumerate() {
        if line_idx == pos.line as usize {
            return byte_offset + utf16_to_byte_offset(line, pos.character as usize);
        }
        byte_offset += line.len() + 1; // +1 for '\n'
    }
    text.len()
}

/// Convert a byte offset within `text` to a tree-sitter Point.
pub fn byte_to_point(text: &str, byte_offset: usize) -> Point {
    let capped = byte_offset.min(text.len());
    let before = &text[..capped];
    let row = before.bytes().filter(|&b| b == b'\n').count();
    let col = before.rfind('\n').map(|i| capped - i - 1).unwrap_or(capped);
    Point { row, column: col }
}

fn utf16_to_byte_offset(line: &str, utf16_offset: usize) -> usize {
    let mut count = 0usize;
    for (byte_idx, ch) in line.char_indices() {
        if count >= utf16_offset {
            return byte_idx;
        }
        count += ch.len_utf16();
    }
    line.len()
}

/// Try to extract the text of the "name" child field from a definition node.
pub fn node_name<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    let name_node = node.child_by_field_name("name")?;
    name_node.utf8_text(source.as_bytes()).ok()
}
