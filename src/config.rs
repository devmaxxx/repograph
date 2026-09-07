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
    /// `{model}` in it is replaced by `enrich_model`.
    pub enrich_command: String,
    /// Reads a prompt on stdin and writes the chosen ids one per line; `ask --rerank` runs it.
    /// `{model}` in it is replaced by `rerank_model`.
    pub rerank_command: String,
    /// What goes in `enrich_command`'s `{model}`. Any name the command understands; nothing here
    /// assumes a vendor. `REPOGRAPH_ENRICH_MODEL` overrides it for one run.
    pub enrich_model: String,
    /// What goes in `rerank_command`'s `{model}`. `REPOGRAPH_RERANK_MODEL` overrides it.
    pub rerank_model: String,
    /// Directory holding `model.onnx` and `tokenizer.json` for `ask --rerank-local`; empty
    /// means `~/.cache/repograph/reranker`.
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

// Headless Claude Code with thinking off: the same answers, 4-5× faster and cheaper. `{model}`
// is where the configured model goes, so choosing one is a word in a config file rather than a
// rewritten command line; a command that names no `{model}` simply ignores the setting.
const ENRICH_COMMAND: &str = "MAX_THINKING_TOKENS=0 claude -p --model {model} --output-format text --tools \"\" --setting-sources \"\" --no-session-persistence";
const RERANK_COMMAND: &str = "MAX_THINKING_TOKENS=0 claude -p --model {model} --output-format text --tools \"\" --setting-sources \"\" --no-session-persistence";
// The cheap model writes a node's questions as well as any (paraphrase 15/30 against a stronger
// model's 17/30, and 5/9 of the developer suite's `rule` answers against its 2/9 — the register
// its questions are written in matters more than the model, and
// docs/bench/2026-09-06-g14-second-diagnostic.md measures that). Picking seeds from a 200-deep
// pool it does not: 11/14 against 14/14, so the reranker defaults to the stronger one.
const ENRICH_MODEL: &str = "haiku";
const RERANK_MODEL: &str = "sonnet";
const MODEL_SLOT: &str = "{model}";

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
            enrich_command: ENRICH_COMMAND.replace(MODEL_SLOT, ENRICH_MODEL),
            rerank_command: RERANK_COMMAND.replace(MODEL_SLOT, RERANK_MODEL),
            enrich_model: ENRICH_MODEL.into(),
            rerank_model: RERANK_MODEL.into(),
            reranker_dir: String::new(),
            embed_model: crate::index::embed::DEFAULT_MODEL.into(),
        }
    }
}

/// The settings that describe the machine rather than the corpus: which command runs a model, and
/// which model it runs. A global file may set these and nothing else. The corpus-shaped settings —
/// the globs, the id families, and above all `embed_model` — are deliberately unreadable from
/// there: one global line would otherwise rewrite every repository's vectors under a model nobody
/// chose for that repository, which is the one mistake this store's design spends effort avoiding.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Machine {
    enrich_command: Option<String>,
    rerank_command: Option<String>,
    enrich_model: Option<String>,
    rerank_model: Option<String>,
    reranker_dir: Option<String>,
}

/// `$REPOGRAPH_CONFIG`, else `$XDG_CONFIG_HOME/repograph/config.toml`, else
/// `~/.config/repograph/config.toml`. `None` when the environment names no home at all, which
/// is a machine with no global settings rather than an error.
fn machine_path() -> Option<std::path::PathBuf> {
    let named = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    if let Some(p) = named("REPOGRAPH_CONFIG") {
        return Some(std::path::PathBuf::from(p));
    }
    let base = match named("XDG_CONFIG_HOME") {
        Some(x) => std::path::PathBuf::from(x),
        None => std::env::home_dir()?.join(".config"),
    };
    Some(base.join("repograph").join("config.toml"))
}

impl Config {
    /// The project's `repograph.toml` over the machine's global file over the built-in defaults,
    /// key by key, with `REPOGRAPH_ENRICH_MODEL` and `REPOGRAPH_RERANK_MODEL` over all three. A
    /// key the project names wins even when it names the built-in value: what the file says is
    /// what the repository asked for.
    pub fn load(repo: &Path) -> Result<Config> {
        let path = repo.join("repograph.toml");
        let text = match path.exists() {
            true => Some(std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?),
            false => None,
        };
        let mut cfg = match &text {
            Some(t) => toml::from_str::<Config>(t).with_context(|| format!("parse {}", path.display()))?,
            None => Config::default(),
        };
        // Which keys the project actually wrote, as opposed to the ones serde filled from
        // `Default`: without this the machine layer could never win, every key always being set.
        let named: toml::Table = match &text {
            Some(t) => toml::from_str(t).unwrap_or_default(),
            None => toml::Table::new(),
        };
        let machine = Self::machine()?;

        let layer = |key: &str, from_machine: Option<String>, field: &mut String| {
            if !named.contains_key(key) {
                if let Some(v) = from_machine {
                    *field = v;
                }
            }
        };
        layer("reranker_dir", machine.reranker_dir, &mut cfg.reranker_dir);
        layer("enrich_model", machine.enrich_model, &mut cfg.enrich_model);
        layer("rerank_model", machine.rerank_model, &mut cfg.rerank_model);

        // The command is resolved from its template rather than from `Default`, whose copy already
        // has the default model substituted: a project that sets only `enrich_model` must still get
        // its model into the built-in command.
        let template = |key: &str, from_machine: Option<String>, builtin: &str| -> String {
            match (named.contains_key(key), from_machine) {
                (true, _) => cfg_str(&named, key, builtin),
                (false, Some(v)) => v,
                (false, None) => builtin.to_string(),
            }
        };
        let enrich_template = template("enrich_command", machine.enrich_command, ENRICH_COMMAND);
        let rerank_template = template("rerank_command", machine.rerank_command, RERANK_COMMAND);

        if let Ok(m) = std::env::var("REPOGRAPH_ENRICH_MODEL") {
            if !m.is_empty() { cfg.enrich_model = m; }
        }
        if let Ok(m) = std::env::var("REPOGRAPH_RERANK_MODEL") {
            if !m.is_empty() { cfg.rerank_model = m; }
        }
        cfg.enrich_command = enrich_template.replace(MODEL_SLOT, &cfg.enrich_model);
        cfg.rerank_command = rerank_template.replace(MODEL_SLOT, &cfg.rerank_model);
        Ok(cfg)
    }

