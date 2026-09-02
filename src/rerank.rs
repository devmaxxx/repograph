//! `ask --rerank`: the configured model command picks the seeds from the deep fused candidate
//! list. Costs tokens per question (≈4.8k on the bench corpus); nothing runs without the flag.

use crate::enrich::run_command;

/// How far down the fused list the model looks. Measured on the bench corpus with questions
/// enriched: 80 deep catches 9/14 paraphrases, 100 deep 11/14; the three left are one target
/// no retriever surfaces, one at rank 137 and one mislabelled case.
pub const DEPTH: usize = 100;

pub fn prompt(question: &str, candidates: &[(String, String)]) -> String {
    let mut p = format!(
        "Question: \"{question}\"\nBelow are candidate entries as `id<TAB>title`. Output the ids of up to \
         5 entries most relevant to the question, one per line, most relevant first. Nothing but ids.\n\n");
    for (id, label) in candidates {
        p.push_str(&format!("{id}\t{}\n", label.chars().take(100).collect::<String>()));
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

/// A failing command is reported and answered with an empty pick, so `ask` falls back to the
/// fused order instead of failing the question.
pub fn run(command: &str, question: &str, candidates: &[(String, String)]) -> Vec<String> {
    match run_command(command, &prompt(question, candidates)) {
        Ok(out) => parse(&out, candidates),
        Err(e) => {
            eprintln!("rerank: {e:#}; answering from the fused order");
            Vec::new()
        }
    }
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
    fn prompt_lists_every_candidate_with_a_clipped_title() {
        let long = "x".repeat(150);
        let p = prompt("why", &[("A-1".into(), long)]);
        assert!(p.contains("\"why\""));
        assert!(p.contains(&format!("A-1\t{}\n", "x".repeat(100))));
        assert!(!p.contains(&"x".repeat(101)));
    }

    #[test]
    fn a_failing_command_yields_no_picks() {
        assert!(run("exit 3", "q", &cands()).is_empty());
    }
}
