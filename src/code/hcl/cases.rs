//! HCL's shapes on inline files: Terraform's addresses and directory resolution, a bake file's
//! blocks, and the lock file, which declares nothing an address names.

use crate::code::prose::testing::{ids, lines, repo};
use crate::model::{EdgeKind, Extraction};

const GLOBS: &[&str] = &["**/*.tf", "**/*.hcl"];

fn extract(files: &[(&str, &str)], rel: &str) -> Extraction {
    let (_dir, resolver) = repo(GLOBS, files);
    let text = files.iter().find(|(f, _)| *f == rel).map(|(_, t)| *t).unwrap();
    super::extract(&resolver, rel, text)
}

const DECLS: &str = r#"# Region the server lands in.
variable "region" {
  type = string
}
locals {
  name = "web-${var.region}"
}
provider "hcloud" {
  token = var.token
}
data "aws_ami" "ubuntu" {}
resource "aws_instance" "web" {
  ami = data.aws_ami.ubuntu.id
}
module "dns" {
  source = "./dns"
}
output "ip" {
  value = aws_instance.web[0].public_ip
}
terraform {
  required_version = ">= 1.6"
}
"#;

#[test]
fn every_terraform_block_that_names_an_address_is_a_symbol_with_slashes_for_dots() {
    let ex = extract(&[("infra/main.tf", DECLS)], "infra/main.tf");
    assert_eq!(ids(&ex), [
        "file:infra/main.tf", "sym:infra/main.tf::aws_instance/web", "sym:infra/main.tf::data/aws_ami/ubuntu",
        "sym:infra/main.tf::local/name", "sym:infra/main.tf::module/dns", "sym:infra/main.tf::output/ip",
        "sym:infra/main.tf::provider/hcloud", "sym:infra/main.tf::var/region",
    ], "`terraform {{}}` declares nothing a reference names");
    let declares = lines(&ex, EdgeKind::Declares);
    assert!(declares.contains(&"file:infra/main.tf -> sym:infra/main.tf::var/region [export]".to_string()), "{declares:?}");
    assert!(declares.contains(&"file:infra/main.tf -> sym:infra/main.tf::output/ip [export]".to_string()), "{declares:?}");
    assert!(declares.contains(&"file:infra/main.tf -> sym:infra/main.tf::aws_instance/web []".to_string()), "{declares:?}");
    let node = |id: &str| ex.nodes.iter().find(|n| n.id == id).unwrap();
    assert_eq!(node("sym:infra/main.tf::var/region").body, "# Region the server lands in.\nvariable \"region\" {");
    assert_eq!((node("sym:infra/main.tf::var/region").line, node("sym:infra/main.tf::var/region").end), (2, 4));
    assert_eq!((node("sym:infra/main.tf::local/name").line, node("sym:infra/main.tf::local/name").end), (6, 6));
}

const MAIN: &str = r#"locals {
  name = "web-${var.region}"
}
provider "hcloud" {
  token = var.token
}
data "aws_ami" "ubuntu" {}
resource "aws_instance" "web" {
  ami   = data.aws_ami.ubuntu.id
  zone  = var.zone
  count = length(each.key)
}
module "dns" {
  source = "./dns"
}
output "ip" {
  value = aws_instance.web[0].public_ip
}
"#;

#[test]
fn a_reference_resolves_against_its_own_file_then_its_directory_and_a_module_source_imports_a_directory() {
    let files = [
        ("infra/main.tf", MAIN),
        ("infra/variables.tf", "variable \"region\" {}\nvariable \"token\" {}\n"),
        ("infra/dns/records.tf", "resource \"hcloud_zone\" \"main\" {\n  name = \"x\"\n}\n"),
        ("other/variables.tf", "variable \"zone\" {}\n"),
    ];
    let ex = extract(&files, "infra/main.tf");
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:infra/main.tf::aws_instance/web -> sym:infra/main.tf::data/aws_ami/ubuntu []",
        "sym:infra/main.tf::local/name -> sym:infra/variables.tf::var/region []",
        "sym:infra/main.tf::module/dns -> sym:infra/dns/records.tf::hcloud_zone/main []",
        "sym:infra/main.tf::output/ip -> sym:infra/main.tf::aws_instance/web []",
        "sym:infra/main.tf::provider/hcloud -> sym:infra/variables.tf::var/token []",
    ], "another directory's variable is another module's, and `each` is a value of the block");
    assert_eq!(lines(&ex, EdgeKind::Imports), ["file:infra/main.tf -> file:infra/dns/records.tf [*]"]);
}

