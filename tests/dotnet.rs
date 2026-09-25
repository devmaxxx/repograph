//! The binary over a .NET repository: what `update` re-reads when a declaration appears, what a
//! body-only edit does not re-read, and a partial type read as one type across its files.

mod common;

use std::path::Path;

fn repograph(repo: &Path, args: &[&str]) -> (bool, String, String) {
    let out = common::run(repo, args);
    (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

fn write(repo: &Path, rel: &str, text: &str) {
    let p = repo.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// .NET is read behind an explicit glob until L7 puts it in the defaults.
const TOML: &str = "code_globs = [\"**/*.cs\", \"**/*.razor\", \"**/*.cshtml\"]\n";

fn ok(repo: &Path, args: &[&str]) -> String {
    let (ok, out, err) = repograph(repo, args);
    assert!(ok, "{args:?} failed: {err}");
    out
}

#[test]
fn a_type_added_in_an_enclosing_namespace_reaches_the_unchanged_file_that_uses_it() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    write(repo, "repograph.toml", TOML);
    write(repo, "Orders/Checkout.cs", "namespace Shop.Orders;\npublic class Checkout\n{\n    private readonly Fresh _fresh;\n    public void Pay() { _fresh.Go(); }\n}\n");
    write(repo, "Orders/Old.cs", "namespace Shop.Orders;\npublic class Old {}\n");
    ok(repo, &["build"]);
    write(repo, "Bedrock/Fresh.cs", "namespace Shop;\npublic class Fresh\n{\n    public void Go() {}\n}\n");
    ok(repo, &["update"]);
    let out = ok(repo, &["impact", "Fresh.Go"]);
    assert!(out.contains("Orders/Checkout.cs"), "Checkout.Pay calls Fresh.Go once Fresh.cs declares it: {out}");
}

#[test]
fn a_member_renamed_without_touching_the_header_re_reads_that_file_alone() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    write(repo, "repograph.toml", TOML);
    write(repo, "Orders/Checkout.cs", "namespace Shop.Orders;\npublic class Checkout\n{\n    private readonly Shop.Payments.Gateway _gateway;\n    public void Pay() { _gateway.Charge(); }\n}\n");
    write(repo, "Payments/Gateway.cs", "namespace Shop.Payments;\npublic class Gateway\n{\n    public void Charge() {}\n}\n");
    ok(repo, &["build"]);
    assert!(ok(repo, &["explain", "Checkout.Pay"]).contains("sym:Payments/Gateway.cs::Gateway.Charge"));
    write(repo, "Payments/Gateway.cs", "namespace Shop.Payments;\npublic class Gateway\n{\n    public void Settle() {}\n}\n");
    let out = ok(repo, &["update"]);
    assert!(out.starts_with("changed 1 removed 0"), "{out}");
    // L3's price, pinned so a change to it is a decision: the header did not move, so the caller was
    // not re-read and still names the member it named when it was last read.
    assert!(ok(repo, &["explain", "Checkout.Pay"]).contains("sym:Payments/Gateway.cs::Gateway.Charge"));
}

#[test]
fn a_partial_type_is_one_type_across_its_files() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    write(repo, "repograph.toml", TOML);
    write(repo, "Shop/OrderService.Bedrock.cs", "namespace Shop;\npublic partial class OrderService\n{\n    public void Place() {}\n}\n");
    write(repo, "Shop/OrderService.Billing.cs", "namespace Shop;\npartial class OrderService\n{\n    public void Bill() { Place(); }\n}\n");
    write(repo, "Shop/Checkout.cs", "namespace Shop;\npublic class Checkout\n{\n    private readonly OrderService _orders;\n    public void Pay() { _orders.Bill(); }\n}\n");
    ok(repo, &["build"]);
    let impact = ok(repo, &["impact", "OrderService.Place"]);
    assert!(impact.contains("Shop/OrderService.Billing.cs") && impact.contains("Shop/Checkout.cs"), "{impact}");
    let trace = ok(repo, &["trace", "Checkout.Pay", "OrderService.Place"]);
    assert!(trace.contains("OrderService.Bill"), "{trace}");
    let importers = ok(repo, &["impact", "OrderService"]);
    assert!(importers.contains("Shop/OrderService.Bedrock.cs") && importers.contains("Shop/Checkout.cs"), "{importers}");
}
