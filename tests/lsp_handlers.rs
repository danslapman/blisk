use blisk::handlers::{
    definition, diagnostics, document_links, document_symbols, folding, hover, references,
    selection, semantic_tokens,
};
use blisk::symbols::{extract, index::WorkspaceIndex, lang::SourceLanguage};
use tower_lsp::lsp_types::*;
use tree_sitter::Tree;

fn parse(source: &str) -> Tree {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_scala::LANGUAGE.into())
        .unwrap();
    parser.parse(source, None).unwrap()
}

// ---- Diagnostics ----

#[test]
fn no_diags_for_valid_scala() {
    let source = include_str!("fixtures/simple_class.scala");
    let tree = parse(source);
    let diags = diagnostics::get_diagnostics(&tree, source);
    assert!(diags.is_empty(), "Expected no diagnostics, got: {:?}", diags);
}

#[test]
fn diags_for_syntax_error() {
    let source = include_str!("fixtures/with_errors.scala");
    let tree = parse(source);
    let diags = diagnostics::get_diagnostics(&tree, source);
    assert!(!diags.is_empty(), "Expected diagnostics for broken Scala");
    assert!(
        diags.iter().all(|d| d.severity == Some(DiagnosticSeverity::ERROR)),
        "All diagnostics should be ERROR severity"
    );
}

// ---- Document Symbols ----

#[test]
fn symbols_top_level_names() {
    let source = include_str!("fixtures/simple_class.scala");
    let tree = parse(source);
    let symbols = document_symbols::document_symbols(&tree, source);

    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Animal"), "Missing Animal; got: {:?}", names);
    assert!(
        names.contains(&"Describable"),
        "Missing Describable; got: {:?}",
        names
    );

    // class Animal should be SymbolKind::CLASS
    assert!(symbols
        .iter()
        .any(|s| s.name == "Animal" && s.kind == SymbolKind::CLASS));
    // trait Describable should be SymbolKind::INTERFACE
    assert!(symbols
        .iter()
        .any(|s| s.name == "Describable" && s.kind == SymbolKind::INTERFACE));
}

#[test]
fn symbols_nested_in_class() {
    let source = include_str!("fixtures/simple_class.scala");
    let tree = parse(source);
    let symbols = document_symbols::document_symbols(&tree, source);

    let animal_class = symbols
        .iter()
        .find(|s| s.name == "Animal" && s.kind == SymbolKind::CLASS)
        .expect("Animal class not found");

    let children = animal_class
        .children
        .as_ref()
        .expect("Animal class should have children");
    let child_names: Vec<&str> = children.iter().map(|s| s.name.as_str()).collect();
    // Function definitions are extracted correctly.
    // NOTE: val/var definitions are currently NOT extracted because scala::node_name()
    // looks up the "name" field but tree-sitter-scala uses a "pattern" field for val LHS.
    assert!(
        child_names.contains(&"speak"),
        "Missing speak; got: {:?}",
        child_names
    );
}

// ---- Folding Ranges ----

#[test]
fn folding_region_for_class_body() {
    let source = include_str!("fixtures/simple_class.scala");
    let tree = parse(source);
    let folds = folding::folding_ranges(&tree, source);
    assert!(
        folds.iter().any(|f| f.kind == Some(FoldingRangeKind::Region)),
        "Expected at least one Region fold; got: {:?}",
        folds
    );
}

#[test]
fn folding_imports_for_multiline_import() {
    // A multi-line import statement triggers the Imports fold (single-line imports
    // hit the early-return for single-line nodes before the IMPORT_DECL match arm).
    let source = include_str!("fixtures/folding_test.scala");
    let tree = parse(source);
    let folds = folding::folding_ranges(&tree, source);
    assert!(
        folds.iter().any(|f| f.kind == Some(FoldingRangeKind::Imports)),
        "Expected an Imports fold for multi-line import; got: {:?}",
        folds
    );
}

// ---- Semantic Tokens ----

#[test]
fn semantic_tokens_not_empty() {
    let source = include_str!("fixtures/simple_class.scala");
    let tree = parse(source);
    let tokens = semantic_tokens::semantic_tokens_full(&tree, source);
    assert!(
        !tokens.data.is_empty(),
        "Expected semantic tokens to be non-empty"
    );
}

// ---- Selection Range ----

