# blisk — Best-Effort Scala LSP Server

## Description

blisk is a fast, zero-configuration Scala language server built in Rust on top of tree-sitter-scala. It requires no build tool integration and no JVM, starts instantly, and targets the sweet spot between a plain text search and a full-featured server like Metals.

blisk was primarily created to be used with [Fresh](https://getfresh.dev), an open-source terminal text editor with LSP support.

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

## Dependency Source Indexing

blisk can fetch, extract, and index source jars for your project's dependencies so that go-to-definition and workspace symbol search work across library code.

**Prerequisites:** `sbt` with the [sbt-dependency-graph](https://www.scala-sbt.org/sbt-dependency-graph/) plugin, and [Coursier](https://get-coursier.io/) (`cs`) on your PATH.

**How it works:**
1. Runs `sbt projects` to discover local subprojects (excluded from fetch)
2. Runs `sbt dependencyList` to collect external dependency coordinates
3. Fetches source jars via `cs fetch --sources`
4. Extracts `.scala` files into `.dep-srcs/` at the workspace root
5. Indexes the extracted sources into the workspace symbol index

Fetching is incremental — already-resolved coordinates are recorded in `.dep-srcs/.resolved.list` and skipped on subsequent runs.

**Enable via CLI (standalone mode):**
```
blisk --fetch-dep-sources /path/to/project
```
This runs the fetch and exits without starting the LSP server, useful for pre-populating `.dep-srcs/` before connecting an editor.

**Enable via LSP initialization options:**
```json
{
  "initializationOptions": { "retrieveSrc": true }
}
```
When set, the fetch runs in the background after the server initializes, in parallel with the regular workspace scan.
