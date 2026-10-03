use crate::{
    memory::{Heap, ObjId},
    value::Value,
};

const INITIAL_CAPACITY: usize = 8;
const MAX_LOAD: usize = 75;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub key: Option<ObjId>,
    pub value: Option<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    entries: Vec<Entry>,
    count: usize,
    active_count: usize,
}

impl Table {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            count: 0,
            active_count: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.active_count
    }

    pub fn is_empty(&self) -> bool {
        self.active_count == 0
    }

    pub(crate) fn gc_roots(&self) -> impl Iterator<Item = Value> + '_ {
        self.entries.iter().flat_map(|entry| {
            match (entry.key, entry.value) {
                (Some(key), Some(value)) => Some([Value::Obj(key), value]),
                _ => None,
            }
            .into_iter()
            .flatten()
        })
    }

    pub fn capacity(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, key: &ObjId, heap: &Heap) -> Option<&Value> {
        if self.entries.is_empty() {
            return None;
        }

        let index = self.find_entry(key, heap)?;
        self.entries[index].key.as_ref()?;
        self.entries[index].value.as_ref()
    }

    pub fn find_string(&self, value: &str, hash: u32, heap: &Heap) -> Option<ObjId> {
        if self.entries.is_empty() {
            return None;
        }

        let mut index = (hash as usize) % self.entries.len();
        loop {
            let entry = &self.entries[index];
            match &entry.key {
                None => return None,
                Some(key)
                    if entry.value.is_some()
                        && heap.get(*key).string_hash() == Some(hash)
                        && heap.get(*key).string_value() == Some(value) =>
                {
                    return Some(*key);
                }
                Some(_) => {}
            }
            index = (index + 1) % self.entries.len();
        }
    }

    /// Inserts a value under a string key.
    ///
    /// # Panics
    /// Panics if the key is not a string object.
    pub fn set(&mut self, key: ObjId, value: Value, heap: &Heap) -> bool {
        assert!(
            heap.get(key).string_value().is_some(),
            "Table keys must be strings."
        );
        if (self.count + 1) * 100 > self.capacity().max(1) * MAX_LOAD {
            self.adjust_capacity(self.capacity().max(INITIAL_CAPACITY) * 2, heap);
        }

        let index = self.find_insert_index(&key, heap);
        let is_new_key = self.entries[index].value.is_none();
        if self.entries[index].key.is_none() {
            self.count += 1;
        }
        if is_new_key {
            self.active_count += 1;
        }
        self.entries[index] = Entry {
            key: Some(key),
            value: Some(value),
        };
        is_new_key
    }

    pub fn delete(&mut self, key: &ObjId, heap: &Heap) -> bool {
        let Some(index) = self.find_entry(key, heap) else {
            return false;
        };

        self.entries[index].value = None;
        self.active_count -= 1;
        true
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.count = 0;
        self.active_count = 0;
    }

    fn find_entry(&self, key: &ObjId, heap: &Heap) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }

        let object = heap.get(*key);
        let mut index = (object.string_hash()? as usize) % self.entries.len();
        let value = object.string_value();
        loop {
            let entry = &self.entries[index];
            match &entry.key {
                None => return None,
                Some(_) if entry.value.is_none() => {}
                Some(entry_key) if heap.get(*entry_key).string_value() == value => {
                    return Some(index);
                }
                Some(_) => {}
            }
            index = (index + 1) % self.entries.len();
        }
    }

    fn find_insert_index(&self, key: &ObjId, heap: &Heap) -> usize {
        let object = heap.get(*key);
        let mut index = (object.string_hash().expect("Table keys must be strings.") as usize)
            % self.entries.len();
        let value = object.string_value();
        let mut tombstone = None;

        loop {
            let entry = &self.entries[index];
            match &entry.key {
                None => return tombstone.unwrap_or(index),
                Some(_) if entry.value.is_none() => {
                    tombstone.get_or_insert(index);
                }
                Some(entry_key) if heap.get(*entry_key).string_value() == value => {
                    return index;
                }
                Some(_) => {}
            }
            index = (index + 1) % self.entries.len();
        }
    }

    fn adjust_capacity(&mut self, capacity: usize, heap: &Heap) {
        let old_entries = std::mem::take(&mut self.entries);
        self.entries = (0..capacity)
            .map(|_| Entry {
                key: None,
                value: None,
            })
            .collect();
        self.count = 0;
        self.active_count = 0;

        for entry in old_entries {
            if let (Some(key), Some(value)) = (entry.key, entry.value) {
                let index = self.find_insert_index(&key, heap);
                self.entries[index] = Entry {
                    key: Some(key),
                    value: Some(value),
                };
                self.count += 1;
                self.active_count += 1;
            }
        }
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(value: &str, heap: &mut Heap) -> ObjId {
        match heap.alloc_string(value.to_string()) {
            Value::Obj(object) => object,
            _ => unreachable!(),
        }
    }

    #[test]
    fn set_get_and_update_key() {
        let mut heap = Heap::new();
        let mut table = Table::new();
        let first = key("answer", &mut heap);
        let equivalent = key("answer", &mut heap);

        assert!(table.set(first, Value::Number(41.0), &heap));
        assert_eq!(table.get(&equivalent, &heap), Some(&Value::Number(41.0)));
        assert!(!table.set(equivalent, Value::Number(42.0), &heap));
        assert_eq!(table.get(&equivalent, &heap), Some(&Value::Number(42.0)));
    }

    #[test]
    fn find_string_matches_content_and_hash() {
        let mut heap = Heap::new();
        let mut table = Table::new();
        let stored = key("interned", &mut heap);
        let hash = heap.get(stored).string_hash().unwrap();
        table.set(stored, Value::Nil, &heap);

        assert_eq!(table.find_string("interned", hash, &heap), Some(stored));
        assert_eq!(table.find_string("different", hash, &heap), None);
    }

    #[test]
    fn stores_real_nil_without_confusing_it_with_tombstone() {
        let mut heap = Heap::new();
        let mut table = Table::new();
        let stored = key("nil-value", &mut heap);

        assert!(table.set(stored, Value::Nil, &heap));
        assert_eq!(table.get(&stored, &heap), Some(&Value::Nil));
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn delete_preserves_probe_sequence_with_tombstone() {
        let mut heap = Heap::new();
        let mut table = Table::new();
        let first = key("a", &mut heap);
        let second = key("b", &mut heap);
        table.set(first, Value::Number(1.0), &heap);
        table.set(second, Value::Number(2.0), &heap);

        assert!(table.delete(&first, &heap));
        assert_eq!(table.get(&second, &heap), Some(&Value::Number(2.0)));
        assert_eq!(table.len(), 1);
        assert!(!table.delete(&first, &heap));
    }

    #[test]
    fn grows_and_rehashes_entries() {
        let mut heap = Heap::new();
        let mut table = Table::new();
        let keys = (0..32)
            .map(|index| key(&format!("key-{index}"), &mut heap))
            .collect::<Vec<_>>();

        for (index, key) in keys.iter().enumerate() {
            table.set(*key, Value::Number(index as f64), &heap);
        }

        assert!(table.capacity() >= 64);
        for (index, key) in keys.iter().enumerate() {
            assert_eq!(table.get(key, &heap), Some(&Value::Number(index as f64)));
        }
    }
}