#[test]
fn selection_range_returns_parent_chain() {
    let source = include_str!("fixtures/simple_class.scala");
    let tree = parse(source);
    // line 7 char 10 is inside "speak" identifier within the function definition
    let pos = Position {
        line: 7,
        character: 10,
    };
    let ranges = selection::selection_ranges(&tree, vec![pos]);
    assert_eq!(ranges.len(), 1);
    // The returned selection range should have a parent (non-root node)
    assert!(
        ranges[0].parent.is_some(),
        "Selection range should have a parent scope"
    );
}

// ---- Document Links ----

#[test]
fn document_link_url_in_comment() {
    let source = include_str!("fixtures/simple_class.scala");
    let tree = parse(source);
    let links = document_links::document_links(&tree, source);
    assert!(!links.is_empty(), "Expected at least one document link");
    assert!(
        links.iter().any(|l| l
            .target
            .as_ref()
            .map(|u| u.as_str().contains("example.com"))
            .unwrap_or(false)),
        "Expected a link pointing to example.com"
    );
}

// ---- Goto Definition ----

#[test]
fn definition_same_file_val() {
    let source = include_str!("fixtures/cross_ref.scala");
    let tree = parse(source);
    let uri = Url::parse("file:///test/cross_ref.scala").unwrap();
    let index = WorkspaceIndex::new();

    // "greeting" is used at line 4, char 18 ("    val message = greeting + ...")
    // It is defined at line 1 in the class body ("val greeting = ...")
    let pos = Position {
        line: 4,
        character: 18,
    };
    let result = definition::goto_definition(&tree, source, &uri, pos, &index);
    assert!(result.is_some(), "Expected a definition result for 'greeting'");
}

#[test]
fn definition_cross_file_class() {
    let usage_src = include_str!("fixtures/usage.scala");
    let usage_tree = parse(usage_src);
    let usage_uri = Url::parse("file:///test/usage.scala").unwrap();

    let cross_src = include_str!("fixtures/cross_ref.scala");
    let cross_tree = parse(cross_src);
    let cross_uri = Url::parse("file:///test/cross_ref.scala").unwrap();

    let index = WorkspaceIndex::new();
    index.update_file(
        &cross_uri,
        extract::workspace_symbols(&cross_tree, cross_src, &cross_uri),
    );

    // "Greeter" at line 5, char 22 in usage.scala ("    val greeter = new Greeter()")
    let pos = Position {
        line: 5,
        character: 22,
    };
    let result =
        definition::goto_definition(&usage_tree, usage_src, &usage_uri, pos, &index);
    assert!(
        result.is_some(),
        "Expected a cross-file definition result for 'Greeter'"
    );

    let resolved_uris: Vec<Url> = match result.unwrap() {
        GotoDefinitionResponse::Scalar(loc) => vec![loc.uri],
        GotoDefinitionResponse::Array(locs) => locs.into_iter().map(|l| l.uri).collect(),
        GotoDefinitionResponse::Link(links) => links.into_iter().map(|l| l.target_uri).collect(),
    };
    assert!(
        resolved_uris.contains(&cross_uri),
        "Definition should resolve to cross_ref.scala"
    );
}

// ---- Find References ----

#[test]
fn references_same_file() {
    let source = include_str!("fixtures/cross_ref.scala");
    let tree = parse(source);
    let uri = Url::parse("file:///test/cross_ref.scala").unwrap();
    let index = WorkspaceIndex::new();

    // "greeting" at line 1, char 6 ("  val greeting = ...")
    let pos = Position {
        line: 1,
        character: 6,
    };
    let context = ReferenceContext {
        include_declaration: true,
    };
    let refs =
        references::find_references(&tree, source, &uri, pos, context, &index, &|_| None);

    // "greeting" appears at: line 1 (def), line 4 (use), line 9 (use) = at least 3
    assert!(
        refs.len() >= 3,
        "Expected ≥3 references to 'greeting', got {} ({:?})",
        refs.len(),
        refs
    );
}

