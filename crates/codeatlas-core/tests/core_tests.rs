use std::path::Path;
use codeatlas_core::models::{Edge, EdgeType, Symbol, SymbolKind};
use codeatlas_core::parser::{split_identifier, AstParser};
use codeatlas_core::graph::CodeGraph;
use codeatlas_core::storage::Database;
use codeatlas_core::retrieval::Retriever;
use codeatlas_core::wiki::WikiGenerator;

#[test]
fn test_split_identifier() {
    assert_eq!(split_identifier("camelCase"), vec!["camel", "case"]);
    assert_eq!(split_identifier("PascalCase"), vec!["pascal", "case"]);
    assert_eq!(split_identifier("snake_case_name"), vec!["snake", "case", "name"]);
    assert_eq!(split_identifier("HTTPResponse"), vec!["http", "response"]);
}

#[test]
fn test_ast_python_parser() {
    let code = r#"
class RequestHandler(BaseHandler):
    """Handles HTTP requests."""
    def handle(self, request):
        return self.process(request)

def standalone_function():
    handler = RequestHandler()
    handler.handle("test")
"#;
    let parser = AstParser::new();
    let (symbols, chunks, edges) = parser.parse_file(Path::new("server.py"), code, "python");

    // Module, RequestHandler, handle, standalone_function
    assert!(symbols.iter().any(|s| s.name == "server" && s.kind == SymbolKind::Module));
    assert!(symbols.iter().any(|s| s.name == "RequestHandler" && s.kind == SymbolKind::Class));
    assert!(symbols.iter().any(|s| s.name == "handle" && s.kind == SymbolKind::Method));
    assert!(symbols.iter().any(|s| s.name == "standalone_function" && s.kind == SymbolKind::Function));

    // Chunks check
    assert!(chunks.iter().any(|c| c.identifiers.contains(&"RequestHandler".to_string())));
    assert!(chunks.iter().any(|c| c.identifiers.contains(&"handle".to_string())));

    // Edges check
    assert!(edges.iter().any(|e| e.edge_type == EdgeType::Inherits));
    assert!(edges.iter().any(|e| e.edge_type == EdgeType::Calls));
}

#[test]
fn test_storage_and_fts() {
    let mut db = Database::open_in_memory().unwrap();

    let parser = AstParser::new();
    let py_code = r#"
def authenticate_user(username, password):
    """Authenticate a user given credentials."""
    return True
"#;
    let (symbols, chunks, edges) = parser.parse_file(Path::new("auth.py"), py_code, "python");

    db.insert_symbols(&symbols).unwrap();
    db.insert_chunks(&chunks).unwrap();
    db.insert_edges(&edges).unwrap();

    let results = db.search_lexical("authenticate credentials", 10).unwrap();
    assert!(!results.is_empty());
}

#[test]
fn test_graph_and_impact() {
    let mut graph = CodeGraph::new();

    let sym_a = Symbol {
        node_id: "mod:a".to_string(),
        file_path: "a.py".to_string(),
        kind: SymbolKind::Function,
        name: "func_a".to_string(),
        parent_symbol: None,
        signature: None,
        docstring: None,
        start_line: 1,
        end_line: 5,
        source_code: "".to_string(),
        content_hash: "hasha".to_string(),
        language: "python".to_string(),
        calls: vec!["func_b".to_string()],
        referenced_types: vec![],
    };

    let sym_b = Symbol {
        node_id: "mod:b".to_string(),
        file_path: "b.py".to_string(),
        kind: SymbolKind::Function,
        name: "func_b".to_string(),
        parent_symbol: None,
        signature: None,
        docstring: None,
        start_line: 1,
        end_line: 5,
        source_code: "".to_string(),
        content_hash: "hashb".to_string(),
        language: "python".to_string(),
        calls: vec![],
        referenced_types: vec![],
    };

    graph.add_symbol(&sym_a);
    graph.add_symbol(&sym_b);

    // a -> calls -> b
    graph.add_edge(&Edge {
        source_id: "mod:a".to_string(),
        target_id: "mod:b".to_string(),
        edge_type: EdgeType::Calls,
        metadata: None,
    });

    // Impact of modifying func_b should include func_a (reverse BFS)
    let affected = graph.impact_analysis("mod:b");
    assert_eq!(affected, vec!["mod:a"]);

    // Neighborhood expansion of mod:a should include mod:b
    let neighbors = graph.expand_neighborhood(&["mod:a".to_string()], 1);
    assert!(neighbors.contains(&"mod:b".to_string()));
}

