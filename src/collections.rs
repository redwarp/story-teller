use std::{
    borrow::Borrow,
    cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
    collections::{BinaryHeap, HashMap},
    hash::Hash,
    time::{Duration, Instant},
};

struct Access<K> {
    instant: Instant,
    key: K,
}
impl<K> PartialOrd for Access<K> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.instant.cmp(&other.instant).reverse())
    }
}
impl<K> Ord for Access<K> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.instant.cmp(&other.instant).reverse()
    }
}
impl<K> PartialEq for Access<K> {
    fn eq(&self, other: &Self) -> bool {
        self.instant.eq(&other.instant)
    }
}
impl<K> Eq for Access<K> {}

struct Value<V> {
    last_access: Instant,
    value: V,
}

/// Inspired by [the rust-lang forums](https://users.rust-lang.org/t/map-that-removes-entries-after-a-given-time-after-last-access/42767/2)
pub struct ExpiringHashMap<K, V> {
    map: HashMap<K, Value<V>>,
    access_log: BinaryHeap<Access<K>>,
    duration: Duration,
}

impl<K: Eq + Hash + Clone, V> ExpiringHashMap<K, V> {
    pub fn new(duration: Duration) -> Self {
        Self {
            map: HashMap::new(),
            access_log: BinaryHeap::new(),
            duration,
        }
    }

    pub fn insert(&mut self, key: K, v: V) -> Option<V> {
        self.cleanup();
        let now = Instant::now();
        match self.map.insert(
            key.clone(),
            Value {
                last_access: now,
                value: v,
            },
        ) {
            Some(prev) => Some(prev.value),
            None => {
                self.access_log.push(Access { instant: now, key });
                None
            }
        }
    }

    fn cleanup(&mut self) {
        let deadline = Instant::now()
            .checked_sub(self.duration)
            .expect("We use duration in minutes");
        while let Some(Access { instant, .. }) = self.access_log.peek() {
            if *instant > deadline {
                return;
            }

            let key = self.access_log.pop().expect("We know it is not empty.").key;

            if let Some(last_access) = self.map.get(&key).map(|value| value.last_access) {
                if last_access > deadline {
                    // Real access is recent, so we put it back in the heap for future check.
                    self.access_log.push(Access {
                        instant: last_access,
                        key,
                    });
                } else {
                    self.map.remove(&key);
                }
            }
        }
    }

    pub fn get(&mut self, k: &K) -> Option<&V> {
        self.cleanup();
        match self.map.get_mut(k) {
            Some(Value {
                last_access: time,
                value,
            }) => {
                *time = Instant::now();
                Some(&*value)
            }
            None => None,
        }
    }

    pub fn remove<Q>(&mut self, k: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.map.remove(k).map(|Value { value, .. }| value)
    }
}