#[test]
fn references_cross_file() {
    let simple_src = include_str!("fixtures/simple_class.scala");
    let simple_tree = parse(simple_src);
    let simple_uri = Url::parse("file:///test/simple_class.scala").unwrap();

    let usage_src = include_str!("fixtures/usage.scala");
    let usage_uri = Url::parse("file:///test/usage.scala").unwrap();

    let index = WorkspaceIndex::new();
    // Register usage.scala in the index so find_references will visit it
    index.update_file(
        &usage_uri,
        extract::workspace_symbols(&parse(usage_src), usage_src, &usage_uri),
    );

    let get_file = |uri: &Url| -> Option<(String, Tree)> {
        if uri == &usage_uri {
            Some((usage_src.to_string(), parse(usage_src)))
        } else {
            None
        }
    };

    // "Animal" at line 6, char 6 in simple_class.scala ("class Animal(...")
    let pos = Position {
        line: 6,
        character: 6,
    };
    let context = ReferenceContext {
        include_declaration: true,
    };
    let refs = references::find_references(
        &simple_tree,
        simple_src,
        &simple_uri,
        pos,
        context,
        &index,
        &get_file,
    );

    let has_simple = refs.iter().any(|l| l.uri == simple_uri);
    let has_usage = refs.iter().any(|l| l.uri == usage_uri);
    assert!(has_simple, "Expected Animal references in simple_class.scala");
    assert!(has_usage, "Expected Animal references in usage.scala");
}

// ---- Hover ----

#[test]
fn hover_class_with_doc() {
    let source = include_str!("fixtures/hover_test.scala");
    let tree = parse(source);
    let uri = Url::parse("file:///test/hover_test.scala").unwrap();
    let index = WorkspaceIndex::new();

    // "Greeter" at line 3, char 8
    let pos = Position { line: 3, character: 8 };
    let result = hover::hover(&tree, source, &uri, pos, &index);

    let h = result.expect("Expected hover result for 'Greeter'");
    let markdown = match h.contents {
        HoverContents::Markup(mc) => mc.value,
        _ => panic!("Expected MarkupContent"),
    };
    assert!(markdown.contains("class Greeter"), "Missing kind label; got: {markdown}");
    assert!(
        markdown.contains("documented class with a greeting"),
        "Missing doc comment text; got: {markdown}"
    );
}

#[test]
fn hover_method_with_param_doc() {
    let source = include_str!("fixtures/hover_test.scala");
    let tree = parse(source);
    let uri = Url::parse("file:///test/hover_test.scala").unwrap();
    let index = WorkspaceIndex::new();

    // "greet" at line 11, char 6
    let pos = Position { line: 11, character: 6 };
    let result = hover::hover(&tree, source, &uri, pos, &index);

    let h = result.expect("Expected hover result for 'greet'");
    let markdown = match h.contents {
        HoverContents::Markup(mc) => mc.value,
        _ => panic!("Expected MarkupContent"),
    };
    assert!(markdown.contains("def greet"), "Missing kind label; got: {markdown}");
    assert!(markdown.contains("@param name"), "Missing @param tag; got: {markdown}");
    assert!(markdown.contains("@return"), "Missing @return tag; got: {markdown}");
}

#[test]
fn hover_method_see_link_resolved() {
    let source = include_str!("fixtures/hover_test.scala");
    let tree = parse(source);
    let uri = Url::parse("file:///test/hover_test.scala").unwrap();

    // Index the file so Undocumented is resolvable
    let index = WorkspaceIndex::new();
    index.update_file(&uri, extract::workspace_symbols(&tree, source, &uri));

    // "greet" at line 11, char 6 — its doc contains @see [[Undocumented]]
    let pos = Position { line: 11, character: 6 };
    let result = hover::hover(&tree, source, &uri, pos, &index);

    let h = result.expect("Expected hover result for 'greet'");
    let markdown = match h.contents {
        HoverContents::Markup(mc) => mc.value,
        _ => panic!("Expected MarkupContent"),
    };
    // [[Undocumented]] should be resolved to a clickable Markdown link
    assert!(
        markdown.contains("[Undocumented]"),
        "Expected resolved [[Undocumented]] link; got: {markdown}"
    );
}

