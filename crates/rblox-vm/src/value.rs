use std::fmt;

use crate::memory::{Heap, ObjId};
use crate::object::{ObjFunction, Object};

/// Rust equality compares handles; use `equals` for Lox equality (string content).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Nil,
    Obj(ObjId),
}

impl Value {
    pub fn as_function<'a>(&self, heap: &'a Heap) -> Option<&'a ObjFunction> {
        match self.as_obj(heap)? {
            Object::Function(function) => Some(function),
            Object::Closure(closure) => match heap.get(closure.function) {
                Object::Function(function) => Some(function),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn as_obj<'a>(&self, heap: &'a Heap) -> Option<&'a Object> {
        match self {
            Value::Obj(id) => Some(heap.get(*id)),
            _ => None,
        }
    }

    pub fn as_string<'a>(&self, heap: &'a Heap) -> Option<&'a String> {
        match self.as_obj(heap)? {
            Object::String { value, .. } => Some(value),
            _ => None,
        }
    }

    pub fn equals(&self, other: &Self, heap: &Heap) -> bool {
        match (self, other) {
            (Self::Obj(left), Self::Obj(right)) => match (heap.get(*left), heap.get(*right)) {
                (Object::String { value: a, .. }, Object::String { value: b, .. }) => a == b,
                (Object::Closure(_), Object::Closure(_))
                | (Object::NativeFunction(_), Object::NativeFunction(_)) => left == right,
                _ => false,
            },
            _ => self == other,
        }
    }

    pub fn display<'a>(&'a self, heap: &'a Heap) -> ValueDisplay<'a> {
        ValueDisplay { value: self, heap }
    }
}

pub fn as_string<'a>(value: &Value, heap: &'a Heap) -> Option<&'a String> {
    value.as_string(heap)
}

pub fn as_str<'a>(value: &Value, heap: &'a Heap) -> Option<&'a str> {
    value.as_string(heap).map(String::as_str)
}

pub struct ValueDisplay<'a> {
    value: &'a Value,
    heap: &'a Heap,
}

impl fmt::Display for ValueDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            Value::Number(value) => write!(f, "{}", value),
            Value::Bool(value) => write!(f, "{}", value),
            Value::Nil => write!(f, "nil"),
            Value::Obj(id) => match self.heap.get(*id) {
                Object::String { value, .. } => write!(f, "{}", value),
                Object::Function(function) => write!(f, "{}", function),
                Object::Closure(closure) => match self.heap.get(closure.function) {
                    Object::Function(function) => write!(f, "{}", function),
                    _ => write!(f, "<invalid closure>"),
                },
                Object::NativeFunction(_) => write!(f, "<native function>"),
                Object::Upvalue(_) => write!(f, "<upvalue>"),
            },
        }
    }
}
