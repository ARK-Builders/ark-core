use std::collections::HashMap;

pub struct CacheEntry<K, V> {
    value: V,
    size: usize,
    top: Option<K>,
    bottom: Option<K>,
}

pub struct LRUCache<K, V> {
    entries: HashMap<K, CacheEntry<K, V>>,
    head: Option<K>,
    tail: Option<K>,
    memory_used: usize,
    max_memory: usize,
}

impl<K, V> LRUCache<K, V>
where
    K: std::hash::Hash + Eq + Clone,
    V: Clone,
{
    /// Creates a new cache instance with the specified maximum memory capacity
    pub fn new(max_memory: usize) -> Self {
        LRUCache {
            entries: HashMap::new(),
            head: None,
            tail: None,
            memory_used: 0,
            max_memory,
        }
    }

    /// Adds an entry to the cache, evicting LRU items if necessary. Returns existing value if replaced.
    pub fn push(&mut self, key: K, value: V, size: usize) -> Option<V> {
        if size > self.max_memory {
            return None;
        }

        // can we return all key popped
        // TODO: return all keys popped later
        while self.memory_used + size > self.max_memory {
            self.pop();
        }

        let entry = CacheEntry {
            value,
            size,
            top: None,
            bottom: self.head.clone(),
        };

        self.memory_used += size;

        // Update the topious head's top pointer
        if let Some(head_key) = self.head.clone() {
            if let Some(head_entry) = self.entries.get_mut(&head_key) {
                head_entry.top = Some(key.clone());
            }
        } else {
            // If there was no head, this is also the tail
            self.tail = Some(key.clone());
        }

        self.head = Some(key.clone());

        // Return the old value if it existed
        self.entries
            .insert(key, entry)
            .map(|old_entry| old_entry.value)
    }

    /// Removes and returns the least recently used entry's key
    pub fn pop(&mut self) -> Option<K> {
        let tail_key = self.tail.clone()?;
        let entry = self.entries.remove(&tail_key)?;

        self.memory_used -= entry.size;

        // Update tail to point to the topious element
        self.tail = entry.top.clone();

        // If there was a topious element, update its bottom pointer
        if let Some(top_key) = entry.top {
            if let Some(top_entry) = self.entries.get_mut(&top_key) {
                top_entry.bottom = None;
            }
        } else {
            // If there was no topious element, the cache is now empty
            self.head = None;
        }

        Some(tail_key)
    }

    /// Retrieves a value and promotes it to most recently used. Returns None if not found.
    pub fn get(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.entries.get(key) {
            let value = entry.value.clone();
            self.move_to_front(key);
            Some(value)
        } else {
            None
        }
    }

    /// Returns a reference to the cache entry without affecting LRU order
    pub fn peek(&self, key: &K) -> Option<&CacheEntry<K, V>> {
        self.entries.get(key)
    }

    /// Checks if the cache contains a specific key
    pub fn contains(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    /// Clears all entries and resets cache state
    pub fn clear(&mut self) {
        self.entries.clear();
        self.head = None;
        self.tail = None;
        self.memory_used = 0;
    }

    // CHECKED

    fn move_to_front(&mut self, key: &K) {
        if self.head.as_ref().map_or(false, |h| h == key) {
            return;
        }

        let (top_key, bottom_key) = if let Some(entry) = self.entries.get(key) {
            (entry.top.clone(), entry.bottom.clone())
        } else {
            return;
        };

        match (top_key, bottom_key) {
            (Some(top_key), Some(bottom_key)) => {
                if let Some(top_entry) = self.entries.get_mut(&top_key) {
                    top_entry.bottom = Some(bottom_key.clone());
                }
                if let Some(bottom_entry) = self.entries.get_mut(&bottom_key) {
                    bottom_entry.top = Some(top_key);
                }
            }
            (Some(top_key), None) => {
                if let Some(top_entry) = self.entries.get_mut(&top_key) {
                    top_entry.bottom = None;
                }
                self.tail = Some(top_key);
            }
            _ => {}
        }

        if let Some(entry) = self.entries.get_mut(key) {
            entry.top = None;
            entry.bottom = self.head.clone();
        }

        if let Some(head_key) = &self.head {
            if let Some(head_entry) = self.entries.get_mut(head_key) {
                head_entry.top = Some(key.clone());
            }
        }

        self.head = Some(key.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lru_eviction_and_order() {
        let mut cache = LRUCache::new(300);

        // Fill the cache
        assert!(cache.push("key1", 1, 100).is_none());
        assert!(cache.push("key2", 2, 100).is_none());
        assert!(cache.push("key3", 3, 100).is_none());

        // Verify initial state
        assert!(cache.contains(&"key1"));
        assert_eq!(cache.head, Some("key3"));
        assert_eq!(cache.tail, Some("key1"));

        // Access key2 to promote it
        assert_eq!(cache.get(&"key2"), Some(2));
        assert_eq!(cache.head, Some("key2"));
        assert_eq!(cache.tail, Some("key1"));

        // Add key4 which should evict key1
        assert!(cache.push("key4", 4, 100).is_none());
        assert!(!cache.contains(&"key1"));
        assert!(cache.contains(&"key2"));
        assert!(cache.contains(&"key3"));
        assert!(cache.contains(&"key4"));
        assert_eq!(cache.memory_used, 300);

        // Verify new order
        assert_eq!(cache.head, Some("key4"));
        assert_eq!(cache.tail, Some("key3"));

        // Access key3 to promote it
        assert_eq!(cache.get(&"key3"), Some(3));
        assert_eq!(cache.head, Some("key3"));
        assert_eq!(cache.tail, Some("key2"));

        // Add key5 which should evict key2 (oldest after promotions)
        assert!(cache.push("key5", 5, 100).is_none());
        assert!(!cache.contains(&"key2"));
        assert!(cache.contains(&"key3"));
        assert!(cache.contains(&"key4"));
        assert!(cache.contains(&"key5"));
        assert_eq!(cache.memory_used, 300);
    }
}
