/// Reciprocal rank fusion: each list votes 1/(k + rank) for its members.
pub fn rrf(lists: &[Vec<String>], k: f32) -> Vec<(String, f32)> {
    let mut score: std::collections::HashMap<&str, f32> = std::collections::HashMap::new();
    for list in lists {
        for (rank, id) in list.iter().enumerate() {
            *score.entry(id.as_str()).or_default() += 1.0 / (k + rank as f32 + 1.0);
        }
    }
    let mut out: Vec<(String, f32)> = score.into_iter().map(|(i, s)| (i.to_string(), s)).collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_in_both_lists_outranks_a_single_top_hit() {
        let a = vec!["x".to_string(), "y".to_string()];
        let b = vec!["z".to_string(), "y".to_string()];
        let r = rrf(&[a, b], 60.0);
        assert_eq!(r[0].0, "y");
        assert_eq!(r[1].0, "x");
        assert_eq!(r[2].0, "z");
    }

    #[test]
    fn single_list_keeps_its_order() {
        let r = rrf(&[vec!["a".into(), "b".into(), "c".into()]], 60.0);
        assert_eq!(r.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["a", "b", "c"]);
    }
}
