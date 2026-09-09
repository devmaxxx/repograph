//! Edge cases of the document extractor, each on an inline text so the shape of a head, a
//! body, a task line or a link is pinned by the assertion and not by a shared fixture.

use super::DocExtractor;
use crate::model::{EdgeKind, Extraction, Extractor, Node, NodeKind};

fn extract(rel: &str, text: &str) -> Extraction {
    DocExtractor::new(crate::families::test_matcher()).extract(rel, text)
}

fn node<'a>(ex: &'a Extraction, id: &str) -> &'a Node {
    ex.nodes.iter().find(|n| n.id == id).unwrap_or_else(|| panic!("no node {id}; have {:?}", ex.nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>()))
}

fn has(ex: &Extraction, s: &str, t: &str, k: EdgeKind) -> bool {
    ex.edges.iter().any(|e| e.source == s && e.target == t && e.kind == k)
}

fn refs<'a>(ex: &'a Extraction, s: &str) -> Vec<&'a str> {
    let mut v: Vec<&str> = ex.edges.iter().filter(|e| e.source == s && e.kind == EdgeKind::References).map(|e| e.target.as_str()).collect();
    v.sort();
    v
}

// ---------------------------------------------------------------- heads

#[test]
fn bold_head_with_trailing_refs_keeps_the_title_clean_and_the_refs() {
    let ex = extract("docs/11.md", "**FR-APP-04 · MUST · Реальное время календаря** (`N-147`, `FR-DM-39`).\nтело\n");
    assert_eq!(node(&ex, "FR-APP-04").label, "Реальное время календаря");
    assert_eq!(node(&ex, "FR-APP-04").body, "(`N-147`, `FR-DM-39`).\nтело", "the tail opens the body so its words still retrieve");
    assert_eq!(refs(&ex, "FR-APP-04"), ["FR-DM-39", "N-147"]);
    assert!(ex.nodes.iter().all(|n| n.kind != NodeKind::Entity), "backticked ids are references, not entities");
}

#[test]
fn heading_head_with_the_modality_after_a_dash() {
    let ex = extract("docs/03.md", "### FR-DM-01 · Идентификаторы — MUST\nтело\n### FR-DM-02 · Время – SHOULD\n### FR-DM-03 · Деньги и налоги — LATER\n");
    assert_eq!(node(&ex, "FR-DM-01").label, "Идентификаторы");
    assert_eq!(node(&ex, "FR-DM-02").label, "Время");
    assert_eq!(node(&ex, "FR-DM-03").label, "Деньги и налоги");
}

#[test]
fn a_dash_inside_the_title_is_not_a_modality() {
    let ex = extract("docs/03.md", "### FR-DM-04 · Деньги — налоги и сборы\n### FR-DM-05 · Что-то — MUST — и детали\n");
    assert_eq!(node(&ex, "FR-DM-04").label, "Деньги — налоги и сборы");
    assert_eq!(node(&ex, "FR-DM-05").label, "Что-то — MUST — и детали");
}

#[test]
fn bullet_bold_head_and_a_bare_head_are_both_dialects() {
    let ex = extract("docs/a.md", "- **FR-WEB-1 · SHOULD · первая**\n  * **FR-WEB-2 · вторая**\nFR-WEB-3 · третья\n");
    assert_eq!(node(&ex, "FR-WEB-1").label, "первая");
    assert_eq!(node(&ex, "FR-WEB-2").label, "вторая");
    assert_eq!(node(&ex, "FR-WEB-3").label, "третья");
}

#[test]
fn non_breaking_spaces_around_the_separator_still_read_as_a_head() {
    let ex = extract("docs/a.md", "**FR-WEB-1\u{a0}·\u{a0}MUST\u{a0}·\u{a0}заголовок**\n");
    assert_eq!(node(&ex, "FR-WEB-1").label, "заголовок");
}

#[test]
fn a_byte_order_mark_does_not_hide_the_first_head() {
    let ex = extract("docs/a.md", "\u{feff}**FR-WEB-1 · MUST · первая**\nтело\n");
    assert_eq!(node(&ex, "FR-WEB-1").label, "первая");
    assert_eq!(node(&ex, "FR-WEB-1").line, 1);
}

