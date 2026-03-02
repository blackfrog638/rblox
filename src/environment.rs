use crate::value::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
// Rust 伪代码
pub struct Environment {
    values: HashMap<String, Value>,
    enclosing: Option<Rc<RefCell<Environment>>>,
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            values: std::collections::HashMap::new(),
            enclosing: None,
        }
    }

    pub fn new_enclosed(enclosing: Rc<RefCell<Environment>>) -> Self {
        Environment {
            values: std::collections::HashMap::new(),
            enclosing: Some(enclosing),
        }
    }

    pub fn define(&mut self, name: String, value: Value) {
        self.values.insert(name, value);
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(value) = self.values.get(name) {
            Some(value.clone())
        } else if let Some(enclosing) = &self.enclosing {
            enclosing.borrow().get(name)
        } else {
            None
        }
    }

    pub fn assign(&mut self, name: &str, value: Value) -> bool {
        if let Some(slot) = self.values.get_mut(name) {
            *slot = value;
            true
        } else if let Some(enclosing) = &self.enclosing {
            enclosing.borrow_mut().assign(name, value)
        } else {
            false
        }
    }

    pub fn get_at(&self, distance: usize, name: &str) -> Option<Value> {
        if distance == 0 {
            self.values.get(name).cloned()
        } else if let Some(enclosing) = &self.enclosing {
            enclosing.borrow().get_at(distance - 1, name)
        } else {
            None
        }
    }

    pub fn assign_at(&mut self, distance: usize, name: &str, value: Value) -> bool {
        if distance == 0 {
            if let Some(slot) = self.values.get_mut(name) {
                *slot = value;
                true
            } else {
                false
            }
        } else if let Some(enclosing) = &self.enclosing {
            enclosing.borrow_mut().assign_at(distance - 1, name, value)
        } else {
            false
        }
    }
}
