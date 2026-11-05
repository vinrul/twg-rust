use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

/// Bounded least-recently-used cache for tile materials and decoded images.
///
/// Tiles are keyed by `(layer, tile)` so switching base layers and back keeps
/// decoded images and materials alive instead of downloading them again. Bevy
/// unloads an image once every strong handle is dropped, so the cache owns a
/// handle while an entry is present.
pub struct LruCache<K, V> {
    entries: HashMap<K, V>,
    order: VecDeque<K>,
    limit: usize,
}

impl<K: Eq + Hash + Clone, V> LruCache<K, V> {
    pub fn new(limit: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            limit: limit.max(1),
        }
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        if !self.entries.contains_key(key) {
            return None;
        }
        self.touch(key);
        self.entries.get(key)
    }

    pub fn insert(&mut self, key: K, value: V) {
        if self.entries.insert(key.clone(), value).is_some() {
            self.touch(&key);
        } else {
            self.order.push_back(key);
        }
        while self.entries.len() > self.limit {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }

    fn touch(&mut self, key: &K) {
        if let Some(index) = self.order.iter().position(|entry| entry == key) {
            self.order.remove(index);
        }
        self.order.push_back(key.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::LruCache;

    #[test]
    fn evicts_least_recently_used_entry() {
        let mut cache = LruCache::new(2);
        cache.insert(0, "zero");
        cache.insert(1, "one");
        assert_eq!(cache.get(&0), Some(&"zero"));

        cache.insert(2, "two");

        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&0), Some(&"zero"));
        assert_eq!(cache.get(&2), Some(&"two"));
    }

    #[test]
    fn replacing_a_key_refreshes_its_position() {
        let mut cache = LruCache::new(2);
        cache.insert(0, "zero");
        cache.insert(1, "one");
        cache.insert(0, "zero updated");

        cache.insert(2, "two");

        assert_eq!(cache.get(&0), Some(&"zero updated"));
        assert_eq!(cache.get(&1), None);
    }
}
