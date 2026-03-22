use crate::config_manager::UserDictData;
use arc_swap::ArcSwap;
use std::sync::Arc;

pub fn update_mru(
    history: &Arc<ArcSwap<UserDictData>>,
    profile: &str,
    key: &str,
    word: &str,
    sort_by_count: bool,
) -> Vec<(String, u32)> {
    let mut result = Vec::new();
    history.rcu(|hist| {
        let mut clone = (**hist).clone();
        let entries = clone
            .entry(profile.to_string())
            .or_default()
            .entry(key.to_string())
            .or_default();

        if let Some(pos) = entries.iter().position(|(w, _)| w == word) {
            if sort_by_count {
                entries[pos].1 += 1;
            } else {
                let old = entries[pos].1;
                entries.remove(pos);
                entries.insert(0, (word.to_string(), old + 1));
            }
        } else {
            if sort_by_count {
                entries.push((word.to_string(), 1));
            } else {
                entries.insert(0, (word.to_string(), 1));
            }
        }

        if sort_by_count {
            entries.sort_by(|a, b| b.1.cmp(&a.1));
        } else if entries.len() > 10 {
            entries.truncate(10);
        }

        result = entries.clone();
        Arc::new(clone)
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_dict_data_type() {
        let data: UserDictData = UserDictData::new();
        assert!(data.is_empty());
    }

    #[test]
    fn test_user_dict_data_insert() {
        let mut data: UserDictData = UserDictData::new();
        data.entry("profile1".to_string())
            .or_default()
            .entry("ni".to_string())
            .or_default()
            .push(("你".to_string(), 1));

        assert_eq!(data.len(), 1);
        let profile_data = data.get("profile1").unwrap();
        assert_eq!(profile_data.len(), 1);
    }
}
