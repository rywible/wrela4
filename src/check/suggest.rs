pub fn edit_distance(left: &str, right: &str) -> usize {
    let left_chars = left.chars().collect::<Vec<_>>();
    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    let mut current = vec![0; right_chars.len() + 1];

    for (left_index, left_ch) in left_chars.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_ch) in right_chars.iter().enumerate() {
            let replace_cost = if left_ch == right_ch { 0 } else { 1 };
            let delete = previous[right_index + 1] + 1;
            let insert = current[right_index] + 1;
            let replace = previous[right_index] + replace_cost;
            current[right_index + 1] = delete.min(insert).min(replace);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right_chars.len()]
}

pub fn nearest_name<'a, I>(needle: &str, candidates: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut best: Option<(usize, &'a str)> = None;
    for candidate in candidates {
        let distance = edit_distance(needle, candidate);
        if distance > 2 {
            continue;
        }
        match best {
            Some((best_distance, best_name)) if distance > best_distance => {}
            Some((best_distance, best_name))
                if distance == best_distance && candidate <= best_name => {}
            _ => best = Some((distance, candidate)),
        }
    }
    best.map(|(_, name)| name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_distance_counts_insert_delete_and_replace() {
        assert_eq!(edit_distance("Console", "Consol"), 1);
        assert_eq!(edit_distance("Driver", "Diver"), 1);
        assert_eq!(edit_distance("abc", "xyz"), 3);
    }

    #[test]
    fn nearest_name_uses_distance_then_lexical_order() {
        let candidates = ["Console", "Consola", "Driver"];
        assert_eq!(
            nearest_name("Consol", candidates.iter().copied()),
            Some("Console")
        );
        let sorted = ["Consola", "Console", "Driver"];
        assert_eq!(
            nearest_name("Consol", sorted.iter().copied()),
            Some("Console")
        );
        assert_eq!(nearest_name("zzzz", candidates.iter().copied()), None);
    }
}
