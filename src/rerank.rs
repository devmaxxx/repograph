//! `ask --rerank`: the configured model command picks the seeds from the deep fused candidate
//! list. Costs tokens per question (≈19k on the bench corpus); nothing runs without the flag.

use crate::command::run_command;

/// How far down the fused list the model looks. Measured on the bench corpus with questions
/// enriched and a snippet per candidate: 100 deep leaves the target at pool rank 142 out of
/// reach (13/14), 200 deep reaches it (14/14, three runs); tokens per question scale with it.
pub const DEPTH: usize = 200;

/// Characters of a candidate's body shown after its title.
const SNIPPET: usize = 120;

/// The question's stemmed terms, the ones `text` looks for in a candidate's sentences.
pub fn terms(question: &str) -> std::collections::HashSet<String> {
    crate::index::lexical::tokenize(question).into_iter().collect()
}

/// What the model sees of a candidate: its title, then on one line the sentence of its text that
/// shares the most terms with the question, or the start of the text where no later sentence
/// shares more. The start alone hid the evidence: NFR-STAFF-04 says «ведомость мастера не видна»
/// 170 characters in, behind a sentence about attribution, and both models passed it over.
pub fn text(n: &crate::model::Node, terms: &std::collections::HashSet<String>) -> String {
    let title: String = n.label.chars().take(100).collect();
    let body = n.body.split_whitespace().collect::<Vec<_>>().join(" ");
    if body.is_empty() { return title; }
    let from = best_sentence(&body, terms);
    let shown = &body[from..];
    let mut snippet: String = shown.chars().take(SNIPPET).collect();
    if shown.chars().count() > SNIPPET { snippet.push('…'); }
    if from > 0 { snippet.insert(0, '…'); }
    format!("{title} — {snippet}")
}

/// Byte offset of the sentence sharing the most distinct terms with the question; the first
/// sentence wins a tie, so a body that matches nowhere keeps showing its start.
fn best_sentence(body: &str, terms: &std::collections::HashSet<String>) -> usize {
    if terms.is_empty() { return 0; }
    let mut best = (0, 0);
    let mut start = 0;
    let mut chars = body.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let end = matches!(c, '.' | '!' | '?' | ';') && chars.peek().is_none_or(|&(_, next)| next == ' ');
        if !end && chars.peek().is_some() { continue; }
        let stop = i + c.len_utf8();
        let shared = crate::index::lexical::tokenize(&body[start..stop]).into_iter()
            .filter(|t| terms.contains(t)).collect::<std::collections::HashSet<_>>().len();
        if shared > best.1 { best = (start, shared); }
        start = (stop + 1).min(body.len());
    }
    best.0
}

pub fn prompt(question: &str, candidates: &[(String, String)]) -> String {
    let mut p = format!(
        "Question: \"{question}\"\nBelow are candidate entries as `id<TAB>title — excerpt of its text`. Output the ids \
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
        let t = text(&n, &Default::default());
        assert!(t.starts_with(&format!("{} — first line yyy", "x".repeat(100))));
        assert!(!t.contains(&"x".repeat(101)));
        assert!(t.ends_with('…'));
        assert_eq!(t.chars().count(), 100 + 3 + SNIPPET + 1);
        n.body.clear();
        assert_eq!(text(&n, &Default::default()), "x".repeat(100));
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
        assert_eq!(text(&n, &Default::default()), "title");
    }

    #[test]
    fn title_and_snippet_are_clipped_by_chars_not_bytes() {
        let mut x = crate::model::Extraction::default();
        let title: String = "ж".repeat(101);
        let body: String = "щ".repeat(200);
        x.node(crate::model::NodeKind::Requirement, "A-1", &title, &body, "f.md", 1);
        let n = x.nodes.remove(0);
        let t = text(&n, &Default::default());
        assert!(t.starts_with(&"ж".repeat(100)));
        assert!(!t.starts_with(&"ж".repeat(101)));
        assert_eq!(t.chars().count(), 100 + 3 + SNIPPET + 1);
        assert!(t.ends_with('…'));
    }

    fn node(body: &str) -> crate::model::Node {
        let mut x = crate::model::Extraction::default();
        x.node(crate::model::NodeKind::Requirement, "NFR-STAFF-04", "Приватность и атрибуция.", body, "f.md", 1);
        x.nodes.remove(0)
    }

    const PRIVACY: &str = "Всё в разделе атрибутировано актору (INV-12): правка графика, утверждение отсутствия, \
        изменение ставки, строка ведомости, отметка выплаты. Ведомость мастера не видна другим мастерам ни одним \
        экраном и ни одним отчётом; `report.staff.compare` — отдельное право.";

    #[test]
    fn the_snippet_is_the_sentence_that_answers_the_question_not_the_start() {
        let t = text(&node(PRIVACY), &terms("ведомость мастера не видна"));
        assert!(t.starts_with("Приватность и атрибуция. — …Ведомость мастера не видна другим мастерам"), "{t}");
    }

    #[test]
    fn a_question_sharing_no_word_with_the_body_is_shown_its_start() {
        let t = text(&node(PRIVACY), &terms("сертификаты подарочные"));
        assert!(t.starts_with("Приватность и атрибуция. — Всё в разделе атрибутировано"), "{t}");
    }

    #[test]
    fn a_tie_keeps_the_earlier_sentence() {
        let t = text(&node("Мастер видит график. Мастер видит отпуск."), &terms("мастер"));
        assert_eq!(t, "Приватность и атрибуция. — Мастер видит график. Мастер видит отпуск.");
    }

    #[test]
    fn a_full_stop_inside_a_word_does_not_end_a_sentence() {
        let t = text(&node("Право payroll.view даёт выписку. Ведомость скрыта."), &terms("ведомость скрыта"));
        assert_eq!(t, "Приватность и атрибуция. — …Ведомость скрыта.");
    }

    #[test]
    fn a_successful_commands_output_is_parsed_into_picks() {
        let picked = run("printf 'N-151\\nFR-PAY-20\\n'", "q", &cands());
        assert_eq!(picked, vec!["N-151".to_string(), "FR-PAY-20".to_string()]);
    }
}
