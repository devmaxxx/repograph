//! Bicep's shapes, each on an inline source so the grammar's reading of the construct is pinned by the
//! assertion rather than by a corpus.

use crate::code::prose::testing::{ids, lines, repo};
use crate::model::EdgeKind;

const GLOBS: &[&str] = &["**/*.bicep"];

fn extract(files: &[(&str, &str)], rel: &str) -> crate::model::Extraction {
    let (_dir, resolver) = repo(GLOBS, files);
    let text = files.iter().find(|(f, _)| *f == rel).map(|(_, t)| *t).unwrap();
    super::extract(&resolver, rel, text)
}

const DECLS: &str = "\
// The region every resource lands in.
@description('region')
param location string
var prefix = 'app'
@export()
type sku = 'A' | 'B'
func label(p string) string => '${p}-x'
resource vnet 'Microsoft.Network/virtualNetworks@2023-04-01' existing = {
  name: 'vnet'
  resource subnet 'subnets' existing = {
    name: 'default'
  }
}
output id string = vnet.id
";

#[test]
fn every_declaration_kind_is_a_symbol_and_outputs_and_children_keep_their_own_names() {
    let ex = extract(&[("stack.bicep", DECLS)], "stack.bicep");
    assert_eq!(ids(&ex), [
        "file:stack.bicep", "sym:stack.bicep::label", "sym:stack.bicep::location", "sym:stack.bicep::output/id",
        "sym:stack.bicep::prefix", "sym:stack.bicep::sku", "sym:stack.bicep::vnet", "sym:stack.bicep::vnet.subnet",
    ]);
    assert_eq!(lines(&ex, EdgeKind::Declares), [
        "file:stack.bicep -> sym:stack.bicep::label []",
        "file:stack.bicep -> sym:stack.bicep::location [export]",
        "file:stack.bicep -> sym:stack.bicep::output/id [export]",
        "file:stack.bicep -> sym:stack.bicep::prefix []",
        "file:stack.bicep -> sym:stack.bicep::sku [export]",
        "file:stack.bicep -> sym:stack.bicep::vnet []",
        "sym:stack.bicep::vnet -> sym:stack.bicep::vnet.subnet []",
    ]);
    let node = |id: &str| ex.nodes.iter().find(|n| n.id == id).unwrap();
    let location = node("sym:stack.bicep::location");
    assert_eq!(location.body, "// The region every resource lands in.\nparam location string", "the comment reaches over the decorator");
    assert_eq!((node("sym:stack.bicep::vnet").line, node("sym:stack.bicep::vnet").end), (8, 13));
    assert_eq!((node("sym:stack.bicep::vnet.subnet").line, node("sym:stack.bicep::vnet.subnet").end), (10, 12));
}

const REFS: &str = "\
param location string
param n int = 2
param description string
var name = '${location}-app'
resource vnet 'Microsoft.Network/virtualNetworks@2023-04-01' existing = {
  name: name
  resource subnet 'subnets' existing = {
    name: 'default'
  }
}
@description('the vault')
resource kv 'Microsoft.KeyVault/vaults@2023-02-01' = {
  name: 'kv'
  location: location
  properties: {
    subnetId: vnet::subnet.id
    copies: [for n in range(0, 3): n]
  }
}
output id string = kv.id
";

#[test]
fn a_name_read_in_the_file_is_a_reference_and_a_key_a_loop_variable_or_a_decorator_is_not() {
    let ex = extract(&[("stack.bicep", REFS)], "stack.bicep");
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:stack.bicep::kv -> sym:stack.bicep::location []",
        "sym:stack.bicep::kv -> sym:stack.bicep::vnet.subnet []",
        "sym:stack.bicep::name -> sym:stack.bicep::location []",
        "sym:stack.bicep::output/id -> sym:stack.bicep::kv []",
        "sym:stack.bicep::vnet -> sym:stack.bicep::name []",
    ]);
}

