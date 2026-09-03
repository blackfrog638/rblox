use crate::chunk::{Object, Value};
use std::rc::Rc;

const INITIAL_CAPACITY: usize = 8;
const MAX_LOAD: usize = 75;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub key: Option<Rc<Object>>,
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    entries: Vec<Entry>,
    count: usize,
}

impl Table {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            count: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn capacity(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, key: &Rc<Object>) -> Option<&Value> {
        if self.entries.is_empty() {
            return None;
        }

        let index = self.find_entry(key)?;
        self.entries[index].key.as_ref()?;
        Some(&self.entries[index].value)
    }

    pub fn find_string(&self, value: &str, hash: u32) -> Option<Rc<Object>> {
        if self.entries.is_empty() {
            return None;
        }

        let mut index = (hash as usize) % self.entries.len();
        loop {
            let entry = &self.entries[index];
            match &entry.key {
                None => return None,
                Some(key) if key.string_hash() == hash && key.string_value() == Some(value) => {
                    return Some(key.clone());
                }
                Some(_) => {}
            }
            index = (index + 1) % self.entries.len();
        }
    }

    pub fn set(&mut self, key: Rc<Object>, value: Value) -> bool {
        if (self.count + 1) * 100 > self.capacity().max(1) * MAX_LOAD {
            self.adjust_capacity(self.capacity().max(INITIAL_CAPACITY) * 2);
        }

        let index = self.find_insert_index(&key);
        let is_new_key = self.entries[index].key.is_none();
        if is_new_key {
            self.count += 1;
        }
        self.entries[index] = Entry {
            key: Some(key),
            value,
        };
        is_new_key
    }

    pub fn delete(&mut self, key: &Rc<Object>) -> bool {
        let Some(index) = self.find_entry(key) else {
            return false;
        };

        self.entries[index].key = Some(key.clone());
        self.entries[index].value = Value::Nil;
        self.count -= 1;
        true
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.count = 0;
    }

    fn find_entry(&self, key: &Rc<Object>) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }

        let mut index = (key.string_hash() as usize) % self.entries.len();
        loop {
            let entry = &self.entries[index];
            match &entry.key {
                None => return None,
                Some(_) if entry.value == Value::Nil => {}
                Some(entry_key) if entry_key.as_ref() == key.as_ref() => return Some(index),
                Some(_) => {}
            }
            index = (index + 1) % self.entries.len();
        }
    }

    fn find_insert_index(&self, key: &Rc<Object>) -> usize {
        let mut index = (key.string_hash() as usize) % self.entries.len();
        let mut tombstone = None;

        loop {
            let entry = &self.entries[index];
            match &entry.key {
                None => return tombstone.unwrap_or(index),
                Some(entry_key) if entry.value == Value::Nil => {
                    tombstone.get_or_insert(index);
                }
                Some(entry_key) if entry_key.as_ref() == key.as_ref() => return index,
                Some(_) => {}
            }
            index = (index + 1) % self.entries.len();
        }
    }

    fn adjust_capacity(&mut self, capacity: usize) {
        let old_entries = std::mem::take(&mut self.entries);
        self.entries = (0..capacity)
            .map(|_| Entry {
                key: None,
                value: Value::Nil,
            })
            .collect();
        self.count = 0;

        for entry in old_entries {
            if let Some(key) = entry.key {
                if entry.value != Value::Nil {
                    let index = self.find_insert_index(&key);
                    self.entries[index] = Entry {
                        key: Some(key),
                        value: entry.value,
                    };
                    self.count += 1;
                }
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
    use crate::chunk::allocate_string;

    fn key(value: &str) -> Rc<Object> {
        match allocate_string(value.to_string()) {
            Value::Obj(object) => object,
            _ => unreachable!(),
        }
    }

    #[test]
    fn set_get_and_update_key() {
        let mut table = Table::new();
        let first = key("answer");
        let equivalent = key("answer");

        assert!(table.set(first, Value::Number(41.0)));
        assert_eq!(table.get(&equivalent), Some(&Value::Number(41.0)));
        assert!(!table.set(equivalent.clone(), Value::Number(42.0)));
        assert_eq!(table.get(&equivalent), Some(&Value::Number(42.0)));
    }

    #[test]
    fn find_string_matches_content_and_hash() {
        let mut table = Table::new();
        let stored = key("interned");
        let hash = stored.string_hash();
        table.set(stored.clone(), Value::Nil);

        assert_eq!(table.find_string("interned", hash), Some(stored));
        assert_eq!(table.find_string("different", hash), None);
    }

    #[test]
    fn delete_preserves_probe_sequence_with_tombstone() {
        let mut table = Table::new();
        let first = key("a");
        let second = key("b");
        table.set(first.clone(), Value::Number(1.0));
        table.set(second.clone(), Value::Number(2.0));

        assert!(table.delete(&first));
        assert_eq!(table.get(&second), Some(&Value::Number(2.0)));
        assert!(!table.delete(&first));
    }

    #[test]
    fn grows_and_rehashes_entries() {
        let mut table = Table::new();
        let keys = (0..32)
            .map(|index| key(&format!("key-{index}")))
            .collect::<Vec<_>>();

        for (index, key) in keys.iter().enumerate() {
            table.set(key.clone(), Value::Number(index as f64));
        }

        assert!(table.capacity() >= 64);
        for (index, key) in keys.iter().enumerate() {
            assert_eq!(table.get(key), Some(&Value::Number(index as f64)));
        }
    }
}
