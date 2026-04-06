# blisk — Best-Effort Scala LSP Server

## Description

blisk is a fast, zero-configuration Scala language server built in Rust on top of tree-sitter-scala. It requires no build tool integration and no JVM, starts instantly, and targets the sweet spot between a plain text search and a full-featured server like Metals.

## Features

### Tree-sitter (solid)
1. `textDocument/documentSymbol` — hierarchical: classes, traits, objects, defs, vals, types
2. `workspace/symbol` — search all top-level definitions across the workspace
3. `textDocument/foldingRange` — blocks, import groups, multi-line comments
4. `textDocument/selectionRange` — expand/shrink selection by AST node
5. `textDocument/semanticTokens/full` — syntax-aware highlighting
6. `textDocument/publishDiagnostics` — parse/syntax errors only
7. `textDocument/documentLink` — URLs in comments and strings

### Heuristic-based
1. `textDocument/definition` — same-file scope walk first, cross-file via workspace symbol index fallback
2. `textDocument/references` — same-file tree walk + cross-file pre-filter then parse
