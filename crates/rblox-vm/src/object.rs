use crate::{chunk::Chunk, memory::ObjId, value::Value};

pub type NativeFn = fn(&mut crate::vm::VM, &[Value]) -> Result<Value, String>;

#[derive(Clone, Debug)]
pub enum Object {
    String { value: String, hash: u32 },
    Function(ObjFunction),
    Closure(Closure),
    NativeFunction(NativeFn),
    Upvalue(Upvalue),
}

impl Object {
    pub fn string_hash(&self) -> Option<u32> {
        match self {
            Object::String { hash, .. } => Some(*hash),
            Object::Function(_) | Object::Closure(_) => None,
            Object::NativeFunction(_) => None,
            Object::Upvalue(_) => None,
        }
    }

    pub fn string_value(&self) -> Option<&str> {
        match self {
            Object::String { value, .. } => Some(value),
            Object::Function(_) | Object::Closure(_) => None,
            Object::NativeFunction(_) => None,
            Object::Upvalue(_) => None,
        }
    }
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

#[derive(Clone, Debug)]
pub struct Upvalue {
    /// Absolute stack slot while open; ignored once `closed` contains a value.
    pub location: usize,
    /// `None` reads the stack; `Some` holds the captured value after closing.
    pub closed: Option<Value>,
    pub next: Option<ObjId>,
}

#[derive(Clone, Debug)]
pub struct Closure {
    pub function: ObjId,
    pub upvalues: Vec<ObjId>,
}

impl Closure {
    pub fn new(function: ObjId, upvalue_count: usize) -> Self {
        let upvalues = Vec::with_capacity(upvalue_count);
        Self { function, upvalues }
    }
}

impl std::fmt::Display for ObjFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.name {
            Some(name) => write!(f, "<fn {}>", name),
            None => write!(f, "<script>"),
        }
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
