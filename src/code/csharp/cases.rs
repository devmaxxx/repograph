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
