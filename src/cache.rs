use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

type Result<T> = anyhow::Result<T>;

#[derive(Copy, Clone)]
struct CacheEntry<V> {
    value: V,
    created_at: Instant,
}

pub(crate) struct TTLCache<K, V> {
    ttl: Duration,
    cache: RwLock<HashMap<K, CacheEntry<V>>>,
}

fn remove_and_callback<K, V, F, C>(
    map: &mut HashMap<K, V>,
    should_remove: F,
    callback: C,
) -> Result<()>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
    F: Fn(&K, &V) -> bool,
    C: Fn(&K, &V) -> Result<()>,
{
    let keys_to_remove: Vec<K> = map
        .iter()
        .filter_map(|(key, value)| {
            if should_remove(key, value) {
                Some(key.clone()) // Copy the key
            } else {
                None
            }
        })
        .collect();

    for key in keys_to_remove {
        if let Some(value) = map.remove(&key) {
            callback(&key, &value)?;
        }
    }
    Ok(())
}

impl<K: Eq + std::hash::Hash + Clone, V: Clone> TTLCache<K, V> {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let cache = self.cache.read().unwrap();
        cache.get(key).map(|entry| entry.value.clone())
    }

    pub fn set(&self, key: K, value: V) {
        let mut cache = self.cache.write().unwrap();
        cache.insert(
            key,
            CacheEntry {
                value,
                created_at: Instant::now(),
            },
        );
    }

    pub fn del(&self, key: &K) {
        let mut cache = self.cache.write().unwrap();
        cache.remove(key);
    }

    pub fn expire<F>(&self, callback: F) -> Result<()>
    where
        F: Fn(&K, &V) -> Result<()>,
    {
        let mut cache = self.cache.write().unwrap();
        let now = Instant::now();
        remove_and_callback(
            &mut cache,
            |_, entry| now.duration_since(entry.created_at) > self.ttl,
            |k, v| callback(&k, &v.value),
        )
    }
}