#[test]
fn windows_line_endings_leave_no_carriage_return_in_titles_or_bodies() {
    let ex = extract("docs/a.md", "**FR-WEB-1 · MUST · первая**\r\nстрока тела\r\n**FR-WEB-2 · вторая**\r\n");
    assert_eq!(node(&ex, "FR-WEB-1").label, "первая");
    assert_eq!(node(&ex, "FR-WEB-1").body, "строка тела");
    assert_eq!(node(&ex, "FR-WEB-2").line, 3);
}

#[test]
fn kinds_follow_the_family() {
    let ex = extract("docs/a.md", "**INV-20 · MUST · одна функциональность**\n**ADR-0007 · решение**\n**BE-M01 · веха**\n**FR-WEB-1 · требование**\n");
    assert_eq!(node(&ex, "INV-20").kind, NodeKind::Invariant);
    assert_eq!(node(&ex, "ADR-0007").kind, NodeKind::Adr);
    assert_eq!(node(&ex, "BE-M01").kind, NodeKind::Milestone);
    assert_eq!(node(&ex, "FR-WEB-1").kind, NodeKind::Requirement);
}

// ---------------------------------------------------------------- bodies

#[test]
fn body_runs_to_the_next_head_or_heading_and_is_capped() {
    let long: String = (1..=60).map(|i| format!("строка {i}\n")).collect();
    let text = format!("**FR-WEB-1 · MUST · длинная**\n{long}**FR-WEB-2 · вторая**\nтело\n## Раздел\nпроза\n");
    let ex = extract("docs/a.md", &text);
    assert_eq!(node(&ex, "FR-WEB-1").body.lines().count(), 40);
    assert_eq!(node(&ex, "FR-WEB-2").body, "тело");
}

#[test]
fn ranges_and_slash_lists_in_a_body_expand_to_every_id() {
    let ex = extract("docs/a.md", "**FR-WEB-1 · MUST · первая**\nсм. FR-RPT-42…44 и INV-11/12, а также R-1601–R-1602\n");
    assert_eq!(refs(&ex, "FR-WEB-1"), ["FR-RPT-42", "FR-RPT-43", "FR-RPT-44", "INV-11", "INV-12", "R-1601", "R-1602"]);
}

#[test]
fn a_range_wider_than_fifty_is_two_ids_not_a_hundred() {
    let ex = extract("docs/a.md", "**FR-WEB-1 · MUST · первая**\nN-1…N-100\n");
    assert_eq!(refs(&ex, "FR-WEB-1"), ["N-1", "N-100"]);
}

#[test]
fn an_id_never_references_itself_and_a_glued_suffix_is_not_an_id() {
    let ex = extract("docs/a.md", "**FR-WEB-1 · MUST · первая**\nFR-WEB-1 повторён, FR-WEB-2-bis не id, (FR-WEB-3) в скобках, FR-WEB-22000 слишком длинный, FR-WEB-2200 нет\n");
    assert_eq!(refs(&ex, "FR-WEB-1"), ["FR-WEB-2200", "FR-WEB-3"]);
}

#[test]
fn entities_are_backticked_names_in_the_title_only() {
    let ex = extract("docs/a.md", "**FR-WEB-1 · MUST · `CancellationPolicy` и `money.ts` (см. `FR-WEB-2`)**\nв теле `Другое` не сущность\n");
    let entities: Vec<&str> = ex.nodes.iter().filter(|n| n.kind == NodeKind::Entity).map(|n| n.id.as_str()).collect();
    assert_eq!(entities, ["entity:CancellationPolicy", "entity:money.ts"]);
    assert!(has(&ex, "FR-WEB-1", "entity:CancellationPolicy", EdgeKind::References));
    assert!(has(&ex, "FR-WEB-1", "FR-WEB-2", EdgeKind::References));
}

#[test]
fn an_entity_named_after_the_closing_bold_still_belongs_to_the_head() {
    let ex = extract("docs/a.md", "**FR-WEB-1 · MUST · Черновик услуги** (`Service.is_draft`, FR-WEB-2).\n");
    assert_eq!(node(&ex, "FR-WEB-1").label, "Черновик услуги");
    assert!(has(&ex, "FR-WEB-1", "entity:Service.is_draft", EdgeKind::References));
    assert!(has(&ex, "FR-WEB-1", "FR-WEB-2", EdgeKind::References));
}

