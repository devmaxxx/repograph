use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdHit {
    pub id: String,
    pub start: usize,
}

#[derive(Clone)]
pub struct IdMatcher {
    single: Regex,
    range: Regex,
    slash: Regex,
}

fn bounded(text: &str, start: usize, end: usize) -> bool {
    let tail = |b: u8| b.is_ascii_alphanumeric() || b == b'-';
    let left_ok = start == 0 || !tail(text.as_bytes()[start - 1]);
    let right_ok = end == text.len() || !tail(text.as_bytes()[end]);
    left_ok && right_ok
}

/// The families as one alternation. An empty list has to be written as a branch that cannot
/// match: left as the empty alternation it matches the empty string, so `(?:)-M\d{2}` reads a
/// bare `-M01` anywhere in prose as an id and `verify` reports it as undeclared. A repository
/// with no milestone families is ordinary — auto mode reaches one on most corpora.
fn alternation(families: &[String]) -> String {
    match families.is_empty() {
        true => r"[^\s\S]".to_string(),
        false => families.iter().map(|f| regex::escape(f)).collect::<Vec<_>>().join("|"),
    }
}

impl IdMatcher {
    pub fn new(families: &[String], milestone_families: &[String]) -> IdMatcher {
        let fam = alternation(families);
        let ms = alternation(milestone_families);
        let single = Regex::new(&format!(r"(?:{fam})-\d{{1,4}}|(?:{ms})-M\d{{2}}")).unwrap();
        // `FR-RPT-42…48`, `R-1601…R-1603`, `INV-11..13`, `N-1—N-3`
        let range = Regex::new(&format!(
            r"((?:{fam})-)(\d{{1,4}})\s*(?:…|\.\.|—|–)\s*(?:(?:{fam})-)?(\d{{1,4}})"
        )).unwrap();
        let slash = Regex::new(&format!(r"((?:{fam})-)(\d{{1,4}}(?:/\d{{1,4}})+)")).unwrap();
        IdMatcher { single, range, slash }
    }

    /// The bare id alternation, for callers that embed it in a larger expression.
    pub fn single_pattern(&self) -> String { self.single.as_str().to_string() }

    pub fn is_id(&self, token: &str) -> bool {
        self.single.find(token).map(|m| m.start() == 0 && m.end() == token.len()).unwrap_or(false)
    }

