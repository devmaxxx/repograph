use crate::code::imports::Resolver;
use crate::model::{EdgeKind, Extraction, NodeKind};
use tree_sitter::{Language, Node, Parser};

pub struct SymbolScanner {
    resolver: Resolver,
}

pub fn language_for(rel: &str) -> Language {
    if rel.ends_with(".tsx") {
        Language::new(tree_sitter_typescript::LANGUAGE_TSX)
    } else {
        Language::new(tree_sitter_typescript::LANGUAGE_TYPESCRIPT)
    }
}

pub fn parse(rel: &str, src: &[u8]) -> Option<tree_sitter::Tree> {
    let mut parser = Parser::new();
    parser.set_language(&language_for(rel)).ok()?;
    parser.parse(src, None)
}

fn text<'a>(n: Node, src: &'a [u8]) -> &'a str {
    n.utf8_text(src).unwrap_or("")
}

fn name_of(n: Node, src: &[u8]) -> Option<String> {
    n.child_by_field_name("name").map(|c| text(c, src).to_string())
}

/// A one-line preview of a declaration: every physical line trimmed and rejoined with a
/// single space, so a multi-line function body still reads as one signature string.
fn flatten(s: &str) -> String {
    s.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ")
}

fn unquote(s: &str) -> String {
    s.trim_matches(|c| c == '\'' || c == '"' || c == '`').to_string()
}

/// The name a class member is known by: `'quoted'()` and `['computed']()` lose their quotes,
/// a computed name that is not a string literal (`[Symbol.iterator]`) names nothing.
pub(crate) fn member_name(m: Node, src: &[u8]) -> Option<String> {
    let name = m.child_by_field_name("name")?;
    match name.kind() {
        "string" => Some(unquote(text(name, src))),
        "computed_property_name" => {
            let inner = name.named_child(0)?;
            (inner.kind() == "string").then(|| unquote(text(inner, src)))
        }
        _ => Some(text(name, src).to_string()),
    }
}

/// Every identifier a binding pattern introduces: `{ x, y: why, ...rest }`, `[a, , b = 1]`.
fn bindings(n: Node, src: &[u8], out: &mut Vec<String>) {
    match n.kind() {
        "identifier" | "shorthand_property_identifier_pattern" => out.push(text(n, src).to_string()),
        "pair_pattern" => {
            if let Some(v) = n.child_by_field_name("value") {
                bindings(v, src, out);
            }
        }
        "assignment_pattern" => {
            if let Some(l) = n.child_by_field_name("left") {
                bindings(l, src, out);
            }
        }
        _ => {
            let mut cur = n.walk();
            for c in n.named_children(&mut cur) {
                bindings(c, src, out);
            }
        }
    }
}

/// Whether a declaration sits directly in the program — possibly behind `export` or
/// `declare` — rather than inside a function or a block.
pub(crate) fn is_top_level(decl: Node) -> bool {
    let mut p = decl.parent();
    while let Some(n) = p {
        match n.kind() {
            "program" => return true,
            "export_statement" | "ambient_declaration" | "lexical_declaration" | "variable_declaration" => p = n.parent(),
            _ => return false,
        }
    }
    false
}

/// First string literal inside a decorator's call arguments, without quotes; `""` when none.
fn decorator_arg(call: Node, src: &[u8]) -> String {
    let Some(args) = call.child_by_field_name("arguments") else { return String::new() };
    let mut cur = args.walk();
    for a in args.named_children(&mut cur) {
        if matches!(a.kind(), "string" | "template_string") {
            return unquote(text(a, src));
        }
    }
    String::new()
}

/// A decorator's callee name and its first string-literal argument (`""` for a bare
/// decorator or one with no string argument).
fn decorator_pair(deco: Node, src: &[u8]) -> (String, String) {
    let mut dc = deco.walk();
    let Some(inner) = deco.named_children(&mut dc).next() else { return (String::new(), String::new()) };
    match inner.kind() {
        "call_expression" => {
            let name = inner.child_by_field_name("function").map(|f| text(f, src)).unwrap_or("").to_string();
            (name, decorator_arg(inner, src))
        }
        _ => (text(inner, src).to_string(), String::new()),
    }
}

impl SymbolScanner {
    pub fn new(resolver: Resolver) -> SymbolScanner {
        SymbolScanner { resolver }
    }