#[test]
fn a_sibling_module_source_is_imported_and_a_module_output_read_references_the_module() {
    // Real stacks keep modules beside the environment, `../modules/<name>`, and read their outputs.
    let files = [
        ("infra/staging/main.tf", "module \"dns\" {\n  source = \"../modules/dns\"\n}\noutput \"zone\" {\n  value = module.dns.zone_id\n}\n"),
        ("infra/modules/dns/zone.tf", "resource \"hcloud_zone\" \"main\" {\n  name = \"x\"\n}\noutput \"zone_id\" {\n  value = hcloud_zone.main.id\n}\n"),
    ];
    let ex = extract(&files, "infra/staging/main.tf");
    assert_eq!(lines(&ex, EdgeKind::Imports), ["file:infra/staging/main.tf -> file:infra/modules/dns/zone.tf [*]"]);
    let refs = lines(&ex, EdgeKind::References);
    assert!(refs.contains(&"sym:infra/staging/main.tf::output/zone -> sym:infra/staging/main.tf::module/dns []".to_string()), "{refs:?}");
    assert!(refs.contains(&"sym:infra/staging/main.tf::module/dns -> sym:infra/modules/dns/zone.tf::output/zone_id []".to_string()), "{refs:?}");
}

const BAKE: &str = r#"variable "TAG" {
  default = "latest"
}
group "default" {
  targets = ["api", "web"]
}
target "base" {
  platforms = ["linux/amd64"]
}
target "api" {
  inherits = ["base"]
  tags     = ["app/api:${TAG}"]
}
target "web" {
  inherits = ["base", "missing"]
}
"#;

#[test]
fn a_bake_file_is_read_by_block_type_and_label_and_its_inherits_and_targets_are_references() {
    let ex = extract(&[("docker-bake.hcl", BAKE)], "docker-bake.hcl");
    assert_eq!(ids(&ex), [
        "file:docker-bake.hcl", "sym:docker-bake.hcl::group/default", "sym:docker-bake.hcl::target/api",
        "sym:docker-bake.hcl::target/base", "sym:docker-bake.hcl::target/web", "sym:docker-bake.hcl::variable/TAG",
    ]);
    assert!(lines(&ex, EdgeKind::Declares).iter().all(|l| l.ends_with("[export]")));
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:docker-bake.hcl::group/default -> sym:docker-bake.hcl::target/api []",
        "sym:docker-bake.hcl::group/default -> sym:docker-bake.hcl::target/web []",
        "sym:docker-bake.hcl::target/api -> sym:docker-bake.hcl::target/base []",
        "sym:docker-bake.hcl::target/web -> sym:docker-bake.hcl::target/base []",
    ]);
}

const LOCK: &str = r#"# This file is maintained automatically by "terraform init".
provider "registry.terraform.io/hetznercloud/hcloud" {
  version     = "1.49.1"
  constraints = "~> 1.49"
  hashes = [
    "h1:abc=",
  ]
}
"#;

#[test]
fn a_lock_file_handed_to_the_extractor_declares_nothing_because_its_labels_are_registry_paths() {
    let ex = extract(&[("infra/.terraform.lock.hcl", LOCK)], "infra/.terraform.lock.hcl");
    assert_eq!(ids(&ex), ["file:infra/.terraform.lock.hcl"]);
    assert!(ex.edges.is_empty(), "{:?}", ex.edges);
}