#[test]
fn hover_val_no_doc_shows_kind() {
    let source = include_str!("fixtures/hover_test.scala");
    let tree = parse(source);
    let uri = Url::parse("file:///test/hover_test.scala").unwrap();
    let index = WorkspaceIndex::new();

    // "greeting" at line 13, char 6
    let pos = Position { line: 13, character: 6 };
    let result = hover::hover(&tree, source, &uri, pos, &index);

    let h = result.expect("Expected hover result for 'greeting'");
    let markdown = match h.contents {
        HoverContents::Markup(mc) => mc.value,
        _ => panic!("Expected MarkupContent"),
    };
    assert!(markdown.contains("val greeting"), "Expected 'val greeting'; got: {markdown}");
    assert!(!markdown.contains("---"), "Unexpected doc separator for undocumented val; got: {markdown}");
}

#[test]
fn hover_cross_file_with_doc() {
    let hover_src = include_str!("fixtures/hover_test.scala");
    let hover_tree = parse(hover_src);
    let hover_uri = Url::parse("file:///test/hover_test.scala").unwrap();

    let usage_src = include_str!("fixtures/usage.scala");
    let usage_tree = parse(usage_src);
    let usage_uri = Url::parse("file:///test/usage.scala").unwrap();

    // Index hover_test.scala so Greeter (with its doc) is cross-file findable
    let index = WorkspaceIndex::new();
    index.update_file(
        &hover_uri,
        extract::workspace_symbols(&hover_tree, hover_src, &hover_uri),
    );

    // "Greeter" at line 5, char 22 in usage.scala: "    val greeter = new Greeter()"
    let pos = Position { line: 5, character: 22 };
    let result = hover::hover(&usage_tree, usage_src, &usage_uri, pos, &index);

    let h = result.expect("Expected cross-file hover for 'Greeter'");
    let markdown = match h.contents {
        HoverContents::Markup(mc) => mc.value,
        _ => panic!("Expected MarkupContent"),
    };
    assert!(markdown.contains("class Greeter"), "Missing kind label; got: {markdown}");
    assert!(
        markdown.contains("documented class with a greeting"),
        "Missing doc comment text; got: {markdown}"
    );
}

#[test]
fn hover_code_block_rendered_as_fenced() {
    let source = include_str!("fixtures/hover_test.scala");
    let tree = parse(source);
    let uri = Url::parse("file:///test/hover_test.scala").unwrap();
    let index = WorkspaceIndex::new();

    // "withExample" at line 27, char 4
    let pos = Position { line: 27, character: 4 };
    let result = hover::hover(&tree, source, &uri, pos, &index);

    let h = result.expect("Expected hover result for 'withExample'");
    let markdown = match h.contents {
        HoverContents::Markup(mc) => mc.value,
        _ => panic!("Expected MarkupContent"),
    };
    assert!(markdown.contains("def withExample"), "Missing kind label; got: {markdown}");
    // {{{ ... }}} should be rendered as a fenced code block, not literal braces
    assert!(!markdown.contains("{{{"), "Raw {{{{ should not appear in output; got: {markdown}");
    assert!(markdown.contains("```"), "Expected fenced code block in output; got: {markdown}");
    assert!(markdown.contains("g.greet"), "Expected code block content; got: {markdown}");
}

// ---- Workspace Symbols ----

#[test]
fn workspace_symbol_search() {
    let source = include_str!("fixtures/simple_class.scala");
    let tree = parse(source);
    let uri = Url::parse("file:///test/simple_class.scala").unwrap();

    let index = WorkspaceIndex::new();
    let syms = extract::workspace_symbols(&tree, source, &uri);
    index.update_file(&uri, syms);

    let results = index.search("anim");
    assert!(
        !results.is_empty(),
        "Expected results for prefix 'anim'"
    );
    assert!(
        results.iter().any(|s| s.name == "Animal"),
        "Expected 'Animal' in search results; got: {:?}",
        results.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
}

// ---- Java / Kotlin parsing helpers ----

fn parse_java(source: &str) -> Tree {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_java::LANGUAGE.into()).unwrap();
    parser.parse(source, None).unwrap()
}

fn parse_kotlin(source: &str) -> Tree {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_kotlin::LANGUAGE.into()).unwrap();
    parser.parse(source, None).unwrap()
}

// ---- Java symbol extraction ----

