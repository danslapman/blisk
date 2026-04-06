use tree_sitter::Node;

// Definition node kinds (tree-sitter-kotlin)
pub const CLASS_DECL:     &str = "class_declaration";
pub const INTERFACE_DECL: &str = "interface_declaration";
pub const FUN_DECL:       &str = "function_declaration";
pub const PROP_DECL:      &str = "property_declaration";
pub const OBJECT_DECL:    &str = "object_declaration";

pub const DEFINITION_KINDS: &[&str] = &[
    CLASS_DECL, INTERFACE_DECL, FUN_DECL, PROP_DECL, OBJECT_DECL,
];

// Comment node kinds
pub const MULTILINE_COMMENT: &str = "multiline_comment"; // includes /** KDoc */
pub const LINE_COMMENT:      &str = "line_comment";

/// Comment node kinds that can contain KDoc (`/** ... */`).
pub const DOC_COMMENT_KINDS: &[&str] = &[MULTILINE_COMMENT];

/// Extract the name text from a Kotlin definition node.
///
/// Kotlin's tree-sitter grammar does not use a `"name"` field on most definition
/// nodes; instead the name appears as the first `simple_identifier` child.
/// This function tries the `"name"` field first for forward-compatibility, then
/// falls back to the first `simple_identifier` child.
pub fn node_name<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    if let Some(n) = node.child_by_field_name("name") {
        return n.utf8_text(source.as_bytes()).ok();
    }
    // tree-sitter-kotlin does not assign a "name" field on most definitions.
    // Classes and objects use `type_identifier` as the name node;
    // functions and properties use `simple_identifier`.
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "simple_identifier" || child.kind() == "type_identifier" {
            return child.utf8_text(source.as_bytes()).ok();
        }
    }
    None
}