#[test]
fn a_head_inside_a_fenced_code_block_is_an_example_not_a_requirement() {
    let ex = extract("docs/style.md", "Пример:\n```md\n**FR-WEB-9 · MUST · пример**\n```\n**FR-WEB-1 · MUST · настоящая**\n");
    assert!(ex.nodes.iter().all(|n| n.id != "FR-WEB-9"));
    assert_eq!(node(&ex, "FR-WEB-1").label, "настоящая");
}

// ---------------------------------------------------------------- prose, tasks, owners

#[test]
fn ids_in_prose_outside_blocks_hang_off_the_file() {
    let ex = extract("docs/a.md", "Проза про FR-PAY-22.\n\n**FR-WEB-1 · MUST · первая**\nтело\n\n## Дальше\nещё N-151\n");
    assert_eq!(refs(&ex, "file:docs/a.md"), ["FR-PAY-22", "N-151"]);
}

#[test]
fn task_lines_in_a_milestone_file_implement_their_ids() {
    let ex = extract("docs/milestones/BE-M01-payments.md", "# Платежи\n- [x] **T01** сделать FR-PAY-22\n- [ ] **T02** и FR-PAY-20/21\n");
    assert_eq!(node(&ex, "BE-M01").label, "Платежи");
    assert_eq!(node(&ex, "BE-M01/T01").kind, NodeKind::Task);
    assert!(has(&ex, "BE-M01", "BE-M01/T01", EdgeKind::Declares));
    assert!(has(&ex, "BE-M01/T01", "FR-PAY-22", EdgeKind::Implements));
    assert!(has(&ex, "BE-M01/T02", "FR-PAY-21", EdgeKind::Implements));
    assert!(!has(&ex, "BE-M01", "BE-M01", EdgeKind::References), "the owner never references itself");
}

#[test]
fn a_task_line_outside_a_milestone_file_is_plain_prose() {
    let ex = extract("docs/notes.md", "- [x] **T01** сделать FR-PAY-22\n");
    assert!(ex.nodes.iter().all(|n| n.kind != NodeKind::Task));
    assert_eq!(refs(&ex, "file:docs/notes.md"), ["FR-PAY-22"]);
}

#[test]
fn an_adr_file_owns_its_prose_under_its_h1() {
    let ex = extract("docs/adr/ADR-0007-postgres.md", "# Use Postgres\nContext: INV-20.\n");
    let adr = node(&ex, "ADR-0007");
    assert_eq!((adr.kind, adr.label.as_str()), (NodeKind::Adr, "Use Postgres"));
    assert!(has(&ex, "ADR-0007", "INV-20", EdgeKind::References));
    assert!(has(&ex, "file:docs/adr/ADR-0007-postgres.md", "ADR-0007", EdgeKind::Declares));
}

#[test]
fn an_owner_file_without_an_h1_is_named_by_its_id() {
    let ex = extract("docs/BE-M02.md", "no heading here\n");
    assert_eq!(node(&ex, "BE-M02").label, "BE-M02");
}

// ---------------------------------------------------------------- links

#[test]
fn links_resolve_relative_paths_and_drop_anchors_and_schemes() {
    let ex = extract("docs/prd/06.md", "[a](./07.md) [b](../adr/ADR-1.md#x) [c](https://x.io/y.md) [d](mailto:a@b.c) [e](/docs/root.md)\n");
    let links: Vec<&str> = ex.edges.iter().filter(|e| e.kind == EdgeKind::Links).map(|e| e.target.as_str()).collect();
    assert_eq!(links, ["file:docs/prd/07.md", "file:docs/adr/ADR-1.md", "file:docs/root.md"]);
}

#[test]
fn an_id_in_link_text_is_a_reference_and_the_link_a_link() {
    let ex = extract("docs/a.md", "см. [FR-PAY-22](06.md#fr-pay-22)\n");
    assert!(has(&ex, "file:docs/a.md", "FR-PAY-22", EdgeKind::References));
    assert!(has(&ex, "file:docs/a.md", "file:docs/06.md", EdgeKind::Links));
}

#[test]
fn an_empty_or_whitespace_document_yields_only_its_file_node() {
    for text in ["", "\n\n", "\u{feff}"] {
        let ex = extract("docs/empty.md", text);
        assert_eq!(ex.nodes.len(), 1, "{text:?}");
        assert!(ex.edges.is_empty());
    }
}