#[test]
fn test_retrieval_and_ranking() {
    let mut db = Database::open_in_memory().unwrap();
    let mut graph = CodeGraph::new();

    let parser = AstParser::new();
    let code_prod = "def render_template(name, context):\n    return f'rendered {name}'\n";
    let code_test = "def test_render_template():\n    assert render_template('index', {}) == 'rendered index'\n";

    let (syms1, chunks1, edges1) = parser.parse_file(Path::new("templating.py"), code_prod, "python");
    let (syms2, chunks2, edges2) = parser.parse_file(Path::new("test_templating.py"), code_test, "python");

    for s in &syms1 { graph.add_symbol(s); }
    for s in &syms2 { graph.add_symbol(s); }
    for e in &edges1 { graph.add_edge(e); }
    for e in &edges2 { graph.add_edge(e); }

    db.insert_symbols(&syms1).unwrap();
    db.insert_chunks(&chunks1).unwrap();
    db.insert_edges(&edges1).unwrap();

    db.insert_symbols(&syms2).unwrap();
    db.insert_chunks(&chunks2).unwrap();
    db.insert_edges(&edges2).unwrap();

    let retriever = Retriever::new(db.conn(), &graph);
    let results = retriever.search("render_template", 5, true).unwrap();

    assert!(!results.is_empty());
    // Production chunk should outrank test chunk due to test penalty (0.35x)
    assert_eq!(results[0].file_path, "templating.py");
}

#[test]
fn test_wiki_generation() {
    let mut db = Database::open_in_memory().unwrap();
    let parser = AstParser::new();

    let code = "class Router:\n    def route(self):\n        pass\n";
    let (syms, chunks, edges) = parser.parse_file(Path::new("router.py"), code, "python");

    db.insert_symbols(&syms).unwrap();
    db.insert_chunks(&chunks).unwrap();
    db.insert_edges(&edges).unwrap();

    let temp_dir = std::env::temp_dir().join("codeatlas_wiki_test");
    let generator = WikiGenerator::new(db.conn(), &temp_dir);
    let generated = generator.generate().unwrap();

    assert!(generated.iter().any(|p| p.ends_with("architecture.md")));
    assert!(generated.iter().any(|p| p.ends_with("index.md")));
    assert!(generated.iter().any(|p| p.ends_with("workflows.md")));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_database_open_file() {
    let temp_db = std::env::temp_dir().join("test_codeatlas_open.db");
    let _ = std::fs::remove_file(&temp_db);
    let db = Database::open(&temp_db).unwrap();
    let stats = db.get_stats().unwrap();
    assert_eq!(stats["files"], 0);
    let _ = std::fs::remove_file(&temp_db);
}

#[test]
fn test_fast_mode_indexing_and_search() {
    use codeatlas_core::Engine;

    let temp_root = std::env::temp_dir().join("codeatlas_fast_test_root");
    let _ = std::fs::remove_dir_all(&temp_root);
    std::fs::create_dir_all(&temp_root).unwrap();

    let sample_code = "def fast_mode_demo():\n    return 'speed'\n";
    std::fs::write(temp_root.join("demo.py"), sample_code).unwrap();

    let temp_db = temp_root.join(".codeatlas").join("index.db");
    let mut engine = Engine::open(&temp_root, &temp_db).unwrap();

    // Cold index with fast mode
    let report1 = engine.index_with_options(false, true).unwrap();
    assert_eq!(report1.files_indexed, 1);
    assert!(report1.symbols_count >= 1);

    // Incremental index with fast mode (should skip because mtime & size match)
    let report2 = engine.index_with_options(false, true).unwrap();
    assert_eq!(report2.files_indexed, 0);

    // Fast search
    let results = engine.search_with_options("fast_mode_demo", 5, false, true).unwrap();
    assert!(!results.is_empty());

    let _ = std::fs::remove_dir_all(&temp_root);
}