    /// The global file, or an empty layer when there is none. A malformed or unknown-key global
    /// file is an error naming the file: a setting silently ignored on every repository at once is
    /// worse than a refusal.
    fn machine() -> Result<Machine> {
        let Some(path) = machine_path() else { return Ok(Machine::default()) };
        if !path.exists() {
            return Ok(Machine::default());
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
    }
}

/// A string the project file named, falling back when it holds something that is not a string.
fn cfg_str(table: &toml::Table, key: &str, fallback: &str) -> String {
    table.get(key).and_then(|v| v.as_str()).unwrap_or(fallback).to_string()
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

    /// The layering is read out of the environment, and cargo runs these on one process: without
    /// a lock two of them race on `REPOGRAPH_CONFIG` and the loser reads the other's file.
    static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Runs `f` with a global config file holding `machine`, and nothing else set.
    fn with_machine<T>(machine: Option<&str>, f: impl FnOnce() -> T) -> T {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        if let Some(text) = machine {
            std::fs::write(&path, text).unwrap();
        }
        // Safety: the lock above makes this the only thread touching the environment.
        unsafe {
            std::env::set_var("REPOGRAPH_CONFIG", &path);
            std::env::remove_var("REPOGRAPH_ENRICH_MODEL");
            std::env::remove_var("REPOGRAPH_RERANK_MODEL");
        }
        let out = f();
        unsafe { std::env::remove_var("REPOGRAPH_CONFIG") };
        out
    }

    #[test]
    fn the_model_named_in_the_project_reaches_the_built_in_command() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("repograph.toml"), "enrich_model = \"some-other-model\"\n").unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert!(cfg.enrich_command.contains("--model some-other-model"), "{}", cfg.enrich_command);
            assert!(!cfg.enrich_command.contains(MODEL_SLOT));
            // The reranker is untouched by the enricher's model.
            assert_eq!(cfg.rerank_command, Config::default().rerank_command);
        });
    }

    #[test]
    fn the_machine_file_fills_only_what_the_project_left_unnamed() {
        with_machine(Some("enrich_model = \"from-machine\"\nrerank_model = \"also-machine\"\n"), || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("repograph.toml"), "rerank_model = \"from-project\"\n").unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert_eq!(cfg.enrich_model, "from-machine");
            assert_eq!(cfg.rerank_model, "from-project");
            assert!(cfg.rerank_command.contains("--model from-project"), "{}", cfg.rerank_command);
        });
    }

    #[test]
    fn a_project_key_wins_even_when_it_names_the_built_in_value() {
        with_machine(Some("enrich_model = \"from-machine\"\n"), || {
            let dir = tempfile::tempdir().unwrap();
            let named = format!("enrich_model = \"{ENRICH_MODEL}\"\n");
            std::fs::write(dir.path().join("repograph.toml"), named).unwrap();
            assert_eq!(Config::load(dir.path()).unwrap().enrich_model, ENRICH_MODEL);
        });
    }

    #[test]
    fn the_environment_beats_the_project_and_the_machine() {
        with_machine(Some("enrich_model = \"from-machine\"\n"), || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("repograph.toml"), "enrich_model = \"from-project\"\n").unwrap();
            unsafe { std::env::set_var("REPOGRAPH_ENRICH_MODEL", "from-env") };
            let cfg = Config::load(dir.path()).unwrap();
            unsafe { std::env::remove_var("REPOGRAPH_ENRICH_MODEL") };
            assert_eq!(cfg.enrich_model, "from-env");
            assert!(cfg.enrich_command.contains("--model from-env"), "{}", cfg.enrich_command);
        });
    }

    #[test]
    fn the_machine_file_may_not_set_a_corpus_key() {
        with_machine(Some("embed_model = \"whatever\"\n"), || {
            let dir = tempfile::tempdir().unwrap();
            let err = Config::load(dir.path()).unwrap_err().to_string();
            assert!(err.contains("config.toml"), "{err}");
        });
    }

    #[test]
    fn a_command_naming_no_slot_is_left_exactly_as_written() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("repograph.toml"),
                "enrich_command = \"my-runner --go\"\nenrich_model = \"ignored-here\"\n").unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert_eq!(cfg.enrich_command, "my-runner --go");
            assert_eq!(cfg.enrich_model, "ignored-here");
        });
    }

    #[test]
    fn a_store_with_no_project_file_still_reads_the_machine_file() {
        with_machine(Some("enrich_model = \"from-machine\"\n"), || {
            let dir = tempfile::tempdir().unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert_eq!(cfg.enrich_model, "from-machine");
            assert!(cfg.enrich_command.contains("--model from-machine"), "{}", cfg.enrich_command);
            assert_eq!(cfg.doc_globs, Config::default().doc_globs, "the corpus keys stay built-in");
        });
    }
}