    pub fn find_all(&self, text: &str) -> Vec<IdHit> {
        let mut hits: Vec<IdHit> = Vec::new();
        let mut covered: Vec<(usize, usize)> = Vec::new();
        for c in self.range.captures_iter(text) {
            let whole = c.get(0).unwrap();
            if !bounded(text, whole.start(), whole.end()) {
                continue;
            }
            let prefix = c.get(1).unwrap().as_str();
            let (a, b): (u32, u32) = (c[2].parse().unwrap(), c[3].parse().unwrap());
            // A range wider than 50 is a typo or a year, not a list of requirements.
            if a <= b && b - a <= 50 {
                let width = c[2].len();
                for n in a..=b {
                    hits.push(IdHit { id: format!("{prefix}{n:0width$}"), start: whole.start() });
                }
                covered.push((whole.start(), whole.end()));
            }
        }
        for c in self.slash.captures_iter(text) {
            let whole = c.get(0).unwrap();
            if !bounded(text, whole.start(), whole.end()) {
                continue;
            }
            let prefix = c.get(1).unwrap().as_str();
            for n in c[2].split('/') {
                hits.push(IdHit { id: format!("{prefix}{n}"), start: whole.start() });
            }
            covered.push((whole.start(), whole.end()));
        }
        for m in self.single.find_iter(text) {
            let inside = covered.iter().any(|(s, e)| m.start() >= *s && m.end() <= *e);
            if inside || !bounded(text, m.start(), m.end()) {
                continue;
            }
            hits.push(IdHit { id: m.as_str().to_string(), start: m.start() });
        }
        hits.sort_by(|a, b| a.start.cmp(&b.start).then(a.id.cmp(&b.id)));
        hits.dedup();
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m() -> IdMatcher {
        crate::families::test_matcher()
    }

    fn ids(t: &str) -> Vec<String> {
        m().find_all(t).into_iter().map(|h| h.id).collect()
    }

    #[test]
    fn plain_ids_in_prose() {
        assert_eq!(ids("см. FR-PAY-22 и INV-01, ADR-0007"), vec!["FR-PAY-22", "INV-01", "ADR-0007"]);
    }

    #[test]
    fn ambiguous_families_do_not_match() {
        assert!(ids("вариант B1, C11 и S3; I-015 тоже").is_empty());
    }

    #[test]
    fn word_boundaries_hold() {
        assert!(ids("XFR-PAY-22Y").is_empty());
        assert_eq!(ids("(FR-PAY-22)."), vec!["FR-PAY-22"]);
    }

    #[test]
    fn ranges_expand_with_and_without_prefix() {
        assert_eq!(ids("FR-RPT-42…48"), vec!["FR-RPT-42", "FR-RPT-43", "FR-RPT-44", "FR-RPT-45", "FR-RPT-46", "FR-RPT-47", "FR-RPT-48"]);
        assert_eq!(ids("R-1601…R-1603"), vec!["R-1601", "R-1602", "R-1603"]);
    }

    #[test]
    fn slash_lists_expand() {
        assert_eq!(ids("INV-11/12/20"), vec!["INV-11", "INV-12", "INV-20"]);
    }

    #[test]
    fn milestones_and_tasks() {
        assert_eq!(ids("BE-M02 T25, PLAT-M01"), vec!["BE-M02", "PLAT-M01"]);
    }

    #[test]
    fn offsets_are_byte_offsets_into_utf8() {
        let hits = m().find_all("Правило FR-PAY-22");
        assert_eq!(hits[0].start, "Правило ".len());
    }

    #[test]
    fn is_id_is_exact() {
        assert!(m().is_id("FR-PAY-22"));
        assert!(!m().is_id("FR-PAY-22."));
        assert!(!m().is_id("asGrosze"));
    }

    #[test]
    fn nfr_and_fr_ops_do_not_collide_with_bare_families() {
        assert_eq!(ids("N-151 и NFR-PH-01"), vec!["N-151", "NFR-PH-01"]);
        assert_eq!(ids("OPS-M02 vs FR-OPS-12"), vec!["OPS-M02", "FR-OPS-12"]);
    }

    #[test]
    fn a_descending_range_is_not_expanded_only_its_start_is_a_single_hit() {
        assert_eq!(ids("FR-RPT-48…42"), vec!["FR-RPT-48"]);
    }

    #[test]
    fn a_range_wider_than_fifty_is_left_unexpanded() {
        assert_eq!(ids("FR-RPT-1…100"), vec!["FR-RPT-1"]);
    }

    #[test]
    fn slash_list_elements_keep_their_own_written_digit_width() {
        assert_eq!(ids("INV-01/02"), vec!["INV-01", "INV-02"]);
    }

    #[test]
    fn the_same_id_repeated_in_prose_yields_two_hits_at_their_own_offsets() {
        let hits = m().find_all("FR-PAY-22 ... позже снова FR-PAY-22");
        assert_eq!(hits.len(), 2);
        assert_ne!(hits[0].start, hits[1].start);
        assert!(hits.iter().all(|h| h.id == "FR-PAY-22"));
    }

    #[test]
    fn an_id_spanning_the_entire_text_satisfies_both_boundary_checks() {
        assert_eq!(ids("FR-PAY-22"), vec!["FR-PAY-22"]);
    }

    #[test]
    fn empty_family_lists_match_nothing_rather_than_every_bare_suffix() {
        let m = IdMatcher::new(&[], &[]);
        assert!(m.find_all("see -M01 and -12").is_empty());
        assert!(m.find_all("-11…13, -1/2").is_empty());
        assert!(!m.is_id("-12"));
    }
}
