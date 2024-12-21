use std::{
    collections::{HashMap, LinkedList},
    hash::Hash,
    rc::Rc,
};

pub struct LRU<K, V> {
    list: LinkedList<(Rc<K>, Rc<V>)>,
    map: HashMap<Rc<K>, (Rc<V>, usize)>,
    num_items: usize,
    max_items: usize,
}

impl<K: Eq + Hash, V> LRU<K, V> {
    pub fn new(max_items: usize) -> Self {
        Self {
            list: LinkedList::new(),
            map: HashMap::new(),
            num_items: 0,
            max_items,
        }
    }

    /// **Each key must map to exactly one value.** Pushing multiple different value for the same key is undefined behaviour.
    pub fn push(&mut self, key: Rc<K>, value: Rc<V>) -> Option<(Rc<K>, Rc<V>)> {
        if let Some((_, count)) = self.map.get_mut(&key) {
            // value already in LRU
            self.list.push_back((key, value));
            *count += 1;

            return self.maybe_gc();
        }

        // new element inserted
        self.list.push_back((key.clone(), value.clone()));
        self.map.insert(key, (value, 1));
        self.num_items += 1;

        self.maybe_gc()
    }

    #[inline(always)]
    pub fn maybe_gc(&mut self) -> Option<(Rc<K>, Rc<V>)> {
        if self.num_items > self.max_items {
            self.gc()
        } else {
            None
        }
    }

