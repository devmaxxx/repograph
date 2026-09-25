//! C# extraction on inline sources, so the grammar's shape is pinned by the assertion.

use crate::code::imports::Resolver;
use crate::code::CodeExtractor;
use crate::config::Config;
use crate::model::{EdgeKind, Extraction, Extractor};

/// The defaults do not glob .NET until L7, and `Resolver::new` collects only globbed families.
pub(crate) fn dotnet_config() -> Config {
    Config { code_globs: vec!["**/*.cs".into(), "**/*.razor".into(), "**/*.cshtml".into()], ..Config::default() }
}

pub(crate) struct Repo {
    pub(crate) dir: tempfile::TempDir,
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

    pub(crate) fn resolver(&self) -> Resolver {
        Resolver::new(self.dir.path(), &dotnet_config()).unwrap()
    }

    /// Extracts `rel` as it is on disk, so the resolver and the extractor read the same bytes.
    pub(crate) fn extract(&self, rel: &str) -> Extraction {
        let src = std::fs::read_to_string(self.dir.path().join(rel)).unwrap();
        CodeExtractor::new(self.resolver()).extract(rel, &src)
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

#[test]
fn a_cs_file_is_csharp_and_a_razor_view_is_razor() {
    use crate::code::lang::{Family, Lang};
    assert_eq!(Lang::of("Shop/OrderService.cs"), Some(Lang::CSharp));
    assert_eq!(Lang::of("Pages/Checkout.razor.cs"), Some(Lang::CSharp));
    assert_eq!(Lang::of("Pages/Checkout.razor"), Some(Lang::Razor));
    assert_eq!(Lang::of("Pages/Index.cshtml"), Some(Lang::Razor));
    assert_eq!(Lang::CSharp.family(), Family::DotNet);
    assert_eq!(Lang::Razor.family(), Family::DotNet);
    assert!(Lang::Razor.grammar().is_none());
    let ex = one("Shop/A.cs", "namespace Shop;\nclass A {}\n");
    assert!(ids(&ex).contains(&"file:Shop/A.cs"), "{:?}", ids(&ex));
}

const ORDERS: &str = r#"namespace Shop.Orders;

/// Places orders.
[Tracked]
public partial class OrderService(IPaymentGateway gateway) : ServiceBase, IOrderService
{
    private readonly IPaymentGateway _gateway;
    internal int Count { get; set; }
    public event EventHandler Changed;
    protected internal event EventHandler<int> Changing { add { } remove { } }
    const int Max = 3, Min = 1;

    public OrderService(IPaymentGateway g, int n) : this(g) { }
    static OrderService() { }

    [HttpGet("x")]
    public Task<Order> Place(int id)
    {
        return null;
    }

    public Task<Order> Place(string code) => null;
    ~OrderService() { }
    public static OrderService operator +(OrderService a, OrderService b) => a;
    public int this[int i] => i;

    public class Line
    {
        void Touch() { }
    }
}

public record Order(int Id, string Name);
public record struct Point(int X, int Y);
readonly struct Coins { }
interface IOrderService { void Place(int id); int Total { get; } }
public enum Status { Open, Closed }
public delegate void PlacedHandler(int x);
public static class CoinsExtensions
{
    public static int ToCoins(this int value) => value;
}
"#;

#[test]
fn types_members_primary_parameters_and_nested_types_are_declared() {
    let ex = one("Shop/Orders.cs", ORDERS);
    for id in [
        "OrderService", "OrderService.gateway", "OrderService._gateway", "OrderService.Count",
        "OrderService.Changed", "OrderService.Changing", "OrderService.Max", "OrderService.Min",
        "OrderService.OrderService", "OrderService.Place", "OrderService.Line", "OrderService.Line.Touch",
        "Order", "Order.Id", "Order.Name", "Point", "Point.X", "Coins", "IOrderService",
        "IOrderService.Place", "IOrderService.Total", "Status", "PlacedHandler", "CoinsExtensions",
        "CoinsExtensions.ToCoins",
    ] {
        let id = format!("sym:Shop/Orders.cs::{id}");
        assert!(ids(&ex).contains(&id.as_str()), "{id} missing from {:?}", ids(&ex));
    }
    assert_eq!(ex.nodes.iter().filter(|n| n.id == "sym:Shop/Orders.cs::OrderService.Place").count(), 1, "overloads are one symbol");
    let odd = ["operator", "this", "~", "Status.", "Shop.Orders"];
    assert!(!ids(&ex).iter().any(|i| odd.iter().any(|o| i.contains(o))), "{:?}", ids(&ex));
}

#[test]
fn visibility_follows_the_csharp_defaults() {
    let src = "namespace Shop;\nclass Implicit { void Hidden() {} public void Shown() {} protected internal void Both() {} private protected void Narrow() {} }\npublic class Open { int field; internal int Inner; }\nfile class Local { }\ninterface IApi { void Call(); }\n";
    let ex = one("Shop/V.cs", src);
    let declares = edges(&ex, EdgeKind::Declares);
    let ctx = |id: &str| {
        let want = format!("sym:Shop/V.cs::{id}");
        declares.iter().find(|(_, t, _)| *t == want).map(|(_, _, c)| c.to_string()).unwrap_or_else(|| panic!("{id} not declared"))
    };
    assert_eq!(ctx("Implicit"), "export", "a top-level type with no modifier is internal, and internal crosses files");
    assert_eq!(ctx("Implicit.Hidden"), "", "a member with no modifier is private");
    assert_eq!(ctx("Implicit.Shown"), "export");
    assert_eq!(ctx("Implicit.Both"), "export");
    assert_eq!(ctx("Implicit.Narrow"), "");
    assert_eq!(ctx("Open.field"), "");
    assert_eq!(ctx("Open.Inner"), "export");
    assert_eq!(ctx("Local"), "", "a file-local type never leaves its file");
    assert_eq!(ctx("IApi.Call"), "export", "an interface member is public without a modifier");
}

#[test]
fn a_member_is_declared_by_its_type_and_spans_its_attribute_lines() {
    let ex = one("Shop/Orders.cs", ORDERS);
    let f = "sym:Shop/Orders.cs::";
    let declares = edges(&ex, EdgeKind::Declares);
    assert!(declares.contains(&("file:Shop/Orders.cs", "sym:Shop/Orders.cs::OrderService", "export")));
    assert!(declares.iter().any(|(s, t, _)| *s == format!("{f}OrderService") && *t == format!("{f}OrderService.Line")));
    assert!(declares.iter().any(|(s, t, _)| *s == format!("{f}OrderService.Line") && *t == format!("{f}OrderService.Line.Touch")));
    let at = |id: &str| {
        let n = ex.nodes.iter().find(|n| n.id == format!("{f}{id}")).unwrap();
        (n.line, n.end)
    };
    assert_eq!(at("OrderService"), (4, 31));
    assert_eq!(at("OrderService.Place"), (16, 20), "the first overload keeps the id, its attribute line included");
    let body = &ex.nodes.iter().find(|n| n.id == format!("{f}OrderService")).unwrap().body;
    assert_eq!(body, "Places orders.\npublic partial class OrderService(IPaymentGateway gateway) : ServiceBase, IOrderService");
}

#[test]
fn each_file_declaring_part_of_a_partial_type_keeps_its_own_id() {
    let repo = Repo::new(&[
        ("Shop/OrderService.Bedrock.cs", "namespace Shop;\npublic partial class OrderService { public void Place() {} }\n"),
        ("Shop/OrderService.Billing.cs", "namespace Shop;\npartial class OrderService { void Bill() {} }\n"),
    ]);
    assert!(ids(&repo.extract("Shop/OrderService.Bedrock.cs")).contains(&"sym:Shop/OrderService.Bedrock.cs::OrderService.Place"));
    assert!(ids(&repo.extract("Shop/OrderService.Billing.cs")).contains(&"sym:Shop/OrderService.Billing.cs::OrderService.Bill"));
}

#[test]
fn usings_of_every_form_are_read_and_a_global_one_is_kept_apart() {
    let src = "global using Shop.Basics;\nusing System;\nusing static Shop.Checks.Guard;\nusing Pay = global::Shop.Payments.IPaymentGateway;\nnamespace Shop.Orders { using Shop.Billing; class A {} }\n";
    let tree = crate::code::lang::Lang::CSharp.parse(src.as_bytes()).unwrap();
    let mut ex = Extraction::default();
    let d = super::declarations::scan(tree.root_node(), "Shop/A.cs", src.as_bytes(), &super::Host::default(), &mut ex);
    use super::declarations::Using;
    assert_eq!(d.global_usings, vec![Using::Namespace("Shop.Basics".into())]);
    assert_eq!(d.usings, vec![
        Using::Namespace("System".into()),
        Using::Static("Shop.Checks.Guard".into()),
        Using::Alias("Pay".into(), "Shop.Payments.IPaymentGateway".into()),
        Using::Namespace("Shop.Billing".into()),
    ]);
    assert_eq!(d.types[0].full(), "Shop.Orders.A");
}

#[test]
fn a_generated_file_is_read_like_any_other() {
    // L9: a tracked generated client is code the rest of the repository calls, so no marker skips it.
    let src = "// <auto-generated>\n//     Generated by a client generator.\n// </auto-generated>\n#nullable enable\nnamespace Shop.Remote.Stubs;\n\n[System.CodeDom.Compiler.GeneratedCode(\"generator\", \"1.0\")]\npublic partial class OrdersClient\n{\n    public System.Threading.Tasks.Task<Order> GetAsync(int id) => throw null!;\n}\n";
    let repo = Repo::new(&[("Remote/OrdersClient.g.cs", src)]);
    let ex = repo.extract("Remote/OrdersClient.g.cs");
    for id in ["sym:Remote/OrdersClient.g.cs::OrdersClient", "sym:Remote/OrdersClient.g.cs::OrdersClient.GetAsync"] {
        assert!(ids(&ex).contains(&id), "{id} missing from {:?}", ids(&ex));
    }
}

#[test]
fn declarations_under_a_preprocessor_branch_are_declared() {
    let ex = one("Shop/A.cs", "namespace Shop;\n#if NET8_0\npublic class B {}\n#endif\npublic class A\n{\n#if DEBUG\n    public void Trace() {}\n#else\n    public void Quiet() {}\n#endif\n    public void Run() {}\n}\n");
    let ids = ids(&ex);
    for id in ["sym:Shop/A.cs::B", "sym:Shop/A.cs::A.Trace", "sym:Shop/A.cs::A.Quiet", "sym:Shop/A.cs::A.Run"] {
        assert!(ids.contains(&id), "{id} missing: {ids:?}");
    }
}

#[test]
fn a_delegate_parameter_is_not_a_member() {
    let ex = one("Shop/H.cs", "namespace Shop;\npublic delegate void PlacedHandler(int x);\n");
    let ids = ids(&ex);
    assert!(ids.contains(&"sym:Shop/H.cs::PlacedHandler"));
    assert!(!ids.iter().any(|i| i.starts_with("sym:Shop/H.cs::PlacedHandler.")), "{ids:?}");
}

use super::declarations::{self, Declared};
use super::index::DotNet;
use super::resolve::Scope;
use super::Host;

fn own(rel: &str, src: &str) -> Declared {
    let tree = crate::code::lang::Lang::CSharp.parse(src.as_bytes()).unwrap();
    declarations::scan(tree.root_node(), rel, src.as_bytes(), &Host::default(), &mut Extraction::default())
}

/// The ids `name` resolves to from `rel`, at `namespace` inside `class`.
fn resolves(repo: &Repo, rel: &str, name: &str, namespace: &str, class: Option<&str>) -> Vec<String> {
    let resolver = repo.resolver();
    let src = std::fs::read_to_string(repo.dir.path().join(rel)).unwrap();
    let own = own(rel, &src);
    let scope = Scope::new(rel, resolver.dotnet(), &own, &Host::default());
    scope.types(name, namespace, class).into_iter().map(|p| format!("sym:{}::{}", p.rel, p.local)).collect()
}

#[test]
fn a_type_in_an_enclosing_namespace_resolves_without_a_using() {
    let repo = Repo::new(&[
        ("Shop/Bedrock/Receipt.cs", "namespace Shop;\npublic class Receipt {}\n"),
        ("Shop/Orders/Use.cs", "namespace Shop.Orders;\nclass Use {}\n"),
    ]);
    assert_eq!(resolves(&repo, "Shop/Orders/Use.cs", "Receipt", "Shop.Orders", Some("Use")), vec!["sym:Shop/Bedrock/Receipt.cs::Receipt"]);
    assert_eq!(resolves(&repo, "Shop/Orders/Use.cs", "Shop.Receipt", "Shop.Orders", Some("Use")), vec!["sym:Shop/Bedrock/Receipt.cs::Receipt"]);
    assert!(resolves(&repo, "Shop/Orders/Use.cs", "Missing", "Shop.Orders", Some("Use")).is_empty());
}

#[test]
fn the_nearest_declaration_wins() {
    let repo = Repo::new(&[
        ("Shop/Receipt.cs", "namespace Shop;\npublic class Receipt {}\n"),
        ("Shop/Orders/Receipt.cs", "namespace Shop.Orders;\npublic class Receipt {}\n"),
        ("Shop/Orders/Use.cs", "namespace Shop.Orders;\nclass Use { class Receipt {} }\n"),
    ]);
    assert_eq!(resolves(&repo, "Shop/Orders/Use.cs", "Receipt", "Shop.Orders", Some("Use")), vec!["sym:Shop/Orders/Use.cs::Use.Receipt"], "a nested type shadows the namespace's");
    assert_eq!(resolves(&repo, "Shop/Orders/Use.cs", "Receipt", "Shop.Orders", None), vec!["sym:Shop/Orders/Receipt.cs::Receipt"], "the inner namespace shadows the outer");
}

#[test]
fn usings_aliases_and_static_usings_resolve() {
    let repo = Repo::new(&[
        ("Payments/IPaymentGateway.cs", "namespace Shop.Payments;\npublic interface IPaymentGateway {}\n"),
        ("Checks/Guard.cs", "namespace Shop.Checks;\npublic static class Guard { public class Rule {} }\n"),
        ("Orders/Use.cs", "using Shop.Payments;\nusing static Shop.Checks.Guard;\nusing Gw = Shop.Payments.IPaymentGateway;\nnamespace Shop.Orders;\nclass Use {}\n"),
    ]);
    let at = |name: &str| resolves(&repo, "Orders/Use.cs", name, "Shop.Orders", Some("Use"));
    assert_eq!(at("IPaymentGateway"), vec!["sym:Payments/IPaymentGateway.cs::IPaymentGateway"]);
    assert_eq!(at("Gw"), vec!["sym:Payments/IPaymentGateway.cs::IPaymentGateway"]);
    assert_eq!(at("Rule"), vec!["sym:Checks/Guard.cs::Guard.Rule"]);
}

#[test]
fn a_global_using_reaches_every_file_of_its_project_and_no_other() {
    let repo = Repo::new(&[
        ("Remote/Remote.csproj", "<Project Sdk=\"Microsoft.NET.Sdk\"></Project>\n"),
        ("Remote/Everywhere.cs", "global using Shop.Payments;\n"),
        ("Remote/Orders/Use.cs", "namespace Remote.Orders;\nclass Use {}\n"),
        ("Web/Web.csproj", "<Project Sdk=\"Microsoft.NET.Sdk\"></Project>\n"),
        ("Web/Page.cs", "namespace Web;\nclass Page {}\n"),
        ("Lib/IPaymentGateway.cs", "namespace Shop.Payments;\npublic interface IPaymentGateway {}\n"),
    ]);
    assert_eq!(resolves(&repo, "Remote/Orders/Use.cs", "IPaymentGateway", "Remote.Orders", Some("Use")), vec!["sym:Lib/IPaymentGateway.cs::IPaymentGateway"]);
    assert!(resolves(&repo, "Web/Page.cs", "IPaymentGateway", "Web", Some("Page")).is_empty(), "code names namespaces, and the other project never imported this one");
}

#[test]
fn a_partial_type_resolves_to_every_part() {
    let repo = Repo::new(&[
        ("Shop/OrderService.Bedrock.cs", "namespace Shop;\npublic partial class OrderService {}\n"),
        ("Shop/OrderService.Billing.cs", "namespace Shop;\npartial class OrderService {}\n"),
        ("Shop/Use.cs", "namespace Shop;\nclass Use {}\n"),
    ]);
    // Parts come back sorted by path.
    assert_eq!(resolves(&repo, "Shop/Use.cs", "OrderService", "Shop", Some("Use")), vec![
        "sym:Shop/OrderService.Bedrock.cs::OrderService",
        "sym:Shop/OrderService.Billing.cs::OrderService",
    ]);
}

#[test]
fn the_header_holds_each_declaring_namespace_and_the_top_level_names() {
    use crate::code::index::header_for;
    use crate::code::lang::Lang;
    let src = "global using static Shop.Checks.Guard;\nusing Shop.Payments;\nnamespace Shop.Bedrock\n{\n    namespace Inner\n    {\n        class Repo { class Nested {} }\n    }\n    interface IRepo {}\n}\nclass Global {}\n";
    let h = header_for(Lang::CSharp, "Shop/Repo.cs", src).unwrap();
    assert_eq!(h.scope, vec!["Shop.Bedrock.Inner".to_string(), "Shop.Bedrock".to_string()]);
    assert_eq!(h.directives.iter().map(String::as_str).collect::<Vec<_>>(), vec!["global using static Shop.Checks.Guard", "using Shop.Payments"]);
    assert_eq!(h.top.iter().map(String::as_str).collect::<Vec<_>>(), vec!["Global", "IRepo", "Repo"]);
}

#[test]
fn a_project_root_namespace_comes_from_the_project_or_its_file_name() {
    let mut d = DotNet::default();
    d.add_project("web/Shop.Web.csproj", "<Project><PropertyGroup><RootNamespace>Shop.Storefront</RootNamespace></PropertyGroup></Project>");
    d.add_project("api/Shop-Remote.csproj", "<Project Sdk=\"Microsoft.NET.Sdk\"></Project>");
    assert_eq!(d.root_namespace("web/Pages/Checkout.razor"), Some(("web", "Shop.Storefront")));
    assert_eq!(d.root_namespace("api/Orders/Use.cs"), Some(("api", "Shop_Remote")), "the project file's name, with what an identifier cannot hold replaced");
    assert_eq!(d.root_namespace("tools/x.cs"), None);
}

// Visual Studio saves C# with a BOM, and a Windows checkout ends its lines in CRLF.
#[test]
fn bom_and_crlf_keep_rows_and_the_header() {
    use crate::code::index::header_for;
    use crate::code::lang::Lang;
    let h = header_for(Lang::CSharp, "Shop/A.cs", "\u{FEFF}using Shop.Payments;\r\nnamespace Shop.Orders;\r\npublic class A { }\r\n").unwrap();
    assert_eq!(h.directives.iter().map(String::as_str).collect::<Vec<_>>(), vec!["using Shop.Payments"]);
    assert_eq!(h.scope, vec!["Shop.Orders".to_string()]);
    assert_eq!(h.top.iter().map(String::as_str).collect::<Vec<_>>(), vec!["A"]);
    let ex = one("Shop/B.cs", "\u{FEFF}namespace Shop;\r\n\r\npublic class B\r\n{\r\n    public void Run() {}\r\n}\r\n");
    assert_eq!(ex.nodes.iter().find(|n| n.id == "sym:Shop/B.cs::B.Run").map(|n| n.line), Some(5));
}

const GATEWAY: &str = "namespace Shop.Payments;\npublic interface IPaymentGateway\n{\n    void Charge(int amount);\n    void Refund(int amount);\n    System.Threading.Tasks.Task ChargeAsync<T>(T amount);\n}\n";

#[test]
fn a_type_the_file_names_is_an_import_of_its_declaring_file() {
    let repo = Repo::new(&[
        ("Payments/IPaymentGateway.cs", GATEWAY),
        ("Orders/Receipt.cs", "namespace Shop.Orders;\npublic class Receipt {}\n"),
        ("Orders/Order.cs", "namespace Shop.Orders;\npublic record Order(int Id);\n"),
        ("Orders/Use.cs", "using Shop.Payments;\nnamespace Shop.Orders;\nclass Use\n{\n    private IPaymentGateway _gateway;\n    System.Collections.Generic.List<Order> Pending() => null;\n    object Cast(object o) => (Receipt)o;\n}\n"),
    ]);
    let ex = repo.extract("Orders/Use.cs");
    let imports = edges(&ex, EdgeKind::Imports);
    for (to, name) in [("file:Payments/IPaymentGateway.cs", "IPaymentGateway"), ("file:Orders/Order.cs", "Order"), ("file:Orders/Receipt.cs", "Receipt")] {
        assert!(imports.contains(&("file:Orders/Use.cs", to, name)), "{to} [{name}] missing from {imports:?}");
    }
    assert_eq!(imports.len(), 3, "a System type is not in the repository and imports nothing: {imports:?}");
}

#[test]
fn a_base_class_and_an_interface_in_other_files_are_extended() {
    let repo = Repo::new(&[
        ("Bedrock/ServiceBase.cs", "namespace Shop.Bedrock;\npublic abstract class ServiceBase {}\n"),
        ("Orders/IOrderService.cs", "namespace Shop.Orders;\npublic interface IOrderService {}\n"),
        ("Orders/OrderService.cs", "using Shop.Bedrock;\nnamespace Shop.Orders;\npublic class OrderService : ServiceBase, IOrderService {}\n"),
    ]);
    let ex = repo.extract("Orders/OrderService.cs");
    let extends = edges(&ex, EdgeKind::Extends);
    assert!(extends.contains(&("sym:Orders/OrderService.cs::OrderService", "sym:Bedrock/ServiceBase.cs::ServiceBase", "")), "{extends:?}");
    assert!(extends.contains(&("sym:Orders/OrderService.cs::OrderService", "sym:Orders/IOrderService.cs::IOrderService", "")), "{extends:?}");
}

#[test]
fn the_parts_of_a_partial_type_import_each_other() {
    let repo = Repo::new(&[
        ("Shop/OrderService.Bedrock.cs", "namespace Shop;\npublic partial class OrderService {}\n"),
        ("Shop/OrderService.Billing.cs", "namespace Shop;\npartial class OrderService {}\n"),
    ]);
    let ex = repo.extract("Shop/OrderService.Billing.cs");
    let imports = edges(&ex, EdgeKind::Imports);
    assert!(imports.contains(&("file:Shop/OrderService.Billing.cs", "file:Shop/OrderService.Bedrock.cs", "OrderService")), "{imports:?}");
}

#[test]
fn an_attribute_points_at_its_declaring_class_and_never_at_a_shared_node() {
    let repo = Repo::new(&[
        ("Bedrock/TrackedAttribute.cs", "namespace Shop.Bedrock;\npublic sealed class TrackedAttribute : System.Attribute {}\n"),
        ("Bedrock/Retry.cs", "namespace Shop.Bedrock;\npublic sealed class Retry : System.Attribute {}\n"),
        ("Orders/OrderService.cs", "using Shop.Bedrock;\nnamespace Shop.Orders;\n[Tracked, Serializable]\npublic class OrderService\n{\n    [Retry(3)]\n    public void Place() {}\n}\n"),
    ]);
    let ex = repo.extract("Orders/OrderService.cs");
    assert_eq!(edges(&ex, EdgeKind::DecoratedBy), vec![
        ("sym:Orders/OrderService.cs::OrderService", "sym:Bedrock/TrackedAttribute.cs::TrackedAttribute", ""),
        ("sym:Orders/OrderService.cs::OrderService.Place", "sym:Bedrock/Retry.cs::Retry", ""),
    ]);
    assert!(!ids(&ex).iter().any(|i| i.starts_with("deco:") || i.starts_with("anno:")), "{:?}", ids(&ex));
}

#[test]
fn an_id_cited_in_a_comment_or_a_string_is_a_reference_from_where_it_sits() {
    let ex = one("Shop/Jcs.cs", "namespace Shop;\n// FR-VIS-47: canonical form before signing\npublic class Jcs\n{\n    public string Name() => \"ADR-022\";\n}\n");
    let refs = edges(&ex, EdgeKind::References);
    assert!(refs.contains(&("file:Shop/Jcs.cs", "FR-VIS-47", "comment")), "{refs:?}");
    assert!(refs.contains(&("sym:Shop/Jcs.cs::Jcs.Name", "ADR-022", "string")), "{refs:?}");
}

#[test]
fn a_type_parameter_is_never_resolved_against_a_same_named_declaration() {
    let repo = Repo::new(&[
        ("Bedrock/TEntity.cs", "namespace Shop;\npublic class TEntity {}\n"),
        ("Bedrock/U.cs", "namespace Shop;\npublic class U {}\n"),
        ("Shop/Repo.cs", "namespace Shop;\npublic class Repo<TEntity>\n{\n    public TEntity Get() => default;\n    public U Map<U>(U x) => x;\n}\n"),
    ]);
    let ex = repo.extract("Shop/Repo.cs");
    let imports = edges(&ex, EdgeKind::Imports);
    assert!(imports.is_empty(), "a type parameter is not a use of the same-named declaration: {imports:?}");
}

#[test]
fn an_assembly_attribute_points_at_its_declaring_class() {
    let repo = Repo::new(&[
        ("Bedrock/TrackedAttribute.cs", "public sealed class TrackedAttribute : System.Attribute {}\n"),
        ("Shop/AssemblyInfo.cs", "[assembly: Tracked]\n"),
    ]);
    let ex = repo.extract("Shop/AssemblyInfo.cs");
    assert_eq!(edges(&ex, EdgeKind::DecoratedBy), vec![
        ("file:Shop/AssemblyInfo.cs", "sym:Bedrock/TrackedAttribute.cs::TrackedAttribute", ""),
    ]);
}

#[test]
fn a_string_literal_inside_an_interpolation_hole_is_referenced_once() {
    // `CodeExtractor::extract` sorts and dedups edges afterwards, which would hide a duplicate
    // push here; scanning directly is the only way this case pins the write itself.
    use super::refs;
    let rel = "Shop/Jcs.cs";
    let src = "namespace Shop;\npublic class Jcs\n{\n    public string Name() => $\"{Foo(\"ADR-022\")}\";\n}\n";
    let tree = crate::code::lang::Lang::CSharp.parse(src.as_bytes()).unwrap();
    let mut ex = Extraction::default();
    let own = declarations::scan(tree.root_node(), rel, src.as_bytes(), &Host::default(), &mut ex);
    let resolver = Repo::new(&[(rel, src)]).resolver();
    refs::scan(tree.root_node(), rel, src.as_bytes(), &Host::default(), &own, &resolver, &mut ex);
    let refs = edges(&ex, EdgeKind::References);
    assert_eq!(refs.iter().filter(|(_, id, _)| *id == "ADR-022").count(), 1, "{refs:?}");
}