#[test]
fn a_for_variable_and_a_dynamic_iterator_are_not_references_to_a_declaration_of_the_same_name() {
    let main = "resource \"web\" \"x\" {\n  a = [for web in var.list : web.x]\n  dynamic \"ebs\" {\n    for_each = var.disks\n    iterator = disk\n    content {\n      size = disk.x\n      kind = ebs.x\n    }\n  }\n  dynamic \"rule\" {\n    for_each = var.rules\n    content {\n      port = rule.x\n    }\n  }\n}\nresource \"disk\" \"x\" {}\nresource \"ebs\" \"x\" {}\nresource \"rule\" \"x\" {}\nvariable \"list\" {}\n";
    let ex = extract(&[("m/main.tf", main)], "m/main.tf");
    // `web.x` is the loop's own element, though `resource "web" "x"` exists; `ebs.x` is no iterator here
    // because `iterator = disk` renamed it, so it is a plain reference.
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:m/main.tf::web/x -> sym:m/main.tf::ebs/x []",
        "sym:m/main.tf::web/x -> sym:m/main.tf::var/list []",
    ]);
}

#[test]
fn a_registry_git_absolute_or_root_escaping_module_source_reaches_nothing() {
    let files = [
        ("infra/main.tf", "module \"a\" {\n  source = \"hashicorp/consul/aws\"\n}\nmodule \"b\" {\n  source = \"git::https://x.example/m.git\"\n}\nmodule \"c\" {\n  source = \"/abs/m\"\n}\nmodule \"d\" {\n  source = \"../../../m\"\n}\nmodule \"e\" {\n  source = \"modules/m\"\n}\nmodule \"f\" {\n  source = \"./${var.x}\"\n}\n"),
        ("infra/modules/m/x.tf", "variable \"y\" {}\n"),
        ("m/x.tf", "variable \"y\" {}\n"),
    ];
    let ex = extract(&files, "infra/main.tf");
    assert!(lines(&ex, EdgeKind::Imports).is_empty(), "{:?}", ex.edges);
    assert!(lines(&ex, EdgeKind::References).is_empty(), "{:?}", ex.edges);
}

#[test]
fn a_built_in_function_is_not_a_reference_and_an_undeclared_address_is_not_an_edge() {
    let main = "resource \"a\" \"b\" {\n  x = length(var.n)\n  y = aws_ghost.g.id\n}\nresource \"length\" \"n\" {}\nvariable \"n\" {}\n";
    let ex = extract(&[("m/main.tf", main)], "m/main.tf");
    assert_eq!(lines(&ex, EdgeKind::References), ["sym:m/main.tf::a/b -> sym:m/main.tf::var/n []"]);
}

#[test]
fn the_last_operand_of_a_comparison_keeps_its_attribute_steps() {
    let main = "variable \"a\" {}\nvariable \"b\" {}\noutput \"o\" {\n  value = var.a == var.b\n}\n";
    let ex = extract(&[("m/main.tf", main)], "m/main.tf");
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:m/main.tf::output/o -> sym:m/main.tf::var/a []",
        "sym:m/main.tf::output/o -> sym:m/main.tf::var/b []",
    ]);
}

#[test]
fn a_for_variable_is_bound_in_the_body_and_the_condition_but_not_in_its_own_collection() {
    // The resources sit in another file, so the self-reference guard cannot hide a wrong edge.
    let files = [
        ("m/b.tf", "resource \"web\" \"id\" {}\n"),
        ("m/c.tf", "locals {\n  b = [for web in var.x : web.id]\n  c = [for x in web.id : x]\n  d = {for web, v in var.x : web => web.id if web.id != \"\"}\n  e = [for web in var.x : [for y in web.id : web.id]]\n}\n"),
    ];
    let ex = extract(&files, "m/c.tf");
    assert_eq!(lines(&ex, EdgeKind::References), ["sym:m/c.tf::local/c -> sym:m/b.tf::web/id []"]);
}

