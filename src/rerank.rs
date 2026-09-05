//! `ask --rerank`: the configured model command picks the seeds from the deep fused candidate
//! list. Costs tokens per question (≈19k on the bench corpus); nothing runs without the flag.

use crate::enrich::run_command;

/// How far down the fused list the model looks. Measured on the bench corpus with questions
/// enriched and a snippet per candidate: 100 deep leaves the target at pool rank 142 out of
/// reach (13/14), 200 deep reaches it (14/14, three runs); tokens per question scale with it.
pub const DEPTH: usize = 200;

/// Characters of a candidate's body shown after its title.
const SNIPPET: usize = 120;

/// What the model sees of a candidate: its title, then the start of its text on one line.
pub fn text(n: &crate::model::Node) -> String {
    let title: String = n.label.chars().take(100).collect();
    let body: Vec<&str> = n.body.split_whitespace().collect();
    let mut snippet: String = body.join(" ").chars().take(SNIPPET).collect();
    if snippet.is_empty() { return title; }
    if n.body.chars().count() > SNIPPET { snippet.push('…'); }
    format!("{title} — {snippet}")
}

pub fn prompt(question: &str, candidates: &[(String, String)]) -> String {
    let mut p = format!(
        "Question: \"{question}\"\nBelow are candidate entries as `id<TAB>title — start of text`. Output the ids \
         of up to 5 entries most relevant to the question, one per line, most relevant first. Nothing but ids.\n\n");
    for (id, text) in candidates {
        p.push_str(&format!("{id}\t{text}\n"));
    }
    p
}

/// Ids the model named, in its order, restricted to the candidates it was shown.
pub fn parse(output: &str, candidates: &[(String, String)]) -> Vec<String> {
    let mut picked = Vec::new();
    for line in output.lines() {
        let id = line.trim().trim_end_matches('.');
        if candidates.iter().any(|(c, _)| c == id) && !picked.iter().any(|p| p == id) {
            picked.push(id.to_string());
        }
    }
    picked
}

/// A failing command answered with an empty pick and the line that says why, so `ask` falls
/// back to the fused order instead of failing the question. The line is returned rather than
/// printed: a resident process's stderr is a socket reply, and a client that quietly loses this
/// warning cannot tell a reranked answer from an unreranked one.
pub fn run_or_notice(command: &str, question: &str, candidates: &[(String, String)]) -> (Vec<String>, Option<String>) {
    match run_command(command, &prompt(question, candidates)) {
        Ok(out) => (parse(&out, candidates), None),
        Err(e) => (Vec::new(), Some(format!("rerank: {e:#}; answering from the fused order"))),
    }
}

/// The same run for `bench`, whose stderr is the reader's terminal.
pub fn run(command: &str, question: &str, candidates: &[(String, String)]) -> Vec<String> {
    let (picked, notice) = run_or_notice(command, question, candidates);
    if let Some(n) = notice { eprintln!("{n}"); }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cands() -> Vec<(String, String)> {
        vec![("FR-PAY-20".into(), "cancel".into()), ("FR-PAY-22".into(), "refund".into()), ("N-151".into(), "policy".into())]
    }

    #[test]
    fn parse_keeps_shown_ids_in_the_models_order_without_repeats() {
        let out = "N-151\nFR-PAY-22.\nFR-PAY-99\nN-151\n\nnot an id\n";
        assert_eq!(parse(out, &cands()), vec!["N-151".to_string(), "FR-PAY-22".to_string()]);
    }

    #[test]
    fn prompt_lists_every_candidate_verbatim() {
        let p = prompt("why", &[("A-1".into(), "cancel — a visit…".into()), ("B-2".into(), "refund".into())]);
        assert!(p.contains("\"why\""));
        assert!(p.contains("A-1\tcancel — a visit…\n"));
        assert!(p.contains("B-2\trefund\n"));
    }

    #[test]
    fn text_clips_the_title_and_shows_the_start_of_a_collapsed_body() {
        let mut x = crate::model::Extraction::default();
        x.node(crate::model::NodeKind::Requirement, "A-1", &"x".repeat(150), &format!("  first\n\n  line   {}", "y".repeat(200)), "f.md", 1);
        let mut n = x.nodes.remove(0);
        let t = text(&n);
        assert!(t.starts_with(&format!("{} — first line yyy", "x".repeat(100))));
        assert!(!t.contains(&"x".repeat(101)));
        assert!(t.ends_with('…'));
        assert_eq!(t.chars().count(), 100 + 3 + SNIPPET + 1);
        n.body.clear();
        assert_eq!(text(&n), "x".repeat(100));
    }

    #[test]
    fn a_failing_command_yields_no_picks() {
        assert!(run("exit 3", "q", &cands()).is_empty());
    }

    #[test]
    fn only_a_trailing_period_is_stripped_not_other_punctuation() {
        let out = "N-151,\nFR-PAY-22.\n";
        // The comma is left in place, so "N-151," never matches a shown id.
        assert_eq!(parse(out, &cands()), vec!["FR-PAY-22".to_string()]);
    }

    #[test]
    fn prompt_with_no_candidates_still_carries_the_question() {
        let p = prompt("why", &[]);
        assert!(p.contains("\"why\""));
        assert!(p.ends_with("Nothing but ids.\n\n"));
    }

    #[test]
    fn a_whitespace_only_body_falls_back_to_the_title_alone() {
        let mut x = crate::model::Extraction::default();
        x.node(crate::model::NodeKind::Requirement, "A-1", "title", "   \n\t  \n", "f.md", 1);
        let n = x.nodes.remove(0);
        assert_eq!(text(&n), "title");
    }

    #[test]
    fn title_and_snippet_are_clipped_by_chars_not_bytes() {
        let mut x = crate::model::Extraction::default();
        let title: String = "ж".repeat(101);
        let body: String = "щ".repeat(200);
        x.node(crate::model::NodeKind::Requirement, "A-1", &title, &body, "f.md", 1);
        let n = x.nodes.remove(0);
        let t = text(&n);
        assert!(t.starts_with(&"ж".repeat(100)));
        assert!(!t.starts_with(&"ж".repeat(101)));
        assert_eq!(t.chars().count(), 100 + 3 + SNIPPET + 1);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn a_successful_commands_output_is_parsed_into_picks() {
        let picked = run("printf 'N-151\\nFR-PAY-20\\n'", "q", &cands());
        assert_eq!(picked, vec!["N-151".to_string(), "FR-PAY-20".to_string()]);
    }
}