    pub fn gc(&mut self) -> Option<(Rc<K>, Rc<V>)> {
        let mut iterations = 0;

        while iterations < self.list.len() && self.num_items > self.max_items {
            iterations += 1;

            if let Some((key, value)) = self.list.pop_front() {
                if let Some((_, count)) = self.map.get_mut(&key) {
                    if *count > 1 {
                        // multiple references exist in list
                        *count -= 1;
                    } else if Rc::strong_count(&value) > 2 {
                        // a reference exists outside this LRU cache
                        self.list.push_back((key, value));
                    } else {
                        // evict from cache
                        self.map.remove(&key);
                        self.num_items -= 1;
                        // return the evicted pair
                        return Some((key, value));
                    }
                } else {
                    return Some((key, value));
                }
            } else {
                // list is empty
                return None;
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn test_basic_insertion_and_eviction() {
        let mut lru = LRU {
            list: LinkedList::new(),
            map: HashMap::new(),
            num_items: 0,
            max_items: 3,
        };

        // Insert elements
        assert!(lru.push(Rc::new(1), Rc::new("a")).is_none());
        assert!(lru.push(Rc::new(2), Rc::new("b")).is_none());
        assert!(lru.push(Rc::new(3), Rc::new("c")).is_none());

        // Check no eviction yet
        assert_eq!(lru.num_items, 3);

        // Insert one more element, triggering eviction
        let evicted = lru.push(Rc::new(4), Rc::new("d")).unwrap();
        assert_eq!(*evicted.0, 1); // LRU element "1" should be evicted

        // Check internal state
        assert_eq!(lru.num_items, 3);
        assert!(lru.map.contains_key(&Rc::new(2)));
        assert!(lru.map.contains_key(&Rc::new(3)));
        assert!(lru.map.contains_key(&Rc::new(4)));
    }

    #[test]
    fn test_reinsertion_of_existing_key() {
        let mut lru = LRU {
            list: LinkedList::new(),
            map: HashMap::new(),
            num_items: 0,
            max_items: 3,
        };

        let key = Rc::new(1);
        let value = Rc::new("a");

        lru.push(key.clone(), value.clone());
        lru.push(Rc::new(2), Rc::new("b"));
        lru.push(Rc::new(3), Rc::new("c"));

        // Reinserting existing key should update its position
        lru.push(key.clone(), value.clone());

        // Insert another element to trigger eviction
        let evicted = lru.push(Rc::new(4), Rc::new("d")).unwrap();
        assert_eq!(*evicted.0, 2); // Element "2" should be evicted as "1" was recently used

        // Check internal state
        assert_eq!(lru.num_items, 3);
        assert!(lru.map.contains_key(&key));
        assert!(lru.map.contains_key(&Rc::new(3)));
        assert!(lru.map.contains_key(&Rc::new(4)));
    }

    #[test]
    fn test_gc_with_reference_counts() {
        let mut lru = LRU {
            list: LinkedList::new(),
            map: HashMap::new(),
            num_items: 0,
            max_items: 2,
        };

        let key1 = Rc::new(1);
        let value1 = Rc::new("a");
        lru.push(key1.clone(), value1.clone());

        let key2 = Rc::new(2);
        let value2 = Rc::new("b");
        lru.push(key2.clone(), value2.clone());

        // External reference to key1 and value1
        let _key1_ref = Rc::clone(&key1);
        let _value1_ref = Rc::clone(&value1);

        let key3 = Rc::new(3);
        let value3 = Rc::new("c");

        // Adding new element should trigger GC
        let evicted = lru.push(key3.clone(), value3.clone());
        assert!(evicted.is_none()); // No eviction as key1/value1 has external refs

        // Drop external reference to value2
        drop(value2);

        // Add another element, this should evict key2/value2
        let evicted = lru.push(Rc::new(4), Rc::new("d")).unwrap();
        assert_eq!(*evicted.0, 2); // Key "2" should be evicted

        // Check internal state
        assert_eq!(lru.num_items, 3);
        assert!(lru.map.contains_key(&Rc::new(1)));
        assert!(lru.map.contains_key(&Rc::new(3)));
        assert!(lru.map.contains_key(&Rc::new(4)));
    }

    #[test]
    fn test_handling_empty_lru() {
        let mut lru = LRU {
            list: LinkedList::new(),
            map: HashMap::new(),
            num_items: 0,
            max_items: 2,
        };

        // Try GC on empty LRU
        let evicted = lru.gc();
        assert!(evicted.is_none());

        // Insert and remove an element
        lru.push(Rc::new(1), Rc::new("a"));
        let evicted = lru.push(Rc::new(2), Rc::new("b"));
        assert!(evicted.is_none());

        // Remove all elements
        let _ = lru.push(Rc::new(3), Rc::new("c"));
        let _ = lru.push(Rc::new(4), Rc::new("d"));
        assert_eq!(lru.num_items, 2);
    }

    #[test]
    fn test_push_new_item() {
        let mut lru = LRU::new(2);
        let key = Rc::new(1);
        let value = Rc::new("a");

        let evicted = lru.push(key.clone(), value.clone());
        assert!(evicted.is_none());
        assert_eq!(lru.num_items, 1);
        assert!(lru.map.contains_key(&key));
    }

    #[test]
    fn test_eviction() {
        let mut lru = LRU::new(2);
        let key1 = Rc::new(1);
        let key2 = Rc::new(2);
        let key3 = Rc::new(3);
        let value1 = Rc::new("a");
        let value2 = Rc::new("b");
        let value3 = Rc::new("c");

        lru.push(key1, value1);
        lru.push(key2.clone(), value2.clone());
        let evicted = lru.push(key3.clone(), value3.clone());

        assert!(evicted.is_some());
        let (evicted_key, evicted_value) = evicted.unwrap();
        assert_eq!(*evicted_key, 1);
        assert_eq!(*evicted_value, "a");
        assert_eq!(lru.num_items, 2);
        assert!(lru.map.contains_key(&key2));
        assert!(lru.map.contains_key(&key3));
    }

    #[test]
    fn test_reference_count_handling() {
        let mut lru = LRU::new(1);
        let key: Rc<i32> = Rc::new(1);
        let value = Rc::new("a");

        {
            let key_ref = key.clone();
            let value_ref = value.clone();
            lru.push(key_ref, value_ref);
            assert_eq!(Rc::strong_count(&key), 3);
            assert_eq!(Rc::strong_count(&value), 3);
        }

        let evicted = lru.push(key.clone(), value.clone());
        assert!(evicted.is_none());
        assert_eq!(Rc::strong_count(&key), 4);
        assert_eq!(Rc::strong_count(&value), 4);
    }

    #[test]
    fn test_gc_on_empty_list() {
        let mut lru = LRU::<i32, Box<dyn std::any::Any>>::new(1);
        let evicted = lru.gc();
        assert!(evicted.is_none());
    }

    fn setup_lru(max_items: usize) -> LRU<i32, i32> {
        LRU {
            list: LinkedList::new(),
            map: HashMap::new(),
            num_items: 0,
            max_items,
        }
    }

    #[test]
    fn test_push_new_element() {
        let mut lru = setup_lru(2);
        let key = Rc::new(1);
        let value = Rc::new(10);

        assert_eq!(lru.push(Rc::clone(&key), Rc::clone(&value)), None);
        assert_eq!(lru.list.len(), 1);
        assert_eq!(lru.map.len(), 1);
        assert_eq!(lru.num_items, 1);
    }

    #[test]
    fn test_push_existing_element() {
        let mut lru = setup_lru(2);
        let key = Rc::new(1);
        let value = Rc::new(10);

        lru.push(Rc::clone(&key), Rc::clone(&value));
        assert_eq!(lru.push(Rc::clone(&key), Rc::clone(&value)), None);
        assert_eq!(lru.list.len(), 2);
        assert_eq!(lru.map.len(), 1);
        assert_eq!(lru.num_items, 1);
    }

    #[test]
    fn test_gc_multiple_references() {
        let mut lru = setup_lru(2);
        let key1 = Rc::new(1);
        let value1 = Rc::new(10);
        let key2 = Rc::new(2);
        let value2 = Rc::new(20);

        lru.push(Rc::clone(&key1), Rc::clone(&value1));
        lru.push(Rc::clone(&key2), Rc::clone(&value2));
        lru.push(Rc::clone(&key1), Rc::clone(&value1)); // key1 is pushed again

        assert_eq!(lru.gc(), None); // No eviction should happen
        assert_eq!(lru.list.len(), 3);
        assert_eq!(lru.map.len(), 2);
        assert_eq!(lru.num_items, 2);
    }

    #[test]
    fn test_push_and_retrieve() {
        let mut lru = LRU {
            list: LinkedList::new(),
            map: HashMap::new(),
            num_items: 0,
            max_items: 3,
        };

        let key1 = Rc::new(1);
        let value1 = Rc::new("a");
        let key2 = Rc::new(2);
        let value2 = Rc::new("b");
        let key3 = Rc::new(3);
        let value3 = Rc::new("c");

        lru.push(key1.clone(), value1.clone());
        lru.push(key2.clone(), value2.clone());
        lru.push(key3.clone(), value3.clone());

        assert_eq!(lru.num_items, 3);
        assert!(lru.map.contains_key(&key1));
        assert!(lru.map.contains_key(&key2));
        assert!(lru.map.contains_key(&key3));
    }

    #[test]
    fn test_eviction_policy() {
        let mut lru = LRU {
            list: LinkedList::new(),
            map: HashMap::new(),
            num_items: 0,
            max_items: 2,
        };

        let key1 = Rc::new(1);
        let value1 = Rc::new("a");
        let key2 = Rc::new(2);
        let value2 = Rc::new("b");
        let key3 = Rc::new(3);
        let value3 = Rc::new("c");

        lru.push(key1.clone(), value1.clone());
        lru.push(key2.clone(), value2.clone());
        let evicted = lru.push(key3.clone(), value3.clone());

        assert_eq!(lru.num_items, 3);
        assert!(lru.map.contains_key(&key1));
        assert!(lru.map.contains_key(&key2));
        assert!(lru.map.contains_key(&key3));
        assert_eq!(evicted, None);
    }

    #[test]
    fn test_multiple_references() {
        let mut lru = LRU {
            list: LinkedList::new(),
            map: HashMap::new(),
            num_items: 0,
            max_items: 2,
        };

        let key1 = Rc::new(1);
        let value1 = Rc::new("a");

        lru.push(key1.clone(), value1.clone());
        lru.push(key1.clone(), value1.clone());

        assert_eq!(lru.num_items, 1);
        assert_eq!(lru.list.len(), 2);
        assert_eq!(lru.map.get(&key1).unwrap().1, 2);
    }

    #[test]
    fn test_eviction_with_external_references() {
        let mut lru = LRU {
            list: LinkedList::new(),
            map: HashMap::new(),
            num_items: 0,
            max_items: 1,
        };

        let key1 = Rc::new(1);
        let value1 = Rc::new("a");
        let key2 = Rc::new(2);
        let value2 = Rc::new("b");

        lru.push(key1, value1);
        let evicted = lru.push(key2.clone(), value2.clone());

        assert_eq!(lru.num_items, 1);
        assert!(lru.map.contains_key(&key2));
        assert_eq!(evicted, Some((Rc::new(1), Rc::new("a"))));
    }

    #[test]
    fn test_gc_behavior() {
        let mut lru = LRU {
            list: LinkedList::new(),
            map: HashMap::new(),
            num_items: 0,
            max_items: 1,
        };

        let key1 = Rc::new(1);
        let value1 = Rc::new("a");
        let key2 = Rc::new(2);
        let value2 = Rc::new("b");

        lru.push(key1.clone(), value1.clone());
        lru.push(key2.clone(), value2.clone());

        let gc_result = lru.gc();

        assert!(gc_result.is_none());
        assert_eq!(lru.num_items, 2);
        assert!(lru.map.contains_key(&key1));
        assert!(lru.map.contains_key(&key2));
    }

    #[test]
    fn test_push_within_limit() {
        let mut lru = LRU::new(3);
        let k1 = Rc::new(1);
        let v1 = Rc::new(10);
        assert_eq!(lru.push(Rc::clone(&k1), Rc::clone(&v1)), None);
        assert_eq!(lru.num_items, 1);
    }

    #[test]
    fn test_push_eviction() {
        let mut lru = LRU::new(2);
        let k1 = Rc::new(1);
        let v1 = Rc::new(10);
        let k2 = Rc::new(2);
        let v2 = Rc::new(20);
        let k3 = Rc::new(3);
        let v3 = Rc::new(30);

        lru.push(k1, v1);
        lru.push(Rc::clone(&k2), Rc::clone(&v2));
        let evicted = lru.push(Rc::clone(&k3), Rc::clone(&v3));
        assert!(evicted.is_some());
        assert_eq!(*evicted.unwrap().0, 1);
        assert_eq!(lru.num_items, 2);
    }

    #[test]
    fn test_gc_no_eviction() {
        let mut lru = LRU::new(3);
        let k1 = Rc::new(1);
        let v1 = Rc::new(10);
        lru.push(Rc::clone(&k1), Rc::clone(&v1));
        assert_eq!(lru.gc(), None);
    }

    #[test]
    fn test_gc_with_eviction() {
        let mut lru = LRU::new(1);
        let k1 = Rc::new(1);
        let v1 = Rc::new(10);
        let k2 = Rc::new(2);
        let v2 = Rc::new(20);

        lru.push(Rc::clone(&k1), Rc::clone(&v1));
        lru.push(Rc::clone(&k2), Rc::clone(&v2));

        drop(v1);

        let evicted = lru.gc();

        assert!(evicted.is_some());
        assert_eq!(*evicted.unwrap().0, 1);
    }
}