fn calling(pairs: &[(&str, &str)]) -> crate::model::Graph {
    let mut g = crate::model::Graph::default();
    for (from, to) in pairs {
        g.edges.insert(crate::model::Edge {
            source: format!("file:{from}"),
            target: format!("file:{to}"),
            kind: EdgeKind::Imports,
            context: "*".into(),
            file: (*from).into(),
        });
    }
    g
}

fn widened(stale: &[&str], removed: &[&str], g: &crate::model::Graph, all: &[&str]) -> Vec<String> {
    let owned = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let mut w = super::widen(&owned(stale), &owned(removed), g, &owned(all));
    w.sort();
    w
}

const DIR: &[&str] = &["m/a.tf", "m/b.tf", "m/c.tf", "m/.terraform.lock.hcl", "n/x.tf"];

#[test]
fn a_changed_tf_file_widens_to_every_other_tf_file_of_its_directory_and_no_other() {
    let g = crate::model::Graph::default();
    assert_eq!(widened(&["m/a.tf"], &[], &g, DIR), ["m/b.tf", "m/c.tf"]);
}

#[test]
fn a_removed_tf_file_widens_to_the_siblings_left_and_to_nothing_of_itself() {
    let g = crate::model::Graph::default();
    assert_eq!(widened(&[], &["m/gone.tf"], &g, DIR), ["m/a.tf", "m/b.tf", "m/c.tf"]);
}

#[test]
fn a_file_whose_module_call_imports_the_directory_is_read_again() {
    let g = calling(&[("env/main.tf", "m/a.tf"), ("env/main.tf", "m/b.tf"), ("other/main.tf", "n/x.tf")]);
    let all = ["env/main.tf", "other/main.tf", "m/a.tf", "m/b.tf", "n/x.tf"];
    assert_eq!(widened(&["m/b.tf"], &[], &g, &all), ["env/main.tf", "m/a.tf"]);
}

#[test]
fn a_file_already_being_read_is_left_out() {
    let g = calling(&[("env/main.tf", "m/a.tf")]);
    let all = ["env/main.tf", "m/a.tf", "m/b.tf"];
    assert_eq!(widened(&["m/a.tf", "m/b.tf", "env/main.tf"], &[], &g, &all), Vec::<String>::new());
}

#[test]
fn a_change_that_is_not_a_tf_file_widens_nothing() {
    let g = calling(&[("env/main.tf", "m/a.tf")]);
    assert!(widened(&["m/.terraform.lock.hcl"], &[], &g, DIR).is_empty());
    assert!(widened(&["m/docker-bake.hcl", "m/notes.md"], &["m/x.hcl"], &g, DIR).is_empty());
}

#[test]
fn a_negated_operand_keeps_its_attribute_steps_however_deep_the_operation_nests() {
    let main = "variable \"on\" {}\nresource \"r\" \"x\" {}\nresource \"r\" \"y\" {}\noutput \"a\" {\n  value = !var.on\n}\noutput \"b\" {\n  value = -r.x.id\n}\noutput \"c\" {\n  value = var.on && !r.y.id\n}\n";
    let ex = extract(&[("m/main.tf", main)], "m/main.tf");
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:m/main.tf::output/a -> sym:m/main.tf::var/on []",
        "sym:m/main.tf::output/b -> sym:m/main.tf::r/x []",
        "sym:m/main.tf::output/c -> sym:m/main.tf::r/y []",
        "sym:m/main.tf::output/c -> sym:m/main.tf::var/on []",
    ]);
}

#[test]
fn aliased_provider_blocks_are_one_symbol() {
    let main = "provider \"aws\" {\n  alias = \"one\"\n}\nprovider \"aws\" {\n  alias = \"two\"\n}\n";
    let ex = extract(&[("m/main.tf", main)], "m/main.tf");
    assert_eq!(ex.nodes.iter().filter(|n| n.id == "sym:m/main.tf::provider/aws").count(), 1);
}