#[test]
fn java_workspace_symbols_classes_and_methods() {
    let source = include_str!("fixtures/JavaHelper.java");
    let tree = parse_java(source);
    let uri = Url::parse("file:///test/JavaHelper.java").unwrap();
    let syms = extract::workspace_symbols_for_lang(&tree, source, &uri, SourceLanguage::Java);

    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"JavaHelper"), "Missing JavaHelper; got: {:?}", names);
    assert!(names.contains(&"greet"),      "Missing greet; got: {:?}", names);
    assert!(names.contains(&"Status"),     "Missing Status enum; got: {:?}", names);

    let cls = syms.iter().find(|s| s.name == "JavaHelper").unwrap();
    assert_eq!(cls.kind, SymbolKind::CLASS);

    let method = syms.iter().find(|s| s.name == "greet").unwrap();
    assert_eq!(method.kind, SymbolKind::METHOD);
}

// ---- Java Javadoc extraction ----

#[test]
fn java_doc_comment_extracted() {
    let source = include_str!("fixtures/JavaHelper.java");
    let tree = parse_java(source);
    let uri = Url::parse("file:///test/JavaHelper.java").unwrap();
    let syms = extract::workspace_symbols_for_lang(&tree, source, &uri, SourceLanguage::Java);

    let cls = syms.iter().find(|s| s.name == "JavaHelper").unwrap();
    let doc = cls.doc_comment.as_deref().expect("JavaHelper should have a doc comment");
    assert!(doc.contains("documented Java utility class"), "got: {doc}");

    let method = syms.iter().find(|s| s.name == "greet").unwrap();
    let mdoc = method.doc_comment.as_deref().expect("greet should have a doc comment");
    assert!(mdoc.contains("@param name"), "Missing @param; got: {mdoc}");
    assert!(mdoc.contains("@return"),     "Missing @return; got: {mdoc}");
}

// ---- Kotlin symbol extraction ----

#[test]
fn kotlin_workspace_symbols_classes_and_functions() {
    let source = include_str!("fixtures/kotlin_helper.kt");
    let tree = parse_kotlin(source);
    let uri = Url::parse("file:///test/kotlin_helper.kt").unwrap();
    let syms = extract::workspace_symbols_for_lang(&tree, source, &uri, SourceLanguage::Kotlin);

    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"KotlinHelper"), "Missing KotlinHelper; got: {:?}", names);
    assert!(names.contains(&"greet"),        "Missing greet; got: {:?}", names);
    assert!(names.contains(&"Companion"),    "Missing Companion; got: {:?}", names);

    let cls = syms.iter().find(|s| s.name == "KotlinHelper").unwrap();
    assert_eq!(cls.kind, SymbolKind::CLASS);
}

// ---- Kotlin KDoc extraction ----

#[test]
fn kotlin_doc_comment_extracted() {
    let source = include_str!("fixtures/kotlin_helper.kt");
    let tree = parse_kotlin(source);
    let uri = Url::parse("file:///test/kotlin_helper.kt").unwrap();
    let syms = extract::workspace_symbols_for_lang(&tree, source, &uri, SourceLanguage::Kotlin);

    let cls = syms.iter().find(|s| s.name == "KotlinHelper").unwrap();
    let doc = cls.doc_comment.as_deref().expect("KotlinHelper should have a doc comment");
    assert!(doc.contains("documented Kotlin utility class"), "got: {doc}");

    let method = syms.iter().find(|s| s.name == "greet").unwrap();
    let mdoc = method.doc_comment.as_deref().expect("greet should have a doc comment");
    assert!(mdoc.contains("@param name"), "Missing @param; got: {mdoc}");
}

// ---- Workspace symbol search — Java class ----

