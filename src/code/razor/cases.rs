//! Razor on inline sources: the blanked copy's shape, then (Task 8) what a component writes.

use super::blank::{blocks, view, View};
use crate::code::lang::Lang;

const CHECKOUT: &str = r#"@page "/checkout"
@using Shop.Payments
@inject IPaymentGateway Payments

<h3>Checkout</h3>
<OrderLine Caption="@item" />
@if (open) { <p>{ not code</p> }

@code {
    private bool open;
    [Parameter] public string Heading { get; set; } = "a { brace";
    private char c = '{';
    // a } in a comment
    private string v = @"verbatim ""}"" ";
    private string i = $"{Heading} and {(open ? "}" : "{")}";
    void Pay()
    {
        @if (open)
        {
            Payments.Charge(1);
        }
        var r = """raw { """;
    }
}
"#;

fn read(src: &str) -> String {
    match view(src) {
        View::Read(text) => text,
        other => panic!("not read: {other:?}"),
    }
}

fn clean(text: &str) -> bool {
    !Lang::CSharp.parse(text.as_bytes()).unwrap().root_node().has_error()
}

#[test]
fn a_code_block_reads_as_csharp_with_every_row_and_column_kept() {
    let text = read(CHECKOUT);
    assert_eq!(text.len(), CHECKOUT.len());
    let newlines = |s: &str| s.match_indices('\n').map(|(i, _)| i).collect::<Vec<_>>();
    assert_eq!(newlines(&text), newlines(CHECKOUT));
    assert!(clean(&text), "{text}");
    let at = CHECKOUT.find("void Pay()").unwrap();
    assert_eq!(&text[at..at + 10], "void Pay()");
    assert!(!text.contains("<h3>") && !text.contains("@page"), "markup outside the block is blank: {text}");
    assert_eq!(blocks(CHECKOUT).unwrap().len(), 1, "a brace in a string, a char, a comment or markup text does not close the block");
}

#[test]
fn functions_holds_the_wrapper_in_its_own_bytes() {
    let text = read("<div>\n@functions {\n    int x;\n}\n");
    assert!(text.contains("class C    {"), "{text:?}");
    assert!(clean(&text));
}

#[test]
fn a_code_directive_puts_class_on_the_line_above() {
    let text = read("@page \"/a\"\n@code {\n    int x;\n}\n");
    assert!(text.starts_with("class"), "{text:?}");
    assert!(text.contains("\nC     {"), "{text:?}");
    assert!(clean(&text));
}

#[test]
fn a_code_block_on_the_first_line_puts_the_wrapper_in_its_indentation() {
    let text = read("@code{\n    int x;\n}\n");
    assert!(text.starts_with("class \n"), "{text:?}");
    assert!(text.contains("C{  int x;"), "{text:?}");
    assert!(clean(&text));
}

#[test]
fn two_blocks_are_one_class() {
    let text = read("<div>\n@functions {\n  int x;\n}\n<p>tail</p>\n@code {\n  void M() { }\n}\n");
    let tree = Lang::CSharp.parse(text.as_bytes()).unwrap();
    let root = tree.root_node();
    assert!(!root.has_error(), "{text:?}");
    let mut c = root.walk();
    assert_eq!(root.named_children(&mut c).filter(|n| n.kind() == "class_declaration").count(), 1);
}

#[test]
fn a_file_with_no_block_or_an_unclosed_one_is_not_read() {
    assert_eq!(view("<p>see @code docs</p>\n<Button />\n"), View::None);
    assert!(matches!(view("@code {\n    int x;\n"), View::Unread(_)));
}

// Visual Studio saves a `.razor` with a UTF-8 BOM by default, so line 1 starts at byte 3.
#[test]
fn a_bom_before_the_first_line_is_not_part_of_it() {
    let src = "\u{FEFF}@code {\n    int x;\n}\n";
    let View::Read(t) = view(src) else { panic!("a BOM'd first-line block is not read: {:?}", view(src)) };
    assert!(clean(&t), "{t:?}");
    assert_eq!(t.len(), src.len());
    assert_eq!(blocks(src).unwrap(), vec![(3, 9, 22)]);
}

#[test]
fn a_crlf_file_keeps_every_row_and_the_wrapper() {
    let src = "@page \"/a\"\r\n@code {\r\n    int x;\r\n}\r\n";
    let View::Read(t) = view(src) else { panic!("unread") };
    assert!(clean(&t), "{t:?}");
    assert_eq!(t.len(), src.len());
    let newlines = |s: &str| s.match_indices('\n').map(|(i, _)| i).collect::<Vec<_>>();
    assert_eq!(newlines(&t), newlines(src));
}

#[test]
fn a_verbatim_string_with_a_doubled_quote_is_not_read_as_a_raw_string() {
    // `@"""a}"` is a verbatim string holding `"a}` (the doubled quote is the escape); the
    // triple-quote raw-string branch must not fire on a verbatim opener.
    let src = "@code {\n  string a = @\"\"\"a}\";\n  int y;\n}\n";
    let text = read(src);
    assert!(text.contains("int y"), "{text:?}");
    assert!(clean(&text), "{text:?}");
}

#[test]
fn a_blank_crlf_line_before_the_first_line_s_block_does_not_stop_the_search() {
    let src = "@code{\r\n\r\n    int x;\r\n}\r\n";
    let text = read(src);
    assert!(text.contains("C{"), "{text:?}");
    assert!(clean(&text), "{text:?}");
}

#[test]
fn a_razor_comment_inside_a_block_does_not_close_it_early() {
    let src = "@code {\n  @* } *@\n  int y;\n}\n";
    let text = read(src);
    assert!(text.contains("int y"), "{text:?}");
    assert!(clean(&text), "{text:?}");
}

#[test]
fn a_commented_out_code_block_is_not_a_directive() {
    let src = "@*\n@code { void Ghost() {} }\n*@\n";
    assert_eq!(view(src), View::None);
}

#[test]
fn a_commented_out_code_block_beside_a_real_one_writes_no_ghost_member() {
    let src = "@*\n@code { void Ghost() {} }\n*@\n@code {\n  void Real() { }\n}\n";
    let text = read(src);
    assert!(clean(&text), "{text:?}");
    assert!(!text.contains("Ghost"), "{text:?}");
    assert!(text.contains("Real"), "{text:?}");
}

#[test]
fn an_at_star_in_a_string_is_not_a_razor_comment_opener() {
    // Both `"@*"` and `"*@"` are complete regular strings; a naive byte search pairing an `@*`
    // found inside one with a `*@` found inside the other would blank everything between them.
    let src = "@code {\n  string a = \"@*\";\n  int y;\n  string b = \"*@\";\n}\n";
    let text = read(src);
    assert!(text.contains("int y"), "{text:?}");
    assert!(clean(&text), "{text:?}");
}

#[test]
fn an_at_star_inside_a_line_comment_is_not_a_razor_comment_opener() {
    // The `@*` is C# comment text, not a Razor comment; a global search for the closing `*@`
    // must not run off into the markup after the block and blank real code on the way.
    let src = "@code {\n  // @*\n  int y;\n}\n@* x *@\n";
    let text = read(src);
    assert!(text.contains("int y"), "{text:?}");
    assert!(clean(&text), "{text:?}");
}

#[test]
fn an_at_star_inside_a_block_comment_is_not_a_razor_comment_opener() {
    let src = "@code {\n  /* @* */ int y; /* *@ */\n}\n";
    let text = read(src);
    assert!(text.contains("int y"), "{text:?}");
    assert!(clean(&text), "{text:?}");
}

#[test]
fn a_quoted_at_star_does_not_pair_with_a_comment_after_the_block() {
    // The bug this pins: an `@*` found inside the string paired with the `*@` in the markup
    // comment below, blanking from inside the string to the block's own close and leaving an
    // unterminated string literal in the copy.
    let src = "@code {\n  string a = \"@*\";\n  int y;\n}\n<p>@* note *@</p>\n";
    let text = read(src);
    assert!(text.contains("int y"), "{text:?}");
    assert!(clean(&text), "{text:?}");
}

#[test]
fn a_comment_opening_mid_line_in_markup_still_hides_the_block_it_holds() {
    let src = "<p>@* note\n@code {\n  void Ghost() {}\n}\n*@</p>\n";
    assert_eq!(view(src), View::None);
}

#[test]
fn a_directive_right_after_a_same_line_comment_is_still_read() {
    let src = "@* x *@ @code {\n  void M() { }\n}\n";
    let text = read(src);
    assert!(text.contains("void M"), "{text:?}");
    assert!(clean(&text), "{text:?}");
}

#[test]
fn an_escaped_at_star_is_not_a_comment_opener() {
    // `@@` is Razor's own escape for a literal `@`; the `*` after it never opens a comment.
    let src = "<p>@@* not a comment</p>\n@code {\n  int y;\n}\n";
    let text = read(src);
    assert!(text.contains("int y"), "{text:?}");
    assert!(clean(&text), "{text:?}");
}

#[test]
fn an_at_star_after_an_identifier_byte_is_not_a_comment_opener() {
    // `a@*b` is email-style text, not a transition into a comment: the byte before `@` is part
    // of an identifier, not markup or whitespace.
    let src = "<p>a@*b</p>\n@code {\n  int y;\n}\n";
    let text = read(src);
    assert!(text.contains("int y"), "{text:?}");
    assert!(clean(&text), "{text:?}");
}

#[test]
fn an_at_star_after_a_slash_that_never_closes_is_unread_not_silently_none() {
    // `packages/@*/x` opens what the scanner reads as a comment (nothing rules it out); with no
    // `*@` anywhere in the file it must surface as `Unread`, not vanish as `None`.
    let src = "<p>packages/@*/x</p>\n@code {\n  int y;\n}\n";
    assert!(matches!(view(src), View::Unread(_)), "{:?}", view(src));
}

#[test]
fn an_at_star_in_css_that_never_closes_is_unread_not_silently_none() {
    let src = "<style>\n  @* { margin: 0 }\n</style>\n@code {\n  int y;\n}\n";
    assert!(matches!(view(src), View::Unread(_)), "{:?}", view(src));
}

#[test]
fn a_failed_string_attempt_rolls_back_any_comment_spans_it_recorded() {
    // The hole's own close_of call crosses a real `@* x *@` and finds a `}` to return before the
    // outer interpolated string as a whole gives up (no closing quote follows): without a
    // rollback, the span that call recorded stays in `razor_comments` from an attempt that never
    // produced a string at all.
    let src = "@code {\n  @<p>$\"{ \"@*\"</p>;\n  int y;\n  string t = \"*@ }\";\n}\n";
    let text = read(src);
    assert!(text.contains("int y"), "{text:?}");
}

#[test]
fn a_bare_inch_mark_in_markup_does_not_swallow_the_rest_of_its_line() {
    let nested = "@code {\n  void M() { if (x) { <p>12\" pizza</p> } }\n  int y;\n}\n";
    assert!(read(nested).contains("int y"), "{:?}", view(nested));
    let opener = "@code {\n  void M() { <p>5\" wide</p> if (x) {\n  int y;\n  } }\n  int z;\n}\n";
    let text = read(opener);
    assert!(text.contains("int y") && text.contains("int z"), "{text:?}");
}

#[test]
fn a_comment_right_after_a_block_s_close_on_the_same_line_hides_its_ghost() {
    let src = "@code {\n  int x;\n} @* \n@code { void Ghost() {} }\n*@\n";
    let text = read(src);
    assert!(clean(&text), "{text:?}");
    assert!(text.contains("int x"), "{text:?}");
    assert!(!text.contains("Ghost"), "{text:?}");
}

/// Not a test: reads every `.razor` and `.cshtml` under `REPOGRAPH_CENSUS_ROOTS` (`:`-separated)
/// through `view` and counts what the C# grammar makes of each copy — the spec's Razor parse clause.
#[test]
#[ignore]
fn razor_block_census() {
    let roots = std::env::var("REPOGRAPH_CENSUS_ROOTS").expect("set REPOGRAPH_CENSUS_ROOTS");
    let (mut files, mut with, mut count, mut unread, mut unclean) = (0, 0, 0, 0, 0);
    for root in roots.split(':') {
        // Dotted directories are walked and `.git` is not, as `walk::walk` does since 0.5.3.
        for dent in ignore::WalkBuilder::new(root).hidden(false).filter_entry(|e| e.file_name() != ".git").git_ignore(true).build().flatten() {
            let p = dent.path();
            if !matches!(p.extension().and_then(|e| e.to_str()), Some("razor" | "cshtml")) {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(p) else { continue };
            files += 1;
            match view(&src) {
                View::None => {}
                View::Unread(_) => {
                    with += 1;
                    unread += 1;
                }
                View::Read(text) => {
                    with += 1;
                    count += blocks(&src).map_or(0, |b| b.len());
                    unclean += usize::from(!clean(&text));
                }
            }
        }
    }
    println!("razor-census: files {files} with-blocks {with} blocks {count} unread {unread} unclean {unclean}");
}

use crate::code::csharp::cases::{edges, ids, Repo};
use crate::model::EdgeKind;

const WEB: &[(&str, &str)] = &[
    ("Web/Shop.Web.csproj", "<Project Sdk=\"Microsoft.NET.Sdk.Web\"><PropertyGroup><RootNamespace>Shop.Web</RootNamespace></PropertyGroup></Project>\n"),
    ("Web/_Imports.razor", "@using Shop.Payments\n@using Shop.Web.Shared\n"),
    ("Payments/IPaymentGateway.cs", "namespace Shop.Payments;\npublic interface IPaymentGateway { void Charge(int amount); }\n"),
    ("Web/Shared/OrderLine.razor", "<li>@Caption</li>\n@code {\n    [Parameter] public string Caption { get; set; }\n}\n"),
    ("Web/Shared/PageBase.cs", "namespace Shop.Web.Shared;\npublic abstract class PageBase {}\n"),
    ("Web/Pages/Checkout.razor", "@page \"/checkout\"\n@inherits PageBase\n@inject IPaymentGateway Payments\n\n<h3>Checkout</h3>\n<OrderLine Caption=\"one\" />\n\n@code {\n    private int total;\n    void Pay()\n    {\n        Payments.Charge(total);\n        Confirm();\n    }\n}\n"),
    ("Web/Pages/Checkout.razor.cs", "namespace Shop.Web.Pages;\npublic partial class Checkout\n{\n    void Confirm() {}\n}\n"),
];

#[test]
fn directives_are_read_outside_the_blocks_and_tags_are_upper_case() {
    let d = super::directives("@namespace Shop.Web\n@using static Shop.Checks.Guard\n@using Pay = Shop.Payments.IPaymentGateway\n@inject IStringLocalizer<Checkout> L\n@implements IDisposable\n<div><OrderLine />\n@code {\n    RenderFragment f = @<Badge />;\n}\n");
    use crate::code::csharp::declarations::Using;
    assert_eq!(d.namespace.as_deref(), Some("Shop.Web"));
    assert_eq!(d.usings, vec![Using::Static("Shop.Checks.Guard".into()), Using::Alias("Pay".into(), "Shop.Payments.IPaymentGateway".into())]);
    assert_eq!(d.injects, vec![("IStringLocalizer<Checkout>".to_string(), "L".to_string(), 4)]);
    assert_eq!(d.types, vec![("IDisposable".to_string(), 5, true)]);
    assert_eq!(d.tags, vec![("OrderLine".to_string(), 6, None)], "a tag inside a block is not read");
}

#[test]
fn a_bom_before_a_first_line_directive_is_not_part_of_it() {
    let d = super::directives("\u{FEFF}@namespace Shop.Web\n@using Shop.Payments\n<h3/>\n");
    assert_eq!(d.namespace.as_deref(), Some("Shop.Web"));
    assert_eq!(d.usings.len(), 1);
    let d = super::directives("\u{FEFF}@inject IPaymentGateway Payments\n");
    assert_eq!(d.injects, vec![("IPaymentGateway".to_string(), "Payments".to_string(), 1)]);
}

#[test]
fn a_component_is_a_symbol_in_the_namespace_the_razor_compiler_gives_it() {
    let repo = Repo::new(WEB);
    assert_eq!(repo.resolver().dotnet().razor_namespace("Web/Pages/Checkout.razor", None), "Shop.Web.Pages");
    let ex = repo.extract("Web/Pages/Checkout.razor");
    let f = "sym:Web/Pages/Checkout.razor::";
    for id in ["Checkout", "Checkout.Payments", "Checkout.total", "Checkout.Pay"] {
        let id = format!("{f}{id}");
        assert!(ids(&ex).contains(&id.as_str()), "{id} missing from {:?}", ids(&ex));
    }
    assert!(edges(&ex, EdgeKind::Declares).contains(&("file:Web/Pages/Checkout.razor", "sym:Web/Pages/Checkout.razor::Checkout", "export")));
    let at = |id: &str| {
        let n = ex.nodes.iter().find(|n| n.id == format!("{f}{id}")).unwrap();
        (n.line, n.end)
    };
    assert_eq!(at("Checkout.Pay"), (10, 14), "a member's rows in the blanked copy are its rows in the file");
    assert_eq!(at("Checkout"), (1, 15));
}

#[test]
fn an_inject_a_tag_an_inherits_and_a_code_behind_member_resolve() {
    let repo = Repo::new(WEB);
    let ex = repo.extract("Web/Pages/Checkout.razor");
    let calls = edges(&ex, EdgeKind::Calls);
    for (from, to) in [
        ("sym:Web/Pages/Checkout.razor::Checkout.Pay", "sym:Payments/IPaymentGateway.cs::IPaymentGateway.Charge"),
        ("sym:Web/Pages/Checkout.razor::Checkout.Pay", "sym:Web/Pages/Checkout.razor.cs::Checkout.Confirm"),
        ("sym:Web/Pages/Checkout.razor::Checkout", "sym:Web/Shared/OrderLine.razor::OrderLine"),
    ] {
        assert!(calls.contains(&(from, to, "")), "{from} -> {to} missing from {calls:?}");
    }
    assert!(edges(&ex, EdgeKind::Extends).contains(&("sym:Web/Pages/Checkout.razor::Checkout", "sym:Web/Shared/PageBase.cs::PageBase", "")));
    let imports = edges(&ex, EdgeKind::Imports);
    for (to, name) in [("file:Payments/IPaymentGateway.cs", "IPaymentGateway"), ("file:Web/Shared/OrderLine.razor", "OrderLine"), ("file:Web/Pages/Checkout.razor.cs", "Checkout")] {
        assert!(imports.contains(&("file:Web/Pages/Checkout.razor", to, name)), "{to} [{name}] missing from {imports:?}");
    }
}

#[test]
fn a_code_behind_part_imports_its_component() {
    let repo = Repo::new(WEB);
    let ex = repo.extract("Web/Pages/Checkout.razor.cs");
    let imports = edges(&ex, EdgeKind::Imports);
    assert!(imports.contains(&("file:Web/Pages/Checkout.razor.cs", "file:Web/Pages/Checkout.razor", "Checkout")), "{imports:?}");
}

#[test]
fn a_component_with_no_block_still_resolves_its_inject_and_its_tags() {
    let mut files = WEB.to_vec();
    files.push(("Web/Shared/Badge.razor", "@inject IPaymentGateway Payments\n<OrderLine Caption=\"x\" />\n"));
    let repo = Repo::new(&files);
    let ex = repo.extract("Web/Shared/Badge.razor");
    assert!(ids(&ex).contains(&"sym:Web/Shared/Badge.razor::Badge.Payments"), "{:?}", ids(&ex));
    assert!(edges(&ex, EdgeKind::Calls).contains(&("sym:Web/Shared/Badge.razor::Badge", "sym:Web/Shared/OrderLine.razor::OrderLine", "")));
}

#[test]
fn a_view_and_an_imports_file_declare_no_component() {
    let mut files = WEB.to_vec();
    files.push(("Web/Pages/IndexModel.cs", "namespace Shop.Web.Pages;\npublic class IndexModel {}\n"));
    files.push(("Web/Pages/Index.cshtml", "@page\n@model Shop.Web.Pages.IndexModel\n@using Shop.Payments\n@inject IPaymentGateway Payments\n<h1>Index</h1>\n"));
    let repo = Repo::new(&files);
    let view = repo.extract("Web/Pages/Index.cshtml");
    assert_eq!(ids(&view), vec!["file:Web/Pages/Index.cshtml"]);
    let imports = edges(&view, EdgeKind::Imports);
    assert!(imports.contains(&("file:Web/Pages/Index.cshtml", "file:Web/Pages/IndexModel.cs", "IndexModel")), "{imports:?}");
    assert!(imports.contains(&("file:Web/Pages/Index.cshtml", "file:Payments/IPaymentGateway.cs", "IPaymentGateway")), "{imports:?}");
    assert_eq!(ids(&repo.extract("Web/_Imports.razor")), vec!["file:Web/_Imports.razor"]);
}

#[test]
fn a_component_header_is_its_name_and_its_own_namespace() {
    use crate::code::index::header_for;
    let h = header_for(Lang::Razor, "Web/Pages/Checkout.razor", "@namespace Shop.Checkout\n<h3/>\n").unwrap();
    assert_eq!(h.scope, vec!["Shop.Checkout".to_string()]);
    assert_eq!(h.top.iter().map(String::as_str).collect::<Vec<_>>(), vec!["Checkout"]);
    let imports = header_for(Lang::Razor, "Web/_Imports.razor", "@using Shop\n@namespace Shop.Web\n").unwrap();
    assert!(imports.top.is_empty());
    assert!(imports.scope.is_empty());
    assert_eq!(imports.directives.iter().map(String::as_str).collect::<Vec<_>>(), vec!["@namespace Shop.Web", "@using Shop"], "an imports file's directives move its header");
}

#[test]
fn a_view_resolves_only_what_its_own_directives_bring() {
    let mut files = WEB.to_vec();
    files.push(("Web/Pages/IndexModel.cs", "namespace Shop.Web.Pages;\npublic class IndexModel {}\n"));
    files.push(("Web/Pages/Plain.cshtml", "@model IndexModel\n@inject IPaymentGateway Payments\n<h1>Plain</h1>\n"));
    let repo = Repo::new(&files);
    let ex = repo.extract("Web/Pages/Plain.cshtml");
    let imports = edges(&ex, EdgeKind::Imports);
    assert!(imports.is_empty(), "a view's class is not in the project's namespace, and _Imports.razor is for components: {imports:?}");
}

#[test]
fn a_component_namespace_is_sanitised_as_the_razor_compiler_does() {
    let repo = Repo::new(&[
        ("App/My-App.csproj", "<Project Sdk=\"Microsoft.NET.Sdk.Web\"></Project>\n"),
        ("App/2024/Admin Pages/Report.razor", "<h3/>\n"),
        ("Site/Shop.Site.csproj", "<Project Sdk=\"Microsoft.NET.Sdk.Web\"></Project>\n"),
        ("Site/Area/_Imports.razor", "@namespace Shop.Named\n"),
        ("Site/Area/Deep/Page.razor", "<h3/>\n"),
    ]);
    let r = repo.resolver();
    assert_eq!(r.dotnet().razor_namespace("App/2024/Admin Pages/Report.razor", None), "My_App._2024.Admin_Pages");
    assert_eq!(r.dotnet().razor_namespace("Site/Area/Deep/Page.razor", None), "Shop.Named.Deep", "an imports file's @namespace plus the folders below it");
}

#[test]
fn an_imports_file_above_the_project_does_not_reach_its_components() {
    let repo = Repo::new(&[
        ("_Imports.razor", "@using Shop.Payments\n@namespace Outer\n"),
        ("Web/Shop.Web.csproj", "<Project Sdk=\"Microsoft.NET.Sdk.Web\"><PropertyGroup><RootNamespace>Shop.Web</RootNamespace></PropertyGroup></Project>\n"),
        ("Payments/IPaymentGateway.cs", "namespace Shop.Payments;\npublic interface IPaymentGateway { void Charge(int amount); }\n"),
        ("Web/Pages/Pay.razor", "@inject IPaymentGateway Payments\n"),
    ]);
    assert_eq!(repo.resolver().dotnet().razor_namespace("Web/Pages/Pay.razor", None), "Shop.Web.Pages");
    let ex = repo.extract("Web/Pages/Pay.razor");
    let imports = edges(&ex, EdgeKind::Imports);
    assert!(imports.is_empty(), "{imports:?}");
}

#[test]
fn a_commented_tag_or_directive_is_not_read_and_a_using_statement_is_not_a_directive() {
    let d = super::directives("@* <OrderLine />\n@inject IPaymentGateway Payments *@\n<!-- <Badge /> -->\n@using (Html.BeginForm())\n{\n}\n<Card />\n");
    assert_eq!(d.tags, vec![("Card".to_string(), 7, None)]);
    assert!(d.injects.is_empty() && d.usings.is_empty(), "{d:?}");
}

#[test]
fn a_tag_naming_a_class_that_is_not_a_component_renders_nothing() {
    let mut files = WEB.to_vec();
    files.push(("Web/Pages/Odd.razor", "<PageBase />\n<OrderLine />\n"));
    let repo = Repo::new(&files);
    let ex = repo.extract("Web/Pages/Odd.razor");
    let calls = edges(&ex, EdgeKind::Calls);
    assert_eq!(calls, vec![("sym:Web/Pages/Odd.razor::Odd", "sym:Web/Shared/OrderLine.razor::OrderLine", "")], "Razor renders a component, and a plain class is an HTML element to it");
}

#[test]
fn a_component_whose_blocks_cannot_be_read_writes_its_file_alone_and_is_rendered_by_no_tag() {
    let mut files = WEB.to_vec();
    files.push(("Web/Shared/Broken.razor", "@inject IPaymentGateway Payments\n<OrderLine />\n@code {\n    void Pay() {\n"));
    files.push(("Web/Pages/Host.razor", "<Broken />\n"));
    let repo = Repo::new(&files);
    let broken = repo.extract("Web/Shared/Broken.razor");
    assert_eq!(ids(&broken), vec!["file:Web/Shared/Broken.razor"]);
    assert!(broken.edges.is_empty(), "{:?}", broken.edges);
    let host = repo.extract("Web/Pages/Host.razor");
    assert!(edges(&host, EdgeKind::Calls).is_empty(), "no symbol to render: {:?}", host.edges);
}

fn calls_of(files: &[(&'static str, &'static str)], rel: &str) -> Vec<String> {
    let mut all = WEB.to_vec();
    all.extend_from_slice(files);
    let ex = Repo::new(&all).extract(rel);
    edges(&ex, EdgeKind::Calls).into_iter().map(|(_, to, _)| to.to_string()).collect()
}

#[test]
fn a_child_content_parameter_tag_is_the_parent_s_parameter_not_a_component() {
    let card = ("Web/Shared/Card.razor", "<div>@Header</div>\n@code {\n    [Parameter] public RenderFragment Header { get; set; }\n}\n");
    let panel = ("Web/Shared/Panel.razor", "<div>@Header</div>\n");
    let behind = ("Web/Shared/Panel.razor.cs", "namespace Shop.Web.Shared;\npublic partial class Panel\n{\n    [Parameter] public RenderFragment Header { get; set; }\n}\n");
    let header = ("Web/Shared/Header.razor", "<h1/>\n");
    let page = ("Web/Pages/P1.razor", "<Card>\n  <Header>hi</Header>\n  <OrderLine />\n</Card>\n<Panel><Header>x</Header></Panel>\n");
    assert_eq!(
        calls_of(&[card, panel, behind, header, page], "Web/Pages/P1.razor"),
        vec!["sym:Web/Shared/Card.razor::Card", "sym:Web/Shared/OrderLine.razor::OrderLine", "sym:Web/Shared/Panel.razor.cs::Panel", "sym:Web/Shared/Panel.razor::Panel"],
    );
}

#[test]
fn a_tag_inside_a_component_the_repo_does_not_declare_is_not_read() {
    let page = ("Web/Pages/P5.razor", "<MudCard>\n  <OrderLine />\n</MudCard>\n<OrderLine/>\n");
    assert_eq!(calls_of(&[page], "Web/Pages/P5.razor"), vec!["sym:Web/Shared/OrderLine.razor::OrderLine"]);
    let only = ("Web/Pages/P6.razor", "<MudCard>\n  <OrderLine />\n</MudCard>\n");
    assert!(calls_of(&[only], "Web/Pages/P6.razor").is_empty());
}

#[test]
fn a_generic_argument_in_markup_code_is_not_a_tag() {
    let src = "@(new List<Badge>())\n@{ var l = new List<Badge>(); }\n@if (x.OfType<Badge>().Any()) {}\n";
    assert!(super::directives(src).tags.is_empty(), "{:?}", super::directives(src).tags);
    let badge = ("Web/Shared/Badge.razor", "<span/>\n");
    let page = ("Web/Pages/P2.razor", src);
    assert!(calls_of(&[badge, page], "Web/Pages/P2.razor").is_empty());
}

#[test]
fn a_plain_class_sharing_a_component_s_name_is_not_rendered() {
    let model = ("Models/Notice.cs", "namespace Shop.Models;\npublic class Notice {}\n");
    let notice = ("Web/Shared/Notice.razor", "<p/>\n");
    let page = ("Web/Pages/P3.razor", "@using Shop.Models\n<Notice />\n");
    assert_eq!(calls_of(&[model, notice, page], "Web/Pages/P3.razor"), vec!["sym:Web/Shared/Notice.razor::Notice"]);
}

#[test]
fn a_view_reads_its_view_imports_namespace_and_usings() {
    let mut files = WEB.to_vec();
    files.extend_from_slice(&[
        ("Web/_ViewImports.cshtml", "@using Shop.Payments\n"),
        ("Web/Pages/_ViewImports.cshtml", "@namespace Shop.Web.Pages\n"),
        ("Web/Pages/IndexModel.cs", "namespace Shop.Web.Pages;\npublic class IndexModel {}\n"),
        ("Admin/IndexModel.cs", "namespace Shop.Admin;\npublic class IndexModel {}\n"),
        ("Web/Pages/Index.cshtml", "@using Shop.Admin\n@model IndexModel\n@inject IPaymentGateway Payments\n"),
    ]);
    let repo = Repo::new(&files);
    assert_eq!(repo.resolver().dotnet().razor_namespace("Web/Pages/Index.cshtml", None), "Shop.Web.Pages");
    let ex = repo.extract("Web/Pages/Index.cshtml");
    let imports = edges(&ex, EdgeKind::Imports);
    assert_eq!(imports, vec![
        ("file:Web/Pages/Index.cshtml", "file:Payments/IPaymentGateway.cs", "IPaymentGateway"),
        ("file:Web/Pages/Index.cshtml", "file:Web/Pages/IndexModel.cs", "IndexModel"),
    ], "the enclosing namespace's IndexModel beats the using's");
}

#[test]
fn an_aliased_using_resolves_an_inject_but_never_a_tag() {
    let page = ("Web/Pages/P7.razor", "@using Line = Shop.Web.Shared.OrderLine\n@using Pay = Shop.Payments.IPaymentGateway\n@inject Pay Payments\n<Line />\n");
    let mut files = WEB.to_vec();
    files.push(page);
    let ex = Repo::new(&files).extract("Web/Pages/P7.razor");
    assert!(edges(&ex, EdgeKind::Calls).is_empty(), "{:?}", ex.edges);
    assert!(edges(&ex, EdgeKind::Imports).contains(&("file:Web/Pages/P7.razor", "file:Payments/IPaymentGateway.cs", "IPaymentGateway")));
}

#[test]
fn a_code_behind_calls_a_member_its_component_declares_in_code() {
    let mut files: Vec<(&str, &str)> = WEB.iter().filter(|(p, _)| *p != "Web/Pages/Checkout.razor.cs").copied().collect();
    files.push(("Web/Pages/Checkout.razor.cs", "namespace Shop.Web.Pages;\npublic partial class Checkout\n{\n    void Confirm() { Pay(); }\n}\n"));
    let ex = Repo::new(&files).extract("Web/Pages/Checkout.razor.cs");
    assert!(edges(&ex, EdgeKind::Calls).contains(&("sym:Web/Pages/Checkout.razor.cs::Checkout.Confirm", "sym:Web/Pages/Checkout.razor::Checkout.Pay", "")), "{:?}", ex.edges);
}
