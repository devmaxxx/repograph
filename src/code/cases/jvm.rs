//! What the JVM languages share in their cases: a repository on disk, and the calls, supertypes
//! and annotations that cross from Kotlin to Java and back.

pub(crate) mod fixture {
    use crate::code::imports::Resolver;
    use crate::code::CodeExtractor;
    use crate::config::Config;
    use crate::model::{EdgeKind, Extraction, Extractor};

    /// A repository on disk, so the resolver indexes every file the way a build does.
    pub(crate) struct Repo {
        dir: tempfile::TempDir,
    }

    impl Repo {
        pub(crate) fn new(files: &[(&str, &str)]) -> Repo {
            let dir = tempfile::tempdir().unwrap();
            for (p, c) in files {
                let full = dir.path().join(p);
                std::fs::create_dir_all(full.parent().unwrap()).unwrap();
                std::fs::write(full, c).unwrap();
            }
            Repo { dir }
        }

        /// The file at `rel` as `build` extracts it. The JVM globs are set because the index is
        /// filled only for the families `code_globs` reaches, and a case should not widen with the defaults.
        pub(crate) fn extract(&self, rel: &str) -> Extraction {
            let cfg = Config { code_globs: vec!["**/*.kt".into(), "**/*.java".into()], ..Config::default() };
            let text = std::fs::read_to_string(self.dir.path().join(rel)).unwrap();
            CodeExtractor::new(Resolver::new(self.dir.path(), &cfg).unwrap()).extract(rel, &text)
        }
    }

    pub(crate) fn one(rel: &str, src: &str) -> Extraction {
        Repo::new(&[(rel, src)]).extract(rel)
    }

    pub(crate) fn ids(ex: &Extraction) -> Vec<&str> {
        ex.nodes.iter().map(|n| n.id.as_str()).collect()
    }

    pub(crate) fn edges(ex: &Extraction, kind: EdgeKind) -> Vec<(&str, &str, &str)> {
        ex.edges.iter().filter(|e| e.kind == kind).map(|e| (e.source.as_str(), e.target.as_str(), e.context.as_str())).collect()
    }
}

mod cross {
    use super::fixture::{edges, Repo};
    use crate::model::EdgeKind;

    const INVOICE: &str = "package shop.billing;\n\npublic class Invoice {\n    public void send() {}\n}\n";

    #[test]
    fn a_kotlin_class_is_decorated_by_a_java_annotation_type_it_imports() {
        let repo = Repo::new(&[
            ("shop/meta/Audited.java", "package shop.meta;\n\npublic @interface Audited {}\n"),
            ("app/Job.kt", "package app\n\nimport shop.meta.Audited\n\n@Audited\nclass Job\n"),
        ]);
        let ex = repo.extract("app/Job.kt");
        assert!(edges(&ex, EdgeKind::DecoratedBy).contains(&("sym:app/Job.kt::Job", "sym:shop/meta/Audited.java::Audited", "")), "{:?}", ex.edges);
    }

    #[test]
    fn a_kotlin_class_extends_a_java_class_it_imports() {
        let repo = Repo::new(&[
            ("shop/billing/Invoice.java", INVOICE),
            ("app/Draft.kt", "package app\n\nimport shop.billing.Invoice\n\nclass Draft : Invoice()\n"),
        ]);
        let ex = repo.extract("app/Draft.kt");
        assert!(edges(&ex, EdgeKind::Imports).contains(&("file:app/Draft.kt", "file:shop/billing/Invoice.java", "Invoice")), "{:?}", ex.edges);
        assert!(edges(&ex, EdgeKind::Extends).contains(&("sym:app/Draft.kt::Draft", "sym:shop/billing/Invoice.java::Invoice", "")), "{:?}", ex.edges);
    }

    #[test]
    fn a_java_class_implements_a_kotlin_interface_it_imports() {
        let repo = Repo::new(&[
            ("app/sync/Wipe.kt", "package app.sync\n\ninterface Wipe {\n    fun run()\n}\n"),
            ("shop/Job.java", "package shop;\n\nimport app.sync.Wipe;\n\npublic class Job implements Wipe {\n    public void run() {}\n}\n"),
        ]);
        let ex = repo.extract("shop/Job.java");
        assert!(edges(&ex, EdgeKind::Imports).contains(&("file:shop/Job.java", "file:app/sync/Wipe.kt", "Wipe")), "{:?}", ex.edges);
        assert!(edges(&ex, EdgeKind::Extends).contains(&("sym:shop/Job.java::Job", "sym:app/sync/Wipe.kt::Wipe", "")), "{:?}", ex.edges);
    }

