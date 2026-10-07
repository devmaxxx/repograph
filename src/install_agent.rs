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

    /// The file each harness reads for its always-on instructions. Codex reads the repository's
    /// **root** `AGENTS.md` and has no `.codex/AGENTS.md` in its spec, so its stanza is written at
    /// the root whether or not one is already there; Claude Code reads either, and a repository
    /// that already keeps a root `CLAUDE.md` should not grow a second file beside it.
    fn instructions(self) -> &'static str {
        match self { Target::Claude => "CLAUDE.md", Target::Codex => "AGENTS.md" }
    }

    /// Whether the harness reads a settings file inside the repository. Claude Code does
    /// (`.claude/settings.json`); Codex's hooks are `CODEX_HOME`-relative — `"hooks":
    /// "./hooks.json"` — so an entry added from inside one repository would fire in every other
    /// repository on the machine. `agent/codex.md` records how both were read off the CLI.
    fn hooks_are_repository_scoped(self) -> bool { self == Target::Claude }
}

#[derive(Debug, Default)]
pub struct Report { pub written: usize, pub paths: Vec<String>, pub notes: Vec<String> }

/// A text with `{{command}}` resolved to how this repository invokes the binary — `repograph`
/// unless a caller says otherwise, `pnpm exec repograph` in a workspace that installs it as a
/// dependency. Every text that names a command carries it: a line naming the bare binary where
/// PATH has none tells an agent to run something that is not there.
fn with_command(template: &str, command: &str) -> String { template.replace("{{command}}", command) }

/// The hook's one declaration of the command its hints name. Replaced whole rather than through
/// `{{command}}`, because this text lands in JavaScript: a JSON string is a JavaScript string
/// literal, so no quote, backslash or `${` in a command can end the literal or open an expression.
/// The template declares the default, which is what lets the hook's own tests run it as it ships.
const HOOK_COMMAND: &str = "const COMMAND = \"repograph\";";

fn hook(command: &str) -> String {
    HOOK.replacen(HOOK_COMMAND, &format!("const COMMAND = {};", serde_json::Value::from(command)), 1)
}

const BEGIN: &str = "<!-- repograph:begin -->";
const END: &str = "<!-- repograph:end -->";