#[test]
fn a_lambda_parameter_and_an_iterable_named_like_a_declaration_bind_only_where_they_are_written() {
    let src = "param n int\nparam items array\nvar a = map(items, n => n)\nvar b = map(items, (n, i) => n)\nvar c = [for n in items: n]\nvar d = [for item in items: n]\n";
    let ex = extract(&[("s.bicep", src)], "s.bicep");
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:s.bicep::a -> sym:s.bicep::items []",
        "sym:s.bicep::b -> sym:s.bicep::items []",
        "sym:s.bicep::c -> sym:s.bicep::items []",
        "sym:s.bicep::d -> sym:s.bicep::items []",
        "sym:s.bicep::d -> sym:s.bicep::n []",
    ]);
}

#[test]
fn a_decorator_argument_is_read_for_the_declaration_it_decorates() {
    let src = "param max int\n@maxValue(max)\nparam n int\n";
    let ex = extract(&[("s.bicep", src)], "s.bicep");
    assert_eq!(lines(&ex, EdgeKind::References), ["sym:s.bicep::n -> sym:s.bicep::max []"]);
}

const MODULES: &str = "\
module net './parts/lan.bicep' = {
  name: 'net'
}
module db 'parts/pg.bicep' = if (true) {
  name: 'db'
}
module reg 'br/public:avm/res/network/virtual-network:0.1.0' = {
  name: 'reg'
}
module gone './parts/gone.bicep' = {
  name: 'gone'
}
";

#[test]
fn a_module_path_this_repository_holds_is_imported_and_a_registry_or_missing_path_is_not() {
    let files = [("stack.bicep", MODULES), ("parts/lan.bicep", "param x string\n"), ("parts/pg.bicep", "param y string\n")];
    let ex = extract(&files, "stack.bicep");
    assert_eq!(lines(&ex, EdgeKind::Imports), [
        "file:stack.bicep -> file:parts/lan.bicep [*]",
        "file:stack.bicep -> file:parts/pg.bicep [*]",
    ]);
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:stack.bicep::db -> sym:parts/pg.bicep::y []",
        "sym:stack.bicep::net -> sym:parts/lan.bicep::x []",
    ]);
}

#[test]
fn a_module_path_climbing_above_the_root_or_naming_the_file_itself_reaches_nothing() {
    let src = "module up '../../lan.bicep' = {\n  name: 'up'\n}\nmodule me './s.bicep' = {\n  name: 'me'\n}\nmodule abs '/parts/lan.bicep' = {\n  name: 'abs'\n}\n";
    let files = [("stack/s.bicep", src), ("lan.bicep", "param x string\n"), ("parts/lan.bicep", "param x string\n")];
    let ex = extract(&files, "stack/s.bicep");
    assert_eq!(lines(&ex, EdgeKind::Imports), Vec::<String>::new());
    assert_eq!(lines(&ex, EdgeKind::References), Vec::<String>::new());
}

#[test]
fn a_module_deployed_in_a_loop_is_imported_and_its_indexed_output_is_a_reference() {
    // Azure's fan-out: one module per item, read back through `mod[i].outputs`.
    let files = [
        ("stack.bicep", "param names array\nmodule dbs './parts/pg.bicep' = [for n in names: {\n  name: n\n}]\noutput first string = dbs[0].outputs.cs\n"),
        ("parts/pg.bicep", "param y string\noutput cs string = y\n"),
    ];
    let ex = extract(&files, "stack.bicep");
    assert_eq!(lines(&ex, EdgeKind::Imports), ["file:stack.bicep -> file:parts/pg.bicep [*]"]);
    let refs = lines(&ex, EdgeKind::References);
    for want in [
        "sym:stack.bicep::dbs -> sym:parts/pg.bicep::output/cs []",
        "sym:stack.bicep::dbs -> sym:stack.bicep::names []",
        "sym:stack.bicep::output/first -> sym:stack.bicep::dbs []",
    ] {
        assert!(refs.contains(&want.to_string()), "{want} missing: {refs:?}");
    }
    assert!(!refs.iter().any(|r| r.contains("::n ")), "the loop variable is bound by the loop: {refs:?}");
}

#[test]
fn an_id_in_a_comment_or_string_is_cited_by_the_declaration_holding_it() {
    let ex = extract(&[("stack.bicep", "// ADR-004 pins the region\nparam location string = 'FR-APP-54'\n")], "stack.bicep");
    assert_eq!(lines(&ex, EdgeKind::References), [
        "file:stack.bicep -> ADR-004 [comment]",
        "sym:stack.bicep::location -> FR-APP-54 [string]",
    ]);
}
