//! `repograph install-agent`: writes the agent-facing surface into a consuming repository, for
//! Claude Code or for Codex. The texts are embedded with `include_str!` rather than copied from a
//! checkout, so a binary and the surface it installs cannot disagree about what the tool can do.
//!
//! Idempotent by construction: a second run rewrites the same bytes and reports nothing written.
//! The settings file is merged rather than replaced — a repository's own hooks survive, and this
//! tool's entries are replaced by command match, which is what makes an upgrade a re-run.
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

const HOOK: &str = include_str!("../agent/hook.mjs");
const RULE: &str = include_str!("../agent/rule.txt");
const STANZA: &str = include_str!("../agent/stanza.md");
const SKILL: &str = include_str!("../agent/skill/SKILL.md");
const SCOUT: &str = include_str!("../agent/subagent.md");

/// Where the surface is being installed. The two harnesses read different files for the same four
/// jobs, and the only thing that differs between them is which path each job goes to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target { Claude, Codex }

impl Target {
    fn dir(self) -> &'static str {
        match self { Target::Claude => ".claude", Target::Codex => ".codex" }
    }

    /// The file each harness reads for its always-on instructions.
    fn instructions(self) -> &'static str {
        match self { Target::Claude => "CLAUDE.md", Target::Codex => "AGENTS.md" }
    }
}

#[derive(Debug, Default)]
pub struct Report { pub written: usize, pub paths: Vec<String> }

/// The stanza between its markers, with `{{command}}` resolved to how this repository invokes the
/// binary — `repograph` unless a caller says otherwise, `pnpm exec repograph` in a workspace that
/// installs it as a dependency.
fn stanza(command: &str) -> String { STANZA.replace("{{command}}", command) }

const BEGIN: &str = "<!-- repograph:begin -->";
const END: &str = "<!-- repograph:end -->";

/// The instructions file with our stanza in it: replacing what is between the markers when they are
/// there, appending when they are not. Everything outside the markers is the repository's own and
/// is never touched — an installer that rewrote CLAUDE.md would be a worse citizen than no
/// installer at all.
pub fn merged_instructions(existing: &str, command: &str) -> String {
    let block = stanza(command);
    match (existing.find(BEGIN), existing.find(END)) {
        (Some(a), Some(b)) if b > a => {
            let mut out = String::with_capacity(existing.len() + block.len());
            out.push_str(&existing[..a]);
            out.push_str(block.trim_end());
            out.push_str(&existing[b + END.len()..]);
            out
        }
        _ => {
            let mut out = existing.to_string();
            if !out.is_empty() && !out.ends_with('\n') { out.push('\n'); }
            if !out.is_empty() { out.push('\n'); }
            out.push_str(&block);
            out
        }
    }
}

/// The four hook entries this tool owns, as the harness's settings file spells them.
fn hook_entries(hook_path: &str) -> Vec<(&'static str, &'static str, serde_json::Value)> {
    let entry = |timeout: u64| serde_json::json!({
        "hooks": [{ "type": "command", "command": format!("node \"{hook_path}\""), "timeout": timeout }]
    });
    vec![
        ("SessionStart", "startup|resume|clear|compact", entry(10)),
        ("SubagentStart", ".*", entry(5)),
        ("PreToolUse", "Bash|Grep|Glob|Agent", entry(10)),
        ("PostToolUse", "Edit|Write|MultiEdit", entry(10)),
    ]
}

