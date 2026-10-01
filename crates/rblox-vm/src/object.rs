use std::rc::Rc;

use crate::{chunk::Chunk, value::Value};

pub type NativeFn = fn(&mut crate::vm::VM, &[Value]) -> Result<Value, String>;

#[derive(Clone, Debug)]
pub enum Object {
    String { value: String, hash: u32 },
    Closure(Closure),
    NativeFunction(NativeFn),
}

impl Object {
    pub fn string_hash(&self) -> Option<u32> {
        match self {
            Object::String { hash, .. } => Some(*hash),
            Object::Closure(_) => None,
            Object::NativeFunction(_) => None,
        }
    }

    pub fn string_value(&self) -> Option<&str> {
        match self {
            Object::String { value, .. } => Some(value),
            Object::Closure(_) => None,
            Object::NativeFunction(_) => None,
        }
    }
}

impl std::fmt::Display for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Object::String { value, .. } => write!(f, "{}", value),
            Object::Closure(closure) => match &closure.function.name {
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
    pub upvalue_count: usize,
    pub upvalues: Vec<UpvalueDesc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpvalueDesc {
    /// Slot in the enclosing frame or enclosing closure's upvalue array.
    pub index: u8,
    /// `true` captures a local slot; `false` forwards an enclosing upvalue.
    pub is_local: bool,
}

/// Runtime function value, ready to own captured upvalues in the VM.
#[derive(Clone, Debug)]
pub struct Closure {
    pub function: ObjFunction,
}

impl Closure {
    pub fn new(function: ObjFunction) -> Self {
        Self { function }
    }
}

impl ObjFunction {
    pub fn new() -> Self {
        Self {
            arity: 0,
            chunk: Chunk::new(),
            name: None,
            upvalue_count: 0,
            upvalues: Vec::new(),
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
            // Closures compare by identity, never by their bytecode or constants.
            (Self::Closure(_), Self::Closure(_)) => std::ptr::eq(self, other),
            (Self::NativeFunction(_), Self::NativeFunction(_)) => std::ptr::eq(self, other),
            _ => false,
        }
    }
}