    pub fn scan(&self, rel: &str, source: &str) -> Extraction {
        let mut ex = Extraction::default();
        let file_id = format!("file:{rel}");
        ex.node(NodeKind::File, &file_id, rel, "", rel, 1);
        let src = source.as_bytes();
        let Some(tree) = parse(rel, src) else { return ex };
        let root = tree.root_node();
        let mut cur = root.walk();
        for stmt in root.named_children(&mut cur) {
            match stmt.kind() {
                "export_statement" => self.export(stmt, rel, &file_id, src, &mut ex),
                "import_statement" => self.import(stmt, rel, &file_id, src, &mut ex),
                "function_declaration" | "function_signature" | "class_declaration" | "abstract_class_declaration"
                | "interface_declaration" | "type_alias_declaration" | "enum_declaration" | "internal_module"
                | "lexical_declaration" | "variable_declaration" | "ambient_declaration" => {
                    self.declaration(stmt, rel, &file_id, src, false, &mut ex);
                }
                _ => {}
            }
        }
        self.export_clauses(root, &file_id, src, &mut ex);
        self.dynamic_imports(root, rel, src, &mut ex);
        ex.edges.sort();
        ex.edges.dedup();
        ex
    }

    fn export(&self, stmt: Node, rel: &str, file_id: &str, src: &[u8], ex: &mut Extraction) {
        if let Some(source) = stmt.child_by_field_name("source") {
            // `export * from` / `export { a, b } from`: a barrel edge, not a symbol.
            let spec = text(source, src).trim_matches(|c| c == '\'' || c == '"');
            let Some(target) = self.resolver.resolve(rel, spec) else { return };
            let mut names = Vec::new();
            let mut cur = stmt.walk();
            for c in stmt.named_children(&mut cur) {
                match c.kind() {
                    "export_clause" => {
                        let mut cc = c.walk();
                        for s in c.named_children(&mut cc) {
                            if s.kind() == "export_specifier" {
                                if let Some(n) = name_of(s, src) {
                                    names.push(n);
                                }
                            }
                        }
                    }
                    "namespace_export" => names.push(c.named_child(0).map(|a| text(a, src)).unwrap_or("*").to_string()),
                    _ => {}
                }
            }
            if names.is_empty() {
                names.push("*".into());
            }
            ex.edge(file_id, &format!("file:{target}"), EdgeKind::ReExports, &names.join(","), rel);
            return;
        }
        if let Some(decl) = stmt.child_by_field_name("declaration") {
            // A decorator directly before `export class …` attaches to the export_statement
            // itself, not to the class_declaration it wraps (verified from the parse tree).
            let decorators = self.decorators_of(stmt, src);
            let created = self.declaration(decl, rel, file_id, src, true, ex);
            for sym in &created {
                for (name, arg) in &decorators {
                    ex.node(NodeKind::Symbol, &format!("deco:{name}"), name, "", rel, stmt.start_position().row as u32 + 1);
                    ex.edge(sym, &format!("deco:{name}"), EdgeKind::DecoratedBy, arg, rel);
                }
            }
        }
    }

    /// Returns the symbol ids created at top level, so decorators on `export class` attach.
    fn declaration(&self, decl: Node, rel: &str, file_id: &str, src: &[u8], exported: bool, ex: &mut Extraction) -> Vec<String> {
        let ctx = if exported { "export" } else { "" };
        // `declare const x` wraps the declaration it describes; `declare module 'm'` is not a
        // symbol of this file and falls through the default arm below with a quoted name.
        let decl = if decl.kind() == "ambient_declaration" { decl.named_child(0).unwrap_or(decl) } else { decl };
        // A decorator is part of a class_declaration's extent; the name is not.
        let line = decl.child_by_field_name("name").unwrap_or(decl).start_position().row as u32 + 1;
        // A function's body is worth flattening into one preview line; a class/interface/enum/
        // const body is not — BM25 documents are built from `id + label + body`, so a
        // multi-kilobyte class body would drown the label terms that make the symbol findable.
        // Header line matches the method/property signature rule below.
        let signature = if decl.kind() == "function_declaration" {
            flatten(text(decl, src))
        } else {
            text(decl, src).lines().next().unwrap_or("").trim().to_string()
        };
        let signature = if exported && !signature.starts_with("export") { format!("export {signature}") } else { signature };
        let mut created = Vec::new();
        let mut declare = |name: &str, ex: &mut Extraction| -> String {
            let id = format!("sym:{rel}::{name}");
            ex.node(NodeKind::Symbol, &id, name, &signature, rel, line);
            ex.edge(file_id, &id, EdgeKind::Declares, ctx, rel);
            created.push(id.clone());
            id
        };
        match decl.kind() {
            "lexical_declaration" | "variable_declaration" => {
                let mut cur = decl.walk();
                for d in decl.named_children(&mut cur) {
                    if d.kind() == "variable_declarator" {
                        let mut names = Vec::new();
                        if let Some(pat) = d.child_by_field_name("name") {
                            bindings(pat, src, &mut names);
                        }
                        for n in names {
                            declare(&n, ex);
                        }
                    }
                }
            }
            "class_declaration" | "abstract_class_declaration" => {
                let Some(name) = name_of(decl, src) else { return created };
                let class_id = declare(&name, ex);
                self.class_body(decl, &class_id, rel, src, ex);
                for (dname, arg) in self.decorators_of(decl, src) {
                    ex.node(NodeKind::Symbol, &format!("deco:{dname}"), &dname, "", rel, line);
                    ex.edge(&class_id, &format!("deco:{dname}"), EdgeKind::DecoratedBy, &arg, rel);
                }
            }
            _ => {
                if let Some(n) = name_of(decl, src).filter(|n| !n.starts_with(['\'', '"'])) {
                    declare(&n, ex);
                }
            }
        }
        created
    }