#[test]
fn links_inside_a_requirement_document_resolve_dot_and_dotdot_and_drop_anchors() {
    let ex = extract("docs/prd/06.md", "**FR-WEB-1 · MUST · первая**\nсм. [далее](./07.md) и [решение](../adr/ADR-1.md#x)\n");
    let links: Vec<&str> = ex.edges.iter().filter(|e| e.kind == EdgeKind::Links).map(|e| e.target.as_str()).collect();
    assert_eq!(links, ["file:docs/prd/07.md", "file:docs/adr/ADR-1.md"]);
}

#[test]
fn a_link_to_a_file_not_present_in_the_graph_still_creates_a_links_edge() {
    let ex = extract("docs/06.md", "**FR-WEB-1 · MUST · первая**\nсм. [нет такого](missing.md)\n");
    assert!(has(&ex, "file:docs/06.md", "file:docs/missing.md", EdgeKind::Links));
    assert!(ex.nodes.iter().all(|n| n.id != "file:docs/missing.md"), "the target is only an edge, not a declared node");
}

#[test]
fn a_markdown_link_inside_a_fenced_code_block_is_not_extracted_as_a_link() {
    let ex = extract("docs/06.md", "**FR-WEB-1 · MUST · первая**\nпример синтаксиса:\n```md\n[пример](example.md)\n```\nсм. [реальная](real.md)\n");
    let links: Vec<&str> = ex.edges.iter().filter(|e| e.kind == EdgeKind::Links).map(|e| e.target.as_str()).collect();
    assert_eq!(links, ["file:docs/real.md"]);
}

#[test]
fn a_fence_toggle_holds_correctly_across_two_separate_code_blocks() {
    let ex = extract("docs/06.md", "**FR-WEB-1 · MUST · первая**\n[a](a.md)\n```\n[x](x.md)\n```\n[b](b.md)\n```\n[y](y.md)\n```\n[c](c.md)\n");
    let links: Vec<&str> = ex.edges.iter().filter(|e| e.kind == EdgeKind::Links).map(|e| e.target.as_str()).collect();
    assert_eq!(links, ["file:docs/a.md", "file:docs/b.md", "file:docs/c.md"]);
}

// `RequirementScanner::scan` sorts and dedups its own edges before `links::scan` appends to the
// same vec, so — unlike References or Declares — Links edges keep the order they were written in.
#[test]
fn multiple_links_stay_in_text_order_after_the_requirement_edges_are_sorted() {
    let ex = extract("docs/06.md", "**FR-WEB-1 · MUST · первая**\n[б](b.md) [а](a.md) [в](v.md)\n");
    let links: Vec<&str> = ex.edges.iter().filter(|e| e.kind == EdgeKind::Links).map(|e| e.target.as_str()).collect();
    assert_eq!(links, ["file:docs/b.md", "file:docs/a.md", "file:docs/v.md"]);
}

/// A checkout with core.autocrlf=true — Git for Windows' installer default — is the same corpus:
/// every id, label, body and line number, and so every passage hash a questions.json was written
/// under. By construction today, since the extractors split with `lines()`; pinned so a slice of
/// raw text in some future extractor cannot quietly make it false.
#[test]
fn a_crlf_document_extracts_the_same_nodes_and_edges_as_its_lf_twin() {
    let lf = "**FR-PAY-1 · MUST · Штраф за отмену**\n\nШтраф списывается сам (INV-1).\n\n```\n**FR-X-9 · MUST · not a head**\n```\n\n**INV-1 · MUST · Деньги не сгорают**\n\nОтмена не сжигает деньги.\n\n- [ ] BE-M01-T1 · задача\n\n[а](a.md)\n";
    let crlf = lf.replace('\n', "\r\n");
    let (a, b) = (extract("docs/pay.md", lf), extract("docs/pay.md", &crlf));
    let extracted: Vec<&str> = a.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(extracted, ["file:docs/pay.md", "FR-PAY-1", "INV-1"], "two extractions of nothing would agree too");
    assert_eq!(a.nodes, b.nodes);
    assert_eq!(a.edges, b.edges);
    assert!(a.nodes.iter().all(|n| !n.body.contains('\r') && !n.label.contains('\r')));
}