/// The instructions file with our stanza in it: replacing what is between the markers when they are
/// there, appending when they are not. Everything outside the markers is the repository's own and
/// is never touched — an installer that rewrote CLAUDE.md would be a worse citizen than no
/// installer at all.
pub fn merged_instructions(existing: &str, command: &str) -> String {
    let block = with_command(STANZA, command);
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
///
/// Returns the file and the events an entry running the *retired* `repograph-notice` hook was
/// dropped from. That script is the previous surface and this one replaces it, but it is a file
/// the repository owns and a person put in that list: removing it without a word would be the one
/// silent edit in an installer that otherwise touches nothing it did not write.
pub fn merged_settings(existing: &str, hook_path: &str) -> Result<(String, Vec<String>)> {
    let mut root: serde_json::Value = match existing.trim().is_empty() {
        true => serde_json::json!({}),
        false => serde_json::from_str(existing).context("the settings file is not JSON")?,
    };
    if !root.is_object() { anyhow::bail!("the settings file is not a JSON object"); }
    let hooks = root.as_object_mut().unwrap().entry("hooks").or_insert_with(|| serde_json::json!({}));
    if !hooks.is_object() { anyhow::bail!("`hooks` in the settings file is not an object"); }
    let mut retired = Vec::new();
    for (event, matcher, entry) in hook_entries(hook_path) {
        let mut ours = entry;
        ours.as_object_mut().unwrap().insert("matcher".into(), serde_json::json!(matcher));
        let list = hooks.as_object_mut().unwrap().entry(event).or_insert_with(|| serde_json::json!([]));
        let Some(arr) = list.as_array_mut() else { anyhow::bail!("`hooks.{event}` is not an array") };
        if arr.iter().any(|e| e.to_string().contains("repograph-notice")) { retired.push(event.to_string()); }
        // Ours by the script it runs, never by position: a repository's own entries keep theirs.
        arr.retain(|e| !e.to_string().contains("repograph-hook") && !e.to_string().contains("repograph-notice"));
        arr.push(ours);
    }
    Ok((serde_json::to_string_pretty(&root)? + "\n", retired))
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

/// The block a reader pastes into `~/.codex/hooks.json` to wire the hook on a machine, printed
/// rather than written: that file is machine-level, and a `--repo` command does not get to change
/// how every other repository on the machine behaves. Three events, not four — the Bash
/// interceptor's verdict is being read on Claude Code and is not carried over blind.
pub fn codex_hooks_block(hook_abs: &str) -> String {
    let entry = |matcher: &str, timeout: u64| serde_json::json!({
        "matcher": matcher,
        "hooks": [{ "type": "command", "command": format!("node '{hook_abs}'"), "timeout": timeout }]
    });
    serde_json::to_string_pretty(&serde_json::json!({
        "hooks": {
            "SessionStart": [entry("startup|resume|clear|compact", 10)],
            "SubagentStart": [entry(".*", 5)],
            "PostToolUse": [entry("apply_patch|Edit|Write", 10)],
        }
    })).unwrap_or_default()
}

/// Installs the surface for one harness under `root`. Returns what it actually wrote: a second run
/// writes nothing, which is how a caller can tell an upgrade from a no-op.
pub fn install(root: &Path, target: Target, command: &str) -> Result<Report> {
    let mut report = Report::default();
    let base: PathBuf = root.join(target.dir());
    let hook_file = base.join("hooks").join("repograph-hook.mjs");
    write_if_changed(&hook_file, &hook(command), &mut report)?;
    write_if_changed(&base.join("hooks").join("rule.txt"), &with_command(RULE, command), &mut report)?;
    write_if_changed(&base.join("skills").join("repo-query").join("SKILL.md"), &with_command(SKILL, command), &mut report)?;
    if target == Target::Claude {
        write_if_changed(&base.join("agents").join("repo-scout.md"), &with_command(SCOUT, command), &mut report)?;
    }

    // A path the harness can resolve from wherever it runs the hook. Claude Code exports the
    // project directory; Codex runs with the repository as its working directory and exports
    // nothing, so the path is relative there.
    let hook_path = match target {
        Target::Claude => "$CLAUDE_PROJECT_DIR/.claude/hooks/repograph-hook.mjs".to_string(),
        Target::Codex => ".codex/hooks/repograph-hook.mjs".to_string(),
    };
    if target.hooks_are_repository_scoped() {
        let settings = base.join("settings.json");
        let existing = std::fs::read_to_string(&settings).unwrap_or_default();
        let (merged, retired) = merged_settings(&existing, &hook_path)?;
        if !retired.is_empty() {
            report.notes.push(format!(
                "removed the retired `repograph-notice` hook from {} — this hook replaces it; \
                 the script itself is still on disk", retired.join(", ")));
        }
        write_if_changed(&settings, &merged, &mut report)?;
    }

    // Codex's is the repository root or nowhere; Claude Code reads either, and a repository that
    // already keeps a root `CLAUDE.md` should not grow a second file beside it.
    let root_instructions = root.join(target.instructions());
    let instructions = match target == Target::Codex || root_instructions.exists() {
        true => root_instructions,
        false => base.join(target.instructions()),
    };
    let existing = std::fs::read_to_string(&instructions).unwrap_or_default();
    write_if_changed(&instructions, &merged_instructions(&existing, command), &mut report)?;
    Ok(report)
}

/// The git events after which the tree can have moved by a pull's worth of files.
const GIT_HOOKS: [&str; 2] = ["post-merge", "post-checkout"];
const GIT_BEGIN: &str = "# repograph:begin";
const GIT_END: &str = "# repograph:end";

/// The block each git hook runs. `update --detach` starts the refresh and returns, so neither a
/// pull nor a checkout waits for it, and a binary missing from PATH costs the hook nothing.
/// `$3` is post-checkout's flag, `0` for a file checkout that moves nothing worth a refresh;
/// post-merge passes no third argument.
fn git_block(command: &str) -> String {
    format!("{GIT_BEGIN}\n\
        # Brings repograph's index in line with what this pull or checkout moved, in the background.\n\
        if [ \"$3\" != \"0\" ]; then {command} update --detach >/dev/null 2>&1 || true; fi\n\
        {GIT_END}\n")
}

/// A hook script with our block in it: replaced between the markers when they are there, and
/// otherwise appended after whatever the repository's own hook does, which keeps running first.
/// A hook that was not there starts as a POSIX shell script, which Git for Windows runs as well.
pub fn merged_git_hook(existing: &str, command: &str) -> String {
    let block = git_block(command);
    match (existing.find(GIT_BEGIN), existing.find(GIT_END)) {
        (Some(a), Some(b)) if b > a => {
            let tail = existing[b + GIT_END.len()..].strip_prefix('\n').unwrap_or(&existing[b + GIT_END.len()..]);
            format!("{}{block}{tail}", &existing[..a])
        }
        _ if existing.trim().is_empty() => format!("#!/bin/sh\n{block}"),
        _ => {
            let sep = if existing.ends_with('\n') { "" } else { "\n" };
            format!("{existing}{sep}{block}")
        }
    }
}

/// Writes the `post-merge` and `post-checkout` hooks into the directory git itself runs hooks
/// from — `core.hooksPath` when a hook manager set one, the worktree's common `hooks/` otherwise.
/// Outside a git repository, or without git on PATH, it writes nothing and says so.
pub fn install_git_hooks(root: &Path, command: &str) -> Result<Report> {
    let mut report = Report::default();
    let out = std::process::Command::new("git").arg("-C").arg(root).args(["rev-parse", "--git-path", "hooks"]).output();
    let dir = match out {
        Ok(o) if o.status.success() => root.join(String::from_utf8_lossy(&o.stdout).trim()),
        _ => {
            report.notes.push("not a git repository, or no git on PATH: the post-merge and post-checkout hooks were not written".into());
            return Ok(report);
        }
    };
    if hooks_path_is_shared(root) {
        report.notes.push("core.hooksPath is set outside this repository, so its hooks run in every repository that shares it: the post-merge and post-checkout hooks were not written".into());
        return Ok(report);
    }
    for name in GIT_HOOKS {
        let path = dir.join(name);
        let existing = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => {
                report.notes.push(format!("{name}: the hook already there could not be read as text ({e}), left alone"));
                continue;
            }
        };
        if !is_shell_hook(&existing) {
            report.notes.push(format!("{name}: the hook already there is not a shell script, left alone; run `{command} update --detach` from it"));
            continue;
        }
        let before = report.written;
        write_if_changed(&path, &merged_git_hook(&existing, command), &mut report)?;
        if report.written > before {
            make_executable(&path)?;
            if !existing.trim().is_empty() && !existing.contains(GIT_BEGIN) {
                report.notes.push(format!("{name}: appended to the hook already there, which still runs first"));
            }
        }
    }
    Ok(report)
}

/// A hook whose interpreter is named and is not a shell would be corrupted by a shell block
/// appended to it.
fn is_shell_hook(existing: &str) -> bool {
    match existing.strip_prefix("#!") {
        Some(rest) => rest.lines().next().is_some_and(|line| line.contains("sh")),
        None => true,
    }
}

/// Whether `core.hooksPath` comes from a global or system config rather than this repository's
/// own: a hook written there runs in every repository that reads it, and would start an index in
/// ones that never asked for one.
fn hooks_path_is_shared(root: &Path) -> bool {
    let get = |local: bool| {
        let mut cmd = std::process::Command::new("git");
        cmd.arg("-C").arg(root).arg("config");
        if local { cmd.arg("--local"); }
        cmd.args(["--get", "core.hooksPath"]).output().ok()
            .filter(|o| o.status.success())
            .is_some_and(|o| !o.stdout.iter().all(u8::is_ascii_whitespace))
    };
    get(false) && !get(true)
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut p = std::fs::metadata(path)?.permissions();
    p.set_mode(p.mode() | 0o755);
    std::fs::set_permissions(path, p).with_context(|| format!("chmod {}", path.display()))
}

/// Git for Windows runs a hook by its shebang and has no executable bit to ask for.
#[cfg(windows)]
fn make_executable(_: &Path) -> Result<()> { Ok(()) }

/// Writes `enrich_languages` into the repository's `repograph.toml` from the documents this root
/// holds, and returns the list it wrote — or `None` where the key was already named, the file
/// could not be parsed, or the documents named no language at all. Nothing is written in any of
/// those cases, so a second run is a no-op like the rest of the install.
///
/// Here rather than in `build` because `Config` refuses a key it does not know: the moment the
/// line exists, every binary older than it fails to read that repository at all. `install-agent`
/// is the one command a person runs on purpose when they upgrade, which makes it the one place
/// the file may grow a key.
pub fn set_languages(root: &Path, cfg: &crate::config::Config) -> Result<Option<Vec<String>>> {
    let path = root.join("repograph.toml");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    // A file this binary cannot parse is a file whose owner is mid-edit or on another version;
    // appending a line to it would bury the error under a second one.
    let Ok(named) = toml::from_str::<toml::Table>(&existing) else { return Ok(None) };
    if named.contains_key("enrich_languages") { return Ok(None); }
    let entries = crate::walk::walk(root, cfg, &crate::walk::Manifest::default())?;
    let texts = entries.iter()
        .filter(|e| e.kind == crate::walk::FileKind::Doc)
        .filter_map(|e| std::fs::read_to_string(root.join(&e.rel)).ok());
    let languages = crate::enrich::languages_of(texts);
    if languages.is_empty() { return Ok(None); }
    let value = toml::Value::Array(languages.iter().map(|l| toml::Value::String(l.clone())).collect());
    let mut text = existing;
    if !text.is_empty() && !text.ends_with('\n') { text.push('\n'); }
    text.push_str(&format!(
        "# Languages `enrich` writes questions in, set by `install-agent` from the documents it found.\n\
         enrich_languages = {value}\n"));
    std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
    Ok(Some(languages))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

    fn git_repo() -> tempfile::TempDir {
        let dir = root();
        let ok = std::process::Command::new("git").arg("-C").arg(dir.path()).args(["init", "-q"])
            .env_remove("GIT_DIR").env_remove("GIT_WORK_TREE").status().unwrap().success();
        assert!(ok, "git init");
        dir
    }

    #[test]
    fn the_git_hooks_are_written_once_and_start_a_detached_update() {
        let dir = git_repo();
        let first = install_git_hooks(dir.path(), "repograph").unwrap();
        assert_eq!(first.written, 2, "{first:?}");
        for name in GIT_HOOKS {
            let text = std::fs::read_to_string(dir.path().join(".git/hooks").join(name)).unwrap();
            assert!(text.starts_with("#!/bin/sh\n"), "{text}");
            assert!(text.contains("repograph update --detach"), "{text}");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(dir.path().join(".git/hooks").join(name)).unwrap().permissions().mode();
                assert!(mode & 0o111 != 0, "{name} is executable");
            }
        }
        assert_eq!(install_git_hooks(dir.path(), "repograph").unwrap().written, 0, "a second install is a no-op");
    }

    #[test]
    fn a_hook_already_there_keeps_its_own_lines_and_gets_the_block_once() {
        let mine = "#!/bin/sh\nnpx lefthook run post-merge\n";
        let once = merged_git_hook(mine, "repograph");
        assert!(once.starts_with(mine), "{once}");
        assert_eq!(merged_git_hook(&once, "repograph"), once, "re-merging is a no-op");
        let upgraded = merged_git_hook(&once, "pnpm exec repograph");
        assert!(upgraded.starts_with(mine) && upgraded.contains("pnpm exec repograph update --detach"), "{upgraded}");
        assert_eq!(upgraded.matches(GIT_BEGIN).count(), 1);
    }

    #[test]
    fn a_hook_in_another_language_is_left_alone() {
        let dir = git_repo();
        let hook = dir.path().join(".git/hooks/post-merge");
        std::fs::write(&hook, "#!/usr/bin/env node\nconsole.log(1)\n").unwrap();
        let r = install_git_hooks(dir.path(), "repograph").unwrap();
        assert_eq!(r.written, 1, "only post-checkout: {r:?}");
        assert_eq!(std::fs::read_to_string(&hook).unwrap(), "#!/usr/bin/env node\nconsole.log(1)\n");
    }

    #[test]
    fn a_hooks_path_a_hook_manager_set_is_where_the_hooks_go() {
        let dir = git_repo();
        let ok = std::process::Command::new("git").arg("-C").arg(dir.path()).args(["config", "core.hooksPath", ".githooks"])
            .env_remove("GIT_DIR").status().unwrap().success();
        assert!(ok);
        install_git_hooks(dir.path(), "repograph").unwrap();
        assert!(dir.path().join(".githooks/post-merge").exists());
        assert!(!dir.path().join(".git/hooks/post-merge").exists());
    }

    #[test]
    fn outside_a_git_repository_nothing_is_written_and_it_says_so() {
        let dir = root();
        let r = install_git_hooks(dir.path(), "repograph").unwrap();
        assert_eq!(r.written, 0);
        assert_eq!(r.notes.len(), 1, "{r:?}");
    }

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
        let r = install(dir.path(), Target::Claude, "repograph").unwrap();
        let v: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join(".claude/settings.json")).unwrap()).unwrap();
        let arr = v["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(arr.len(), 1, "the old entry and the retired notice hook both went: {arr:?}");
        // The retired hook is a file the repository owns and a person listed: it goes, and the
        // install says so rather than leaving them to find out from a hook that stopped firing.
        assert!(r.notes.iter().any(|n| n.contains("repograph-notice") && n.contains("SessionStart")),
                "{:?}", r.notes);
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

    /// Read off the CLI rather than assumed — `agent/codex.md` records how. Codex reads the
    /// repository's root `AGENTS.md`, so a `.codex/AGENTS.md` is a file nothing would ever open;
    /// and its hooks live at `CODEX_HOME`, so no repository-scoped hooks file is written at all.
    #[test]
    fn codex_gets_its_stanza_at_the_root_and_no_repository_hooks_file() {
        let dir = root();
        install(dir.path(), Target::Codex, "repograph").unwrap();
        assert!(dir.path().join(".codex/hooks/repograph-hook.mjs").exists(), "something to point at");
        assert!(dir.path().join(".codex/skills/repo-query/SKILL.md").exists());
        let stanza = std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
        assert!(stanza.contains("repograph ask <words>"), "{stanza}");
        assert!(!dir.path().join(".codex/AGENTS.md").exists(), "nothing reads that path");
        assert!(!dir.path().join(".codex/hooks.json").exists(), "hooks are CODEX_HOME-relative");
        assert!(!dir.path().join(".codex/agents").exists(), "the scout is a Claude Code definition");
    }

    #[test]
    fn the_codex_block_names_three_events_and_not_the_interceptor() {
        let v: serde_json::Value = serde_json::from_str(&codex_hooks_block("/abs/hook.mjs")).unwrap();
        let events: Vec<&String> = v["hooks"].as_object().unwrap().keys().collect();
        assert_eq!(events.len(), 3, "{events:?}");
        assert!(v["hooks"]["PreToolUse"].is_null(), "the interceptor is not carried over blind");
        assert!(v["hooks"]["SessionStart"][0]["hooks"][0]["command"].as_str().unwrap().contains("/abs/hook.mjs"));
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

    /// `--command` is how this repository runs the binary, and a pnpm workspace has no bare
    /// `repograph` on PATH: a file that still says the bare name tells an agent to run nothing.
    /// So every text either harness installs carries it, and not only the stanza.
    #[test]
    fn the_command_reaches_every_file_either_harness_installs() {
        for (target, texts) in [
            (Target::Claude, &[".claude/hooks/rule.txt", ".claude/skills/repo-query/SKILL.md",
                               ".claude/agents/repo-scout.md", ".claude/CLAUDE.md"][..]),
            (Target::Codex, &[".codex/hooks/rule.txt", ".codex/skills/repo-query/SKILL.md", "AGENTS.md"][..]),
        ] {
            let dir = root();
            install(dir.path(), target, "pnpm exec repograph").unwrap();
            for rel in texts {
                let text = std::fs::read_to_string(dir.path().join(rel)).unwrap();
                assert!(text.contains("`pnpm exec repograph "), "{rel} names the command: {text}");
                assert!(!text.contains("`repograph ") && !text.contains("{{command}}"), "{rel} still names the bare binary: {text}");
            }
            let script = std::fs::read_to_string(dir.path().join(target.dir()).join("hooks/repograph-hook.mjs")).unwrap();
            assert!(script.contains("\nconst COMMAND = \"pnpm exec repograph\";\n"), "{target:?}: the hook's hints read one declaration");
        }
    }

    /// A command is whatever a person typed, and the hook is JavaScript: it goes in as one JSON
    /// string, which JavaScript reads as a string literal, so nothing in it can run.
    #[test]
    fn the_hook_takes_the_command_as_one_json_string_and_the_default_as_it_ships() {
        assert_eq!(HOOK.matches(HOOK_COMMAND).count(), 1, "the declaration the installer replaces is in the hook once");
        assert_eq!(hook("repograph"), HOOK, "the default installs the template byte for byte");
        let script = hook(r#"pnpm exec "re'po\graph" `x` ${y}"#);
        assert!(script.contains(r#"const COMMAND = "pnpm exec \"re'po\\graph\" `x` ${y}";"#), "the command is one string literal");
        assert_eq!(script.matches("const COMMAND = ").count(), 1);
    }

    /// An upgrade is a re-run, and a re-run under the same command writes nothing — for a command
    /// other than the default, and for both harnesses.
    #[test]
    fn a_second_install_under_a_named_command_writes_nothing_for_either_harness() {
        for target in [Target::Claude, Target::Codex] {
            let dir = root();
            std::fs::write(dir.path().join(target.instructions()), "# House rules\n").unwrap();
            assert!(install(dir.path(), target, "pnpm exec repograph").unwrap().written > 0);
            let again = install(dir.path(), target, "pnpm exec repograph").unwrap();
            assert_eq!(again.written, 0, "{target:?}: {again:?}");
        }
    }

    /// Measured with prettier 3.9.6, under no config and under a consumer's: the one change it made
    /// to the 0.5.0 stanza was a blank line after the begin marker, because a comment is a block of
    /// its own. A stanza carrying that line is left alone by the formatter and by a re-run of this.
    #[test]
    fn the_stanza_is_what_prettier_writes_and_a_rerun_keeps_it() {
        let block = with_command(STANZA, "repograph");
        assert!(block.starts_with("<!-- repograph:begin -->\n\n## repograph\n"), "{block}");

        let fresh = merged_instructions("# House rules\n", "repograph");
        assert_eq!(fresh, format!("# House rules\n\n{block}"));
        assert_eq!(merged_instructions(&fresh, "repograph"), fresh, "a re-run is a fixed point");

        // A block 0.5.0 wrote gains the line; the text after the end marker stays the repository's.
        let old = "# House rules\n\n<!-- repograph:begin -->\n## repograph\nold\n<!-- repograph:end -->\n\n## After\n";
        let upgraded = merged_instructions(old, "repograph");
        assert_eq!(upgraded, format!("# House rules\n\n{}\n\n## After\n", block.trim_end()));
        assert_eq!(merged_instructions(&upgraded, "repograph"), upgraded);
    }

    /// The languages a corpus is written in decide what `enrich` writes its questions in, and the
    /// key that says so is written here rather than by a writer: a binary one version older
    /// refuses a `repograph.toml` carrying a key it does not know.
    #[test]
    fn the_languages_of_the_documents_are_written_once_and_never_again() {
        let dir = root();
        std::fs::write(dir.path().join("ru.md"),
            "# Отмена записи\n\nКлиент не пришёл на приём, и администратор отменил визит по политике салона.\n").unwrap();
        std::fs::write(dir.path().join("en.md"), "# Cancellation\n\nThe client did not show up.\n").unwrap();
        let cfg = crate::config::Config::default();
        assert_eq!(set_languages(dir.path(), &cfg).unwrap(), Some(vec!["Russian".to_string(), "English".to_string()]));
        let written = std::fs::read_to_string(dir.path().join("repograph.toml")).unwrap();
        assert!(written.ends_with("enrich_languages = [\"Russian\", \"English\"]\n"), "{written}");
        assert_eq!(set_languages(dir.path(), &cfg).unwrap(), None, "the key is there: a second run says nothing");
        assert_eq!(std::fs::read_to_string(dir.path().join("repograph.toml")).unwrap(), written);
    }

    /// A repository that chose its own languages keeps them, whatever this run reads off the disk.
    #[test]
    fn a_repograph_toml_that_already_names_the_key_is_untouched() {
        let dir = root();
        std::fs::write(dir.path().join("ru.md"), "# Отмена\n\nКлиент не пришёл.\n").unwrap();
        let before = "enrich_languages = [\"English\"]\nskip = []\n";
        std::fs::write(dir.path().join("repograph.toml"), before).unwrap();
        assert_eq!(set_languages(dir.path(), &crate::config::Config::default()).unwrap(), None);
        assert_eq!(std::fs::read_to_string(dir.path().join("repograph.toml")).unwrap(), before);
    }

    /// The file is the one thing this reads before it appends to it; a file it cannot read is left
    /// for its owner rather than given a line on the end.
    #[test]
    fn a_repograph_toml_that_does_not_parse_is_left_alone() {
        let dir = root();
        std::fs::write(dir.path().join("ru.md"), "# Отмена\n\nКлиент не пришёл.\n").unwrap();
        std::fs::write(dir.path().join("repograph.toml"), "skip = [\n").unwrap();
        assert_eq!(set_languages(dir.path(), &crate::config::Config::default()).unwrap(), None);
        assert_eq!(std::fs::read_to_string(dir.path().join("repograph.toml")).unwrap(), "skip = [\n");
    }
}
