use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub doc_globs: Vec<String>,
    pub code_globs: Vec<String>,
    pub skip: Vec<String>,
    pub id_families: Vec<String>,
    pub milestone_families: Vec<String>,
    pub registries: Vec<String>,
    /// Reads a prompt on stdin and writes `id<TAB>question` lines; `repograph enrich` runs it.
    pub enrich_command: String,
    /// Reads a prompt on stdin and writes the chosen ids one per line; `ask --rerank` runs it.
    pub rerank_command: String,
    /// Directory holding `model.onnx` and `tokenizer.json` for `ask --rerank-local`; empty
    /// means `$HOME/.cache/repograph/reranker`.
    pub reranker_dir: String,
    /// Hub id of the model the vectors are written with. `build`, `update`, `enrich`, `embed`
    /// and `watch` embed with it and rewrite the index whole when the store holds another
    /// model's rows; `ask`, `bench` and `dump` open the model the store records instead, so a
    /// store keeps answering with what wrote it whatever this says today. The large model is
    /// the default: it read paraphrase 22/30 against the small one's 15/30 on the fixture and
    /// held-out 103 → 119 of 400 (p = 0.00086); the latency, memory and download that choice
    /// costs are measured in docs/bench/2026-09-05-dev-cases-results.md.
    /// `intfloat/multilingual-e5-small` is the cheap way back, and the model the floors were
    /// set with.
    pub embed_model: String,
}

// Headless Claude Code with thinking off: the same answers, 4-5× faster and cheaper. Haiku
// writes the questions as well as any model; picking seeds from a 200-deep pool it does not
// (11/14 against sonnet's 14/14), so the reranker defaults to sonnet.
const ENRICH_COMMAND: &str = "MAX_THINKING_TOKENS=0 claude -p --model haiku --output-format text --tools \"\" --setting-sources \"\" --no-session-persistence";
const RERANK_COMMAND: &str = "MAX_THINKING_TOKENS=0 claude -p --model sonnet --output-format text --tools \"\" --setting-sources \"\" --no-session-persistence";

impl Default for Config {
    fn default() -> Self {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        Config {
            doc_globs: s(&["**/*.md"]),
            code_globs: s(&["**/*.ts", "**/*.tsx"]),
            skip: s(&["**/node_modules/**", "**/dist/**", "**/TRACKER.md", "graphify-out/**", ".repograph/**"]),
            // The strict families measured in beauty-crm's census; ambiguous
            // one-letter families (B1, C11, S3) collide with prose and are left out.
            id_families: s(&[
                "FR-DM", "FR-CAL", "FR-VIS", "FR-PAY", "FR-PH", "FR-SEC", "FR-APP", "FR-MKT",
                "FR-AI", "FR-CRM", "FR-SHELL", "FR-TOOL", "FR-SVC", "FR-LIFE", "FR-WH", "FR-RPT",
                "FR-MIG", "FR-WEB", "FR-OPS", "FR-STAFF",
                // Bare "NFR" trails its ten "NFR-<SUB>" siblings, but alternation order can't
                // cause a wrong match here: every branch requires digits right after its own
                // literal suffix, so "NFR-PH-01" can only ever satisfy the "NFR-PH" arm.
                "NFR-PH", "NFR-MKT", "NFR-MIG", "NFR-PAY", "NFR-DM", "NFR-RPT", "NFR-WEB", "NFR-SVC", "NFR-STAFF", "NFR",
                "AC-DM", "AC-VIS", "INV", "ADR", "OD", "OQ", "N", "R", "M", "W", "D", "G",
                "PREP", "CAL", "OR", "MON", "SEAM", "SG", "IDEA",
            ]),
            milestone_families: s(&["BE", "FE", "PLAT", "SYNC", "OPS", "AI", "MOB"]),
            registries: s(&["docs/constitution.yaml"]),
            enrich_command: ENRICH_COMMAND.into(),
            rerank_command: RERANK_COMMAND.into(),
            reranker_dir: String::new(),
            embed_model: crate::index::embed::DEFAULT_MODEL.into(),
        }
    }
}

impl Config {
    pub fn load(repo: &Path) -> Result<Config> {
        let path = repo.join("repograph.toml");
        if !path.exists() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config::load(dir.path()).unwrap();
        assert!(cfg.id_families.contains(&"FR-PAY".to_string()));
        assert_eq!(cfg.doc_globs, vec!["**/*.md".to_string()]);
    }

    #[test]
    fn partial_file_overrides_only_named_keys() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("repograph.toml"), "skip = [\"docs/**/TRACKER.md\"]\n").unwrap();
        let cfg = Config::load(dir.path()).unwrap();
        assert_eq!(cfg.skip, vec!["docs/**/TRACKER.md".to_string()]);
        assert!(cfg.id_families.contains(&"INV".to_string()));
    }

    #[test]
    fn an_unknown_key_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("repograph.toml"), "bogus_key = 1\n").unwrap();
        let err = Config::load(dir.path()).unwrap_err().to_string();
        assert!(err.contains("repograph.toml"), "{err}");
    }

    #[test]
    fn malformed_toml_errors_naming_the_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("repograph.toml"), "skip = [\n").unwrap();
        let err = Config::load(dir.path()).unwrap_err().to_string();
        assert!(err.contains("repograph.toml"), "{err}");
    }

    #[test]
    fn the_embed_model_defaults_to_the_large_e5_and_reads_from_the_file() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Config::load(dir.path()).unwrap().embed_model, "intfloat/multilingual-e5-large");
        std::fs::write(dir.path().join("repograph.toml"), "embed_model = \"intfloat/multilingual-e5-small\"\n").unwrap();
        assert_eq!(Config::load(dir.path()).unwrap().embed_model, "intfloat/multilingual-e5-small");
    }

    #[test]
    fn overriding_the_enrich_command_leaves_the_rerank_command_and_lists_at_their_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("repograph.toml"), "enrich_command = \"echo hi\"\n").unwrap();
        let cfg = Config::load(dir.path()).unwrap();
        assert_eq!(cfg.enrich_command, "echo hi");
        assert_eq!(cfg.rerank_command, Config::default().rerank_command);
        assert_eq!(cfg.doc_globs, Config::default().doc_globs);
    }
}
