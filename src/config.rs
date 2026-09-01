use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub doc_globs: Vec<String>,
    pub code_globs: Vec<String>,
    pub skip: Vec<String>,
    pub id_families: Vec<String>,
    pub milestone_families: Vec<String>,
    pub registries: Vec<String>,
}

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
                "NFR-PH", "NFR-MKT", "NFR-MIG", "NFR-PAY", "NFR-DM", "NFR-RPT", "NFR-WEB", "NFR-SVC", "NFR-STAFF", "NFR",
                "AC-DM", "AC-VIS", "INV", "ADR", "OD", "OQ", "N", "R", "M", "W", "D", "G",
                "PREP", "CAL", "OR", "MON", "SEAM", "SG", "IDEA",
            ]),
            milestone_families: s(&["BE", "FE", "PLAT", "SYNC", "OPS", "AI"]),
            registries: s(&["docs/constitution.yaml"]),
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
}
