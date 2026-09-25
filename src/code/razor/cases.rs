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
