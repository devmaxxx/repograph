//! HCL through the binary: `impact` on a `/`-joined address, a trace across a Terraform directory,
//! the lock file a node with nothing under it, and a variable moved to a new file followed after an `update`.

mod common;

use common::{ok, path, write};

const VARIABLES: &str = "variable \"domain\" {\n  type = string\n}\nvariable \"server_type\" {\n  default = \"cx22\"\n}\nvariable \"ssh_public_key\" {\n  type = string\n}\n";
const SERVER: &str = "resource \"hcloud_ssh_key\" \"admin\" {\n  name       = \"admin\"\n  public_key = var.ssh_public_key\n}\nresource \"hcloud_server\" \"staging\" {\n  server_type = var.server_type\n  ssh_keys    = [hcloud_ssh_key.admin.id]\n}\n";
const DNS: &str = "resource \"hcloud_zone_rrset\" \"a\" {\n  zone    = var.domain\n  records = [hcloud_server.staging.ipv4_address]\n}\n";
const LOCK: &str = "provider \"registry.terraform.io/hetznercloud/hcloud\" {\n  version = \"1.49.1\"\n}\n";

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    // HCL is read behind an explicit glob until its readings pass (spec §6).
    write(r, "repograph.toml", "doc_globs = []\ncode_globs = [\"**/*.tf\", \"**/*.hcl\"]\n");
    write(r, "infra/staging/variables.tf", VARIABLES);
    write(r, "infra/staging/server.tf", SERVER);
    write(r, "infra/staging/dns.tf", DNS);
    write(r, "infra/staging/.terraform.lock.hcl", LOCK);
    dir
}

#[test]
fn impact_on_a_slash_joined_address_names_the_resource_reading_it_from_a_sibling_file() {
    let dir = repo();
    ok(dir.path(), &["build"]);
    let out = ok(dir.path(), &["impact", "hcloud_server/staging", "--depth", "3"]);
    assert!(out.starts_with("sym:infra/staging/server.tf::hcloud_server/staging"), "{out}");
    assert!(out.contains("sym:infra/staging/dns.tf::hcloud_zone_rrset/a"), "{out}");
}

#[test]
fn a_trace_crosses_the_directory_from_a_record_to_the_variable_its_server_key_reads() {
    let dir = repo();
    ok(dir.path(), &["build"]);
    let out = ok(dir.path(), &["trace", "hcloud_zone_rrset/a", "var/ssh_public_key"]);
    assert_eq!(path(&out), [
        "sym:infra/staging/dns.tf::hcloud_zone_rrset/a",
        "sym:infra/staging/server.tf::hcloud_server/staging",
        "sym:infra/staging/server.tf::hcloud_ssh_key/admin",
        "sym:infra/staging/variables.tf::var/ssh_public_key",
    ], "{out}");
}

#[test]
fn the_lock_file_is_a_file_that_declares_nothing() {
    let dir = repo();
    ok(dir.path(), &["build"]);
    // The walker reads dotted paths, so the lock file is walked; its labels are registry paths no address can hold.
    let out = ok(dir.path(), &["explain", "file:infra/staging/.terraform.lock.hcl"]);
    assert!(out.starts_with("file:infra/staging/.terraform.lock.hcl"), "{out}");
    assert!(!out.contains("Declares"), "{out}");
}

#[test]
fn a_variable_moved_to_a_new_file_is_still_what_its_readers_reference_after_an_update() {
    let dir = repo();
    let r = dir.path();
    ok(r, &["build"]);
    // `dns.tf` does not change; the address it reads moves between two other files of its directory.
    write(r, "infra/staging/variables.tf", &VARIABLES.replace("variable \"domain\" {\n  type = string\n}\n", ""));
    write(r, "infra/staging/zone.tf", "variable \"domain\" {\n  type = string\n}\n");
    ok(r, &["update"]);
    let out = ok(r, &["impact", "var/domain", "--depth", "3"]);
    assert!(out.starts_with("sym:infra/staging/zone.tf::var/domain"), "{out}");
    assert!(out.contains("sym:infra/staging/dns.tf::hcloud_zone_rrset/a"), "the sibling was read again: {out}");
}