    fn class_body(&self, class: Node, class_id: &str, rel: &str, src: &[u8], ex: &mut Extraction) {
        let mut cur = class.walk();
        for c in class.named_children(&mut cur) {
            if c.kind() == "class_heritage" {
                let mut hc = c.walk();
                for h in c.named_children(&mut hc) {
                    if h.kind() == "extends_clause" {
                        if let Some(base) = h.child_by_field_name("value") {
                            let base = text(base, src).trim();
                            let file = self.import_origin(class, base, rel, src).unwrap_or_else(|| rel.to_string());
                            ex.edge(class_id, &format!("sym:{file}::{base}"), EdgeKind::Extends, "", rel);
                        }
                    }
                }
            }
        }
        let Some(body) = class.child_by_field_name("body") else { return };
        let mut bc = body.walk();
        // A decorator on a method or accessor is a sibling of the member in `class_body`,
        // not a child of the `method_definition` itself (verified from the parse tree) —
        // so it is queued here and drained onto the next member we see.
        let mut pending: Vec<(String, String)> = Vec::new();
        for m in body.named_children(&mut bc) {
            if m.kind() == "decorator" {
                pending.push(decorator_pair(m, src));
                continue;
            }
            if !matches!(m.kind(), "method_definition" | "public_field_definition" | "abstract_method_signature") {
                // Anything else (an index signature, a static block) breaks the lexical
                // chain; decorators queued before it don't belong to what follows it.
                pending.clear();
                continue;
            }
            let Some(name) = member_name(m, src) else {
                pending.clear();
                continue;
            };
            let class_name = class_id.rsplit("::").next().unwrap_or("");
            let id = format!("sym:{rel}::{class_name}.{name}");
            let signature = text(m, src).lines().find(|l| !l.trim_start().starts_with('@')).unwrap_or("").trim().to_string();
            ex.node(NodeKind::Symbol, &id, &format!("{class_name}.{name}"), &signature, rel, m.start_position().row as u32 + 1);
            ex.edge(class_id, &id, EdgeKind::Declares, "", rel);

            // `method_definition`/`abstract_method_signature` have no `decorator` field of
            // their own — the grammar hoists it to `class_body`, hence `pending` — while
            // `public_field_definition` has one; the two sources are mutually exclusive per
            // member kind, so no cross-source dedup is needed.
            let decos = self.decorators_of(m, src).into_iter().chain(pending.drain(..));
            for (dname, arg) in decos {
                ex.node(NodeKind::Symbol, &format!("deco:{dname}"), &dname, "", rel, m.start_position().row as u32 + 1);
                ex.edge(&id, &format!("deco:{dname}"), EdgeKind::DecoratedBy, &arg, rel);
            }
        }
    }