/// The settings file with this tool's entries in it and everything else left alone. An entry whose
/// command names our hook script is replaced rather than added, so a re-run after the hook changed
/// is an upgrade and not a second copy firing twice per event.
pub fn merged_settings(existing: &str, hook_path: &str) -> Result<String> {
    let mut root: serde_json::Value = match existing.trim().is_empty() {
        true => serde_json::json!({}),
        false => serde_json::from_str(existing).context("the settings file is not JSON")?,
    };
    if !root.is_object() { anyhow::bail!("the settings file is not a JSON object"); }
    let hooks = root.as_object_mut().unwrap().entry("hooks").or_insert_with(|| serde_json::json!({}));
    if !hooks.is_object() { anyhow::bail!("`hooks` in the settings file is not an object"); }
    for (event, matcher, entry) in hook_entries(hook_path) {
        let mut ours = entry;
        ours.as_object_mut().unwrap().insert("matcher".into(), serde_json::json!(matcher));
        let list = hooks.as_object_mut().unwrap().entry(event).or_insert_with(|| serde_json::json!([]));
        let Some(arr) = list.as_array_mut() else { anyhow::bail!("`hooks.{event}` is not an array") };
        // Ours by the script it runs, never by position: a repository's own entries keep theirs.
        arr.retain(|e| !e.to_string().contains("repograph-hook") && !e.to_string().contains("repograph-notice"));
        arr.push(ours);
    }
    Ok(serde_json::to_string_pretty(&root)? + "\n")
}

/// Writes `text` at `path` unless the same bytes are already there, and says which it did.
fn write_if_changed(path: &Path, text: &str, report: &mut Report) -> Result<()> {
    if std::fs::read_to_string(path).is_ok_and(|old| old == text) { return Ok(()); }
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
    report.written += 1;
    report.paths.push(path.display().to_string());
    Ok(())
}