    #[test]
    fn a_kotlin_call_reaches_a_java_method_through_a_typed_property() {
        let repo = Repo::new(&[
            ("shop/orders/OrderService.java", "package shop.orders;\n\npublic class OrderService {\n    public void place(int n) {}\n}\n"),
            ("app/Checkout.kt", "package app\n\nimport shop.orders.OrderService\n\nclass Checkout(private val orders: OrderService) {\n    fun pay() { orders.place(1) }\n}\n"),
        ]);
        let ex = repo.extract("app/Checkout.kt");
        assert!(edges(&ex, EdgeKind::Calls).contains(&("sym:app/Checkout.kt::Checkout.pay", "sym:shop/orders/OrderService.java::OrderService.place", "")), "{:?}", ex.edges);
    }

    #[test]
    fn a_java_call_reaches_a_kotlin_method_through_a_typed_field() {
        let repo = Repo::new(&[
            ("app/network/Tokens.kt", "package app.network\n\nclass SessionTokens {\n    fun read(): String = \"\"\n}\n"),
            ("shop/Login.java", "package shop;\n\nimport app.network.SessionTokens;\n\npublic class Login {\n    private final SessionTokens tokens = null;\n    public void go() { tokens.read(); }\n}\n"),
        ]);
        let ex = repo.extract("shop/Login.java");
        assert!(edges(&ex, EdgeKind::Calls).contains(&("sym:shop/Login.java::Login.go", "sym:app/network/Tokens.kt::SessionTokens.read", "")), "{:?}", ex.edges);
    }

    /// Two files share the qualified name `shared.Base`, as an `expect`/`actual` pair does. One
    /// candidate's `run` is a class level a subtype outside its package may not fully inherit
    /// (`Inherits::Some`, refused); the other's is a clean public match. The call is no edge
    /// whatever the other candidate holds — it must not settle for the clean one alone.
    #[test]
    fn a_refused_class_candidate_beside_a_declaring_one_is_no_edge() {
        let repo = Repo::new(&[
            ("shared/Base1.java", "package shared;\n\npublic class Base {\n    public void run(int a) {}\n    void run(String s) {}\n}\n"),
            ("shared/Base2.java", "package shared;\n\npublic class Base {\n    public void run(int n) {}\n}\n"),
            ("app/Leaf.java", "package app;\n\nimport shared.Base;\n\npublic class Leaf extends Base {\n    public void go() { run(5); }\n}\n"),
        ]);
        let ex = repo.extract("app/Leaf.java");
        assert!(edges(&ex, EdgeKind::Calls).is_empty(), "{:?}", ex.edges);
    }

    #[test]
    fn a_kotlin_receiver_typed_by_a_java_subclass_binds_the_java_base_s_method() {
        let repo = Repo::new(&[
            ("shop/Base.java", "package shop;\n\npublic class Base {\n    public void run(int x) {}\n    void help() {}\n}\n"),
            ("shop/Sub.java", "package shop;\n\npublic class Sub extends Base {}\n"),
            ("app/U.kt", "package app\n\nimport shop.Sub\n\nclass U(private val s: Sub) {\n    fun go() { s.run(1) }\n    fun none() { s.run() }\n    fun help() { s.help() }\n}\n"),
        ]);
        let ex = repo.extract("app/U.kt");
        let calls = edges(&ex, EdgeKind::Calls);
        assert!(calls.contains(&("sym:app/U.kt::U.go", "sym:shop/Base.java::Base.run", "")), "{:?}", ex.edges);
        assert!(!calls.iter().any(|(s, _, _)| *s == "sym:app/U.kt::U.none"), "no declaration takes no argument: {:?}", ex.edges);
        assert!(!calls.iter().any(|(s, _, _)| *s == "sym:app/U.kt::U.help"), "a package-private method stays in its package: {:?}", ex.edges);
    }

    /// Pins the contract's empty-scope rule: a file with no package line is indexed by bare name.
    #[test]
    fn a_class_in_the_default_package_resolves_by_its_bare_name() {
        let repo = Repo::new(&[("Base.java", "public class Base {}\n"), ("Use.kt", "class Use : Base()\n")]);
        let ex = repo.extract("Use.kt");
        assert!(edges(&ex, EdgeKind::Extends).contains(&("sym:Use.kt::Use", "sym:Base.java::Base", "")), "{:?}", ex.edges);
    }
}