    /// Decorator children of a node found directly as its own named children — the
    /// `class_declaration`/`export_statement` and `public_field_definition` case.
    fn decorators_of(&self, n: Node, src: &[u8]) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut cur = n.walk();
        for c in n.named_children(&mut cur) {
            if c.kind() == "decorator" {
                out.push(decorator_pair(c, src));
            }
        }
        out
    }

    /// The file an identifier was imported from, if it was imported at all.
    fn import_origin(&self, any: Node, ident: &str, rel: &str, src: &[u8]) -> Option<String> {
        let mut root = any;
        while let Some(p) = root.parent() {
            root = p;
        }
        let mut cur = root.walk();
        for stmt in root.named_children(&mut cur) {
            if stmt.kind() != "import_statement" {
                continue;
            }
            if !text(stmt, src).split(|c: char| !c.is_alphanumeric() && c != '_').any(|t| t == ident) {
                continue;
            }
            let spec = text(stmt.child_by_field_name("source")?, src).trim_matches(|c| c == '\'' || c == '"');
            return self.resolver.resolve(rel, spec);
        }
        None
    }

    fn import(&self, stmt: Node, rel: &str, file_id: &str, src: &[u8], ex: &mut Extraction) {
        // `import x = require('y')` keeps its source on the clause, not the statement.
        let mut cur = stmt.walk();
        let require = stmt.named_children(&mut cur).find(|c| c.kind() == "import_require_clause");
        let Some(source) = stmt.child_by_field_name("source").or_else(|| require?.child_by_field_name("source")) else { return };
        let spec = unquote(text(source, src));
        let Some(target) = self.resolver.resolve(rel, &spec) else { return };
        let mut names = Vec::new();
        let mut cur = stmt.walk();
        for c in stmt.named_children(&mut cur) {
            if c.kind() == "import_require_clause" {
                names.extend(c.named_child(0).map(|n| text(n, src).to_string()));
                continue;
            }
            if c.kind() != "import_clause" {
                continue;
            }
            let mut ic = c.walk();
            for part in c.named_children(&mut ic) {
                match part.kind() {
                    "identifier" => names.push(text(part, src).to_string()),
                    "named_imports" => {
                        let mut nc = part.walk();
                        for s in part.named_children(&mut nc) {
                            if s.kind() == "import_specifier" {
                                if let Some(n) = name_of(s, src) {
                                    names.push(n);
                                }
                            }
                        }
                    }
                    "namespace_import" => names.push("*".into()),
                    _ => {}
                }
            }
        }
        ex.edge(file_id, &format!("file:{target}"), EdgeKind::Imports, &names.join(","), rel);
    }

    /// `export { a, b as c }`, `export default a` and `export = a` export symbols declared
    /// elsewhere in the file: their `declares` edges are upgraded rather than re-created.
    fn export_clauses(&self, root: Node, file_id: &str, src: &[u8], ex: &mut Extraction) {
        let mut names: Vec<String> = Vec::new();
        let mut cur = root.walk();
        for stmt in root.named_children(&mut cur) {
            if stmt.kind() != "export_statement" || stmt.child_by_field_name("source").is_some() || stmt.child_by_field_name("declaration").is_some() {
                continue;
            }
            let mut sc = stmt.walk();
            for c in stmt.named_children(&mut sc) {
                match c.kind() {
                    "identifier" => names.push(text(c, src).to_string()),
                    "export_clause" => {
                        let mut cc = c.walk();
                        names.extend(c.named_children(&mut cc).filter(|s| s.kind() == "export_specifier").filter_map(|s| name_of(s, src)));
                    }
                    _ => {}
                }
            }
        }
        for e in &mut ex.edges {
            if e.kind == EdgeKind::Declares && e.source == file_id {
                if let Some(name) = e.target.rsplit("::").next() {
                    if names.iter().any(|n| n == name) {
                        e.context = "export".into();
                    }
                }
            }
        }
    }

    /// `import('./x')` and `require('./x')` anywhere in the file, attributed to the symbol
    /// they sit in so a lazy route or a test's fixture load still reaches its file.
    fn dynamic_imports(&self, root: Node, rel: &str, src: &[u8], ex: &mut Extraction) {
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            let mut cur = n.walk();
            stack.extend(n.named_children(&mut cur));
            if n.kind() != "call_expression" {
                continue;
            }
            let Some(callee) = n.child_by_field_name("function") else { continue };
            if !(callee.kind() == "import" || text(callee, src) == "require") {
                continue;
            }
            let Some(arg) = n.child_by_field_name("arguments").and_then(|a| a.named_child(0)) else { continue };
            if arg.kind() != "string" {
                continue;
            }
            let Some(target) = self.resolver.resolve(rel, &unquote(text(arg, src))) else { continue };
            let from = crate::code::idrefs::owner(n, rel, src);
            ex.edge(&from, &format!("file:{target}"), EdgeKind::Imports, "", rel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(rel: &str, fixture: &str) -> Extraction {
        let d = tempfile::tempdir().unwrap();
        for f in [
            "apps/api/src/shared/audit/index.ts",
            "apps/api/src/shared/rbac/index.ts",
            "apps/api/src/modules/staff/staff.service.ts",
            "apps/api/src/modules/staff/staff.types.ts",
        ] {
            let p = d.path().join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "export {};\n").unwrap();
        }
        let text = std::fs::read_to_string(format!("{}/tests/fixtures/{fixture}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        SymbolScanner::new(Resolver::new(d.path()).unwrap()).scan(rel, &text)
    }

    fn has(ex: &Extraction, s: &str, t: &str, k: EdgeKind, ctx: &str) -> bool {
        ex.edges.iter().any(|e| e.source == s && e.target == t && e.kind == k && e.context == ctx)
    }

    const C: &str = "apps/api/src/modules/staff/staff.controller.ts";

    #[test]
    fn exports_and_class_members() {
        let ex = scan(C, "staff.controller.ts");
        let ids: Vec<&str> = ex.nodes.iter().map(|n| n.id.as_str()).collect();
        for s in ["StaffController", "helper", "LIMIT", "CreateStaffDto", "Role"] {
            assert!(ids.contains(&format!("sym:{C}::{s}").as_str()), "{s}");
        }
        assert!(ids.contains(&format!("sym:{C}::StaffController.create").as_str()));
        let cls = ex.nodes.iter().find(|n| n.id == format!("sym:{C}::StaffController")).unwrap();
        assert_eq!(cls.kind, NodeKind::Symbol);
        assert_eq!(cls.line, 8);
        assert_eq!(cls.label, "StaffController");
        assert!(has(&ex, &format!("file:{C}"), &format!("sym:{C}::StaffController"), EdgeKind::Declares, "export"));
        assert!(has(&ex, &format!("sym:{C}::StaffController"), &format!("sym:{C}::StaffController.create"), EdgeKind::Declares, ""));
    }

    #[test]
    fn decorators_carry_their_first_string_argument() {
        let ex = scan(C, "staff.controller.ts");
        let m = format!("sym:{C}::StaffController.create");
        assert!(has(&ex, &m, "deco:RequireAction", EdgeKind::DecoratedBy, "staff.manage"));
        assert!(has(&ex, &m, "deco:Audited", EdgeKind::DecoratedBy, ""));
        assert!(has(&ex, &m, "deco:Post", EdgeKind::DecoratedBy, ""));
        assert!(has(&ex, &format!("sym:{C}::StaffController"), "deco:Controller", EdgeKind::DecoratedBy, "staff"));
        assert!(ex.nodes.iter().any(|n| n.id == "deco:RequireAction" && n.kind == NodeKind::Symbol));
    }

    /// A decorator on a method sits beside it in `class_body`, not inside the
    /// `method_definition`; a decorator on a property sits inside `public_field_definition`.
    #[test]
    fn decorator_on_a_class_property_is_recorded() {
        let ex = scan(C, "staff.controller.ts");
        let p = format!("sym:{C}::StaffController.logger");
        assert!(ex.nodes.iter().any(|n| n.id == p), "property symbol missing");
        assert!(has(&ex, &p, "deco:Inject", EdgeKind::DecoratedBy, "LOGGER"));
    }

    #[test]
    fn imports_and_re_exports_resolve_to_files() {
        let ex = scan(C, "staff.controller.ts");
        assert!(has(&ex, &format!("file:{C}"), "file:apps/api/src/shared/rbac/index.ts", EdgeKind::Imports, "RequireAction"));
        assert!(has(&ex, &format!("file:{C}"), "file:apps/api/src/modules/staff/staff.types.ts", EdgeKind::ReExports, "*"));
        assert!(has(&ex, &format!("file:{C}"), "file:apps/api/src/modules/staff/staff.service.ts", EdgeKind::ReExports, "StaffService"));
        assert!(!ex.edges.iter().any(|e| e.target.contains("nestjs")));
    }

    #[test]
    fn extends_targets_a_symbol_in_the_same_file_when_unresolved() {
        let ex = scan(C, "staff.controller.ts");
        assert!(has(&ex, &format!("sym:{C}::StaffController"), &format!("sym:{C}::BaseController"), EdgeKind::Extends, ""));
    }

    #[test]
    fn non_exported_functions_are_declared_without_export_context() {
        let ex = scan("packages/contracts/src/money.ts", "money.ts");
        assert!(has(&ex, "file:packages/contracts/src/money.ts", "sym:packages/contracts/src/money.ts::asGrosze", EdgeKind::Declares, "export"));
        assert!(has(&ex, "file:packages/contracts/src/money.ts", "sym:packages/contracts/src/money.ts::internal", EdgeKind::Declares, ""));
        let n = ex.nodes.iter().find(|n| n.id.ends_with("::asGrosze")).unwrap();
        assert_eq!(n.body, "export function asGrosze(v: number): number { return Math.round(v * 100); }");
    }
}