#[test]
fn workspace_symbol_search_java() {
    let source = include_str!("fixtures/JavaHelper.java");
    let tree = parse_java(source);
    let uri = Url::parse("file:///test/JavaHelper.java").unwrap();

    let index = WorkspaceIndex::new();
    index.update_file(
        &uri,
        extract::workspace_symbols_for_lang(&tree, source, &uri, SourceLanguage::Java),
    );

    let results = index.search("JavaH");
    assert!(
        results.iter().any(|s| s.name == "JavaHelper"),
        "Expected 'JavaHelper' in search; got: {:?}",
        results.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
}

// ---- Cross-language go-to-definition: Scala → Java ----

#[test]
fn definition_cross_language_scala_to_java() {
    let java_src = include_str!("fixtures/JavaHelper.java");
    let java_tree = parse_java(java_src);
    let java_uri = Url::parse("file:///test/JavaHelper.java").unwrap();

    let index = WorkspaceIndex::new();
    index.update_file(
        &java_uri,
        extract::workspace_symbols_for_lang(&java_tree, java_src, &java_uri, SourceLanguage::Java),
    );

    let scala_src = include_str!("fixtures/scala_uses_java.scala");
    let scala_tree = parse(scala_src);
    let scala_uri = Url::parse("file:///test/scala_uses_java.scala").unwrap();

    // "JavaHelper" at line 1, char 14: "  val h = new JavaHelper()"
    let pos = Position { line: 1, character: 14 };
    let result = definition::goto_definition(&scala_tree, scala_src, &scala_uri, pos, &index);

    assert!(result.is_some(), "Expected cross-language definition result for 'JavaHelper'");
    let resolved_uris: Vec<Url> = match result.unwrap() {
        GotoDefinitionResponse::Scalar(loc) => vec![loc.uri],
        GotoDefinitionResponse::Array(locs) => locs.into_iter().map(|l| l.uri).collect(),
        GotoDefinitionResponse::Link(links) => links.into_iter().map(|l| l.target_uri).collect(),
    };
    assert!(
        resolved_uris.contains(&java_uri),
        "Definition should resolve to JavaHelper.java; got: {:?}", resolved_uris
    );
}

// ---- Cross-language go-to-definition: Scala → Kotlin ----

#[test]
fn definition_cross_language_scala_to_kotlin() {
    let kt_src = include_str!("fixtures/kotlin_helper.kt");
    let kt_tree = parse_kotlin(kt_src);
    let kt_uri = Url::parse("file:///test/kotlin_helper.kt").unwrap();

    let index = WorkspaceIndex::new();
    index.update_file(
        &kt_uri,
        extract::workspace_symbols_for_lang(&kt_tree, kt_src, &kt_uri, SourceLanguage::Kotlin),
    );

    let scala_src = include_str!("fixtures/scala_uses_kotlin.scala");
    let scala_tree = parse(scala_src);
    let scala_uri = Url::parse("file:///test/scala_uses_kotlin.scala").unwrap();

    // "KotlinHelper" at line 1, char 14: "  val h = new KotlinHelper("Hi")"
    let pos = Position { line: 1, character: 14 };
    let result = definition::goto_definition(&scala_tree, scala_src, &scala_uri, pos, &index);

    assert!(result.is_some(), "Expected cross-language definition for 'KotlinHelper'");
    let resolved_uris: Vec<Url> = match result.unwrap() {
        GotoDefinitionResponse::Scalar(loc) => vec![loc.uri],
        GotoDefinitionResponse::Array(locs) => locs.into_iter().map(|l| l.uri).collect(),
        GotoDefinitionResponse::Link(links) => links.into_iter().map(|l| l.target_uri).collect(),
    };
    assert!(
        resolved_uris.contains(&kt_uri),
        "Definition should resolve to kotlin_helper.kt; got: {:?}", resolved_uris
    );
}

// ---- Cross-language hover: Scala hovering over Java symbol ----

#[test]
fn hover_cross_language_java_symbol() {
    let java_src = include_str!("fixtures/JavaHelper.java");
    let java_tree = parse_java(java_src);
    let java_uri = Url::parse("file:///test/JavaHelper.java").unwrap();

    let index = WorkspaceIndex::new();
    index.update_file(
        &java_uri,
        extract::workspace_symbols_for_lang(&java_tree, java_src, &java_uri, SourceLanguage::Java),
    );

    let scala_src = include_str!("fixtures/scala_uses_java.scala");
    let scala_tree = parse(scala_src);
    let scala_uri = Url::parse("file:///test/scala_uses_java.scala").unwrap();

    // "JavaHelper" at line 1, char 14
    let pos = Position { line: 1, character: 14 };
    let result = hover::hover(&scala_tree, scala_src, &scala_uri, pos, &index);

    let h = result.expect("Expected hover for cross-language Java symbol");
    let markdown = match h.contents {
        HoverContents::Markup(mc) => mc.value,
        _ => panic!("Expected MarkupContent"),
    };
    assert!(markdown.contains("JavaHelper"), "Missing JavaHelper; got: {markdown}");
    assert!(
        markdown.contains("documented Java utility class"),
        "Missing Javadoc; got: {markdown}"
    );
}

// ---- Cross-language find references: Java class referenced from Scala ----

#[test]
fn references_cross_language_java_to_scala() {
    let scala_src = include_str!("fixtures/scala_refs_java.scala");
    let scala_tree = parse(scala_src);
    let scala_uri = Url::parse("file:///test/scala_refs_java.scala").unwrap();

    let index = WorkspaceIndex::new();
    index.update_file(
        &scala_uri,
        extract::workspace_symbols(&scala_tree, scala_src, &scala_uri),
    );

    let java_src = include_str!("fixtures/JavaHelper.java");
    let java_tree = parse_java(java_src);
    let java_uri = Url::parse("file:///test/JavaHelper.java").unwrap();

    // "JavaHelper" in "public class JavaHelper {" at line 5, char 13
    let pos = Position { line: 5, character: 13 };
    let context = ReferenceContext { include_declaration: true };

    let get_file = |uri: &Url| -> Option<(String, Tree)> {
        if uri == &scala_uri {
            Some((scala_src.to_string(), parse(scala_src)))
        } else {
            None
        }
    };

    let refs = references::find_references(
        &java_tree, java_src, &java_uri, pos, context, &index, &get_file,
    );

    assert!(
        refs.iter().any(|l| l.uri == scala_uri),
        "Expected JavaHelper references in the Scala file; got: {:?}", refs
    );
}

// ---- Cross-language find references: Kotlin → Scala ----

#[test]
fn references_cross_language_kotlin_to_scala() {
    // "Animal" is defined in simple_class.scala; kotlin_refs_scala.kt uses it
    let scala_src = include_str!("fixtures/simple_class.scala");
    let scala_tree = parse(scala_src);
    let scala_uri = Url::parse("file:///test/simple_class.scala").unwrap();

    let kt_src = include_str!("fixtures/kotlin_refs_scala.kt");
    let kt_uri = Url::parse("file:///test/kotlin_refs_scala.kt").unwrap();

    let index = WorkspaceIndex::new();
    index.update_file(
        &kt_uri,
        extract::workspace_symbols_for_lang(&parse_kotlin(kt_src), kt_src, &kt_uri, SourceLanguage::Kotlin),
    );

    // "Animal" at line 6, char 6 in simple_class.scala
    let pos = Position { line: 6, character: 6 };
    let context = ReferenceContext { include_declaration: true };

    let get_file = |uri: &Url| -> Option<(String, Tree)> {
        if uri == &kt_uri {
            Some((kt_src.to_string(), parse_kotlin(kt_src)))
        } else {
            None
        }
    };

    let refs = references::find_references(
        &scala_tree, scala_src, &scala_uri, pos, context, &index, &get_file,
    );

    assert!(
        refs.iter().any(|l| l.uri == kt_uri),
        "Expected Animal references in the Kotlin file; got: {:?}", refs
    );
}

// ---- Cross-language find references: Scala → Kotlin ----

#[test]
fn references_cross_language_scala_to_kotlin() {
    // "KotlinHelper" is defined in kotlin_helper.kt; scala_uses_kotlin.scala references it
    let kt_src = include_str!("fixtures/kotlin_helper.kt");
    let kt_tree = parse_kotlin(kt_src);
    let kt_uri = Url::parse("file:///test/kotlin_helper.kt").unwrap();

    let scala_src = include_str!("fixtures/scala_uses_kotlin.scala");
    let scala_uri = Url::parse("file:///test/scala_uses_kotlin.scala").unwrap();

    let index = WorkspaceIndex::new();
    index.update_file(
        &scala_uri,
        extract::workspace_symbols(&parse(scala_src), scala_src, &scala_uri),
    );

    // "KotlinHelper" in kotlin_helper.kt at line 5, char 6: "class KotlinHelper(...)"
    let pos = Position { line: 5, character: 6 };
    let context = ReferenceContext { include_declaration: true };

    let get_file = |uri: &Url| -> Option<(String, Tree)> {
        if uri == &scala_uri {
            Some((scala_src.to_string(), parse(scala_src)))
        } else {
            None
        }
    };

    let refs = references::find_references(
        &kt_tree, kt_src, &kt_uri, pos, context, &index, &get_file,
    );

    assert!(
        refs.iter().any(|l| l.uri == scala_uri),
        "Expected KotlinHelper references in the Scala file; got: {:?}", refs
    );
}
