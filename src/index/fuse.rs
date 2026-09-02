/// Round-robin over the lists: rank r of every list precedes rank r + 1 of any of them, and an
/// id keeps its first position. Reciprocal rank fusion was measured to bury a retriever's second
/// hit under ids the two lists merely agree on — five of six paraphrase hits sat at dense rank 2,
/// and one of them never reached the seeds.
pub fn interleave(lists: &[Vec<String>]) -> Vec<(String, f32)> {
    let mut out: Vec<(String, f32)> = Vec::new();
    let depth = lists.iter().map(Vec::len).max().unwrap_or(0);
    for rank in 0..depth {
        for id in lists.iter().filter_map(|l| l.get(rank)) {
            if !out.iter().any(|(seen, _)| seen == id) {
                out.push((id.clone(), 1.0 / (rank as f32 + 1.0)));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }

    #[test]
    fn every_list_places_its_first_hit_before_any_second_hit() {
        let r = interleave(&[ids(&["x", "y"]), ids(&["z", "y"])]);
        assert_eq!(r.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["x", "z", "y"]);
    }

    #[test]
    fn an_id_keeps_its_first_position() {
        let r = interleave(&[ids(&["a", "b"]), ids(&["b", "a"])]);
        assert_eq!(r.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
        assert_eq!(r[1].1, 1.0);
    }

    #[test]
    fn empty_and_short_lists_neither_panic_nor_leave_gaps() {
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(interleave(&[]).is_empty());
        assert!(interleave(&[Vec::new(), Vec::new()]).is_empty());
        let out = interleave(&[l(&["a"]), Vec::new(), l(&["b", "c", "d"])]);
        assert_eq!(out.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["a", "b", "c", "d"]);
        assert_eq!(out[3].1, 1.0 / 3.0);
    }

    #[test]
    fn single_list_keeps_its_order() {
        let r = interleave(&[ids(&["a", "b", "c"])]);
        assert_eq!(r.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["a", "b", "c"]);
    }

    #[test]
    fn three_lists_interleave_rank_by_rank_in_list_order() {
        let r = interleave(&[ids(&["a1", "a2"]), ids(&["b1", "b2"]), ids(&["c1", "c2"])]);
        assert_eq!(r.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["a1", "b1", "c1", "a2", "b2", "c2"]);
    }

    #[test]
    fn scores_are_the_reciprocal_of_rank_for_a_deeper_list() {
        let r = interleave(&[ids(&["a", "b", "c", "d"])]);
        assert_eq!(r[2].1, 1.0 / 3.0);
        assert_eq!(r[3].1, 0.25);
    }

    #[test]
    fn a_duplicate_id_within_one_list_is_kept_only_at_its_first_rank() {
        let r = interleave(&[ids(&["a", "b", "a"])]);
        assert_eq!(r.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
        assert_eq!(r[0].1, 1.0);
    }
}
