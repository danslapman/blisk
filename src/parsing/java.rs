// Definition node kinds (tree-sitter-java)
pub const CLASS_DECL:       &str = "class_declaration";
pub const INTERFACE_DECL:   &str = "interface_declaration";
pub const ENUM_DECL:        &str = "enum_declaration";
pub const METHOD_DECL:      &str = "method_declaration";
pub const FIELD_DECL:       &str = "field_declaration";
pub const CONSTRUCTOR_DECL: &str = "constructor_declaration";
pub const ANNOTATION_DECL:  &str = "annotation_type_declaration";

pub const DEFINITION_KINDS: &[&str] = &[
    CLASS_DECL, INTERFACE_DECL, ENUM_DECL,
    METHOD_DECL, FIELD_DECL, CONSTRUCTOR_DECL, ANNOTATION_DECL,
];

// Comment node kinds
pub const BLOCK_COMMENT: &str = "block_comment"; // includes /** Javadoc */
pub const LINE_COMMENT:  &str = "line_comment";

/// Comment node kinds that can contain Javadoc (`/** ... */`).
pub const DOC_COMMENT_KINDS: &[&str] = &[BLOCK_COMMENT];
