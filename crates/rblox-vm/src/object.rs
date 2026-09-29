use std::rc::Rc;

use crate::{chunk::Chunk, value::Value};

pub type NativeFn = fn(&mut crate::vm::VM, &[Value]) -> Result<Value, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjType {
    String,
    Function,
    NativeFunction,
}

#[derive(Clone, Debug)]
pub enum Object {
    String { value: String, hash: u32 },
    Function(ObjFunction),
    NativeFunction(NativeFn),
}

impl Object {
    pub fn obj_type(&self) -> ObjType {
        match self {
            Object::String { .. } => ObjType::String,
            Object::Function(_) => ObjType::Function,
            Object::NativeFunction(_) => ObjType::NativeFunction,
        }
    }

    pub fn string_hash(&self) -> Option<u32> {
        match self {
            Object::String { hash, .. } => Some(*hash),
            Object::Function(_) => None,
            Object::NativeFunction(_) => None,
        }
    }

    pub fn string_value(&self) -> Option<&str> {
        match self {
            Object::String { value, .. } => Some(value),
            Object::Function(_) => None,
            Object::NativeFunction(_) => None,
        }
    }
}

impl std::fmt::Display for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Object::String { value, .. } => write!(f, "{}", value),
            Object::Function(function) => match &function.name {
                Some(name) => write!(f, "<fn {}>", name),
                None => write!(f, "<script>"),
            },
            Object::NativeFunction(_) => write!(f, "<native function>"),
        }
    }
}

pub fn allocate_string(value: String) -> Value {
    Value::Obj(Rc::new(Object::String {
        hash: hash_string(&value),
        value,
    }))
}

pub fn hash_string(value: &str) -> u32 {
    value.bytes().fold(2166136261, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(16777619)
    })
}

#[derive(Clone, Debug)]
pub struct ObjFunction {
    pub arity: usize,
    pub chunk: Chunk,
    pub name: Option<String>,
}

impl ObjFunction {
    pub fn new() -> Self {
        Self {
            arity: 0,
            chunk: Chunk::new(),
            name: None,
        }
    }
}

impl Default for ObjFunction {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::String {
                    value: left,
                    hash: left_hash,
                },
                Self::String {
                    value: right,
                    hash: right_hash,
                },
            ) => left == right && left_hash == right_hash,
            // Functions compare by identity, never by their bytecode or constants.
            (Self::Function(_), Self::Function(_)) => std::ptr::eq(self, other),
            (Self::NativeFunction(_), Self::NativeFunction(_)) => std::ptr::eq(self, other),
            _ => false,
        }
    }
}