/// Installs the surface for one harness under `root`. Returns what it actually wrote: a second run
/// writes nothing, which is how a caller can tell an upgrade from a no-op.
pub fn install(root: &Path, target: Target, command: &str) -> Result<Report> {
    let mut report = Report::default();
    let base: PathBuf = root.join(target.dir());
    let hook_file = base.join("hooks").join("repograph-hook.mjs");
    write_if_changed(&hook_file, HOOK, &mut report)?;
    write_if_changed(&base.join("hooks").join("rule.txt"), RULE, &mut report)?;
    write_if_changed(&base.join("skills").join("repo-query").join("SKILL.md"), SKILL, &mut report)?;
    if target == Target::Claude {
        write_if_changed(&base.join("agents").join("repo-scout.md"), SCOUT, &mut report)?;
    }

    // A path the harness can resolve from wherever it runs the hook. Claude Code exports the
    // project directory; Codex runs with the repository as its working directory and exports
    // nothing, so the path is relative there.
    let hook_path = match target {
        Target::Claude => "$CLAUDE_PROJECT_DIR/.claude/hooks/repograph-hook.mjs".to_string(),
        Target::Codex => ".codex/hooks/repograph-hook.mjs".to_string(),
    };
    let settings = base.join(match target { Target::Claude => "settings.json", Target::Codex => "hooks.json" });
    let existing = std::fs::read_to_string(&settings).unwrap_or_default();
    let merged = merged_settings(&existing, &hook_path)?;
    write_if_changed(&settings, &merged, &mut report)?;

    // The root instructions file if the repository has one, else the harness's own.
    let root_instructions = root.join(target.instructions());
    let instructions = match root_instructions.exists() {
        true => root_instructions,
        false => base.join(target.instructions()),
    };
    let existing = std::fs::read_to_string(&instructions).unwrap_or_default();
    write_if_changed(&instructions, &merged_instructions(&existing, command), &mut report)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

    #[test]
    fn installing_twice_writes_the_same_tree_and_says_it_changed_nothing() {
        let dir = root();
        let first = install(dir.path(), Target::Claude, "repograph").unwrap();
        assert!(first.written > 0, "{first:?}");
        for rel in [".claude/hooks/repograph-hook.mjs", ".claude/skills/repo-query/SKILL.md",
                    ".claude/agents/repo-scout.md", ".claude/settings.json", ".claude/CLAUDE.md"] {
            assert!(dir.path().join(rel).exists(), "{rel} was not written");
        }
        let second = install(dir.path(), Target::Claude, "repograph").unwrap();
        assert_eq!(second.written, 0, "a second install is a no-op: {second:?}");
    }

    /// An installer that rewrote a repository's settings file would be a worse citizen than no
    /// installer: the hooks a team wrote are the reason the file exists.
    #[test]
    fn an_existing_settings_file_keeps_its_own_hooks() {
        let dir = root();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join(".claude/settings.json"),
            r#"{"model":"opus","hooks":{"Stop":[{"hooks":[{"type":"command","command":"echo mine"}]}]}}"#).unwrap();
        install(dir.path(), Target::Claude, "repograph").unwrap();
        let v: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join(".claude/settings.json")).unwrap()).unwrap();
        assert_eq!(v["model"], "opus", "a key that is not ours is untouched");
        assert!(v["hooks"]["Stop"].is_array(), "the repository's own hook survived");
        assert_eq!(v["hooks"]["SessionStart"].as_array().unwrap().len(), 1);
        assert_eq!(v["hooks"]["PreToolUse"][0]["matcher"], "Bash|Grep|Glob|Agent");
    }

    /// The upgrade case: an older entry of ours is replaced, not joined by a second copy that
    /// would fire the hook twice on every event.
    #[test]
    fn an_older_entry_of_ours_is_replaced_rather_than_doubled() {
        let dir = root();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join(".claude/settings.json"), r#"{"hooks":{"SessionStart":[
            {"matcher":"startup","hooks":[{"type":"command","command":"node /old/path/repograph-hook.mjs"}]},
            {"matcher":"startup","hooks":[{"type":"command","command":"node .claude/hooks/repograph-notice.mjs"}]}
        ]}}"#).unwrap();
        install(dir.path(), Target::Claude, "repograph").unwrap();
        let v: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join(".claude/settings.json")).unwrap()).unwrap();
        let arr = v["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(arr.len(), 1, "the old entry and the retired notice hook both went: {arr:?}");
        assert!(arr[0]["hooks"][0]["command"].as_str().unwrap().contains("CLAUDE_PROJECT_DIR"));
    }

    #[test]
    fn the_stanza_lands_in_a_root_instructions_file_when_there_is_one_and_replaces_only_itself() {
        let dir = root();
        std::fs::write(dir.path().join("CLAUDE.md"), "# House rules\n\nRun the tests.\n").unwrap();
        install(dir.path(), Target::Claude, "pnpm exec repograph").unwrap();
        let text = std::fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        assert!(text.starts_with("# House rules\n\nRun the tests.\n"), "{text}");
        assert!(text.contains("pnpm exec repograph ask <words>"), "{text}");
        assert!(!dir.path().join(".claude/CLAUDE.md").exists(), "the root file was the one to use");

        // A re-install under a different invocation name replaces the block and nothing else.
        install(dir.path(), Target::Claude, "repograph").unwrap();
        let text = std::fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        assert!(text.starts_with("# House rules\n\nRun the tests.\n"), "{text}");
        assert!(!text.contains("pnpm exec"), "the old block went: {text}");
        assert_eq!(text.matches(BEGIN).count(), 1, "one block, not two: {text}");
    }

    #[test]
    fn codex_gets_the_same_texts_under_its_own_names() {
        let dir = root();
        install(dir.path(), Target::Codex, "repograph").unwrap();
        assert!(dir.path().join(".codex/hooks/repograph-hook.mjs").exists());
        assert!(dir.path().join(".codex/hooks.json").exists());
        let stanza = std::fs::read_to_string(dir.path().join(".codex/AGENTS.md")).unwrap();
        assert!(stanza.contains("repograph ask <words>"), "{stanza}");
        assert!(!dir.path().join(".codex/agents").exists(), "the scout is a Claude Code definition");
        let v: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join(".codex/hooks.json")).unwrap()).unwrap();
        assert!(v["hooks"]["SessionStart"][0]["hooks"][0]["command"].as_str().unwrap().contains(".codex/hooks"));
    }

    #[test]
    fn a_settings_file_that_is_not_json_is_refused_by_name_rather_than_overwritten() {
        let dir = root();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join(".claude/settings.json"), "not json").unwrap();
        let err = install(dir.path(), Target::Claude, "repograph").unwrap_err().to_string();
        assert!(err.contains("not JSON"), "{err}");
        assert_eq!(std::fs::read_to_string(dir.path().join(".claude/settings.json")).unwrap(), "not json");
    }
}
