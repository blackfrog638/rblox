use crate::Table;
use crate::{chunk::Chunk, memory::ObjId, value::Value};

pub type NativeFn = fn(&mut crate::vm::VM, &[Value]) -> Result<Value, String>;

#[derive(Clone, Debug)]
pub enum Object {
    String { value: String, hash: u32 },
    Function(ObjFunction),
    Closure(Closure),
    NativeFunction(NativeFn),
    Upvalue(Upvalue),
    Class { name: String, methods: Table },
    Instance { class: ObjId, fields: Table },
}

impl Object {
    pub(crate) fn managed_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::String { value, .. } => value.capacity(),
                Self::Function(function) => {
                    function.chunk.code.capacity()
                        + function.chunk.constants.capacity() * std::mem::size_of::<Value>()
                        + function.chunk.lines.capacity()
                            * std::mem::size_of::<crate::chunk::LineRun>()
                        + function.name.as_ref().map_or(0, String::capacity)
                        + function.upvalues.capacity() * std::mem::size_of::<UpvalueDesc>()
                }
                Self::Closure(closure) => {
                    closure.upvalues.capacity() * std::mem::size_of::<ObjId>()
                }
                Self::Class { name, methods } => {
                    name.capacity()
                        + methods.capacity() * std::mem::size_of::<crate::table::Entry>()
                }
                Self::NativeFunction(_) | Self::Upvalue(_) => 0,
                Self::Instance { fields, .. } => {
                    fields.capacity() * std::mem::size_of::<crate::table::Entry>()
                }
            }
    }

    pub fn string_hash(&self) -> Option<u32> {
        match self {
            Object::String { hash, .. } => Some(*hash),
            Object::Function(_) | Object::Closure(_) => None,
            Object::NativeFunction(_) => None,
            Object::Upvalue(_) => None,
            Object::Class { .. } => None,
            Object::Instance { .. } => None,
        }
    }

    pub fn string_value(&self) -> Option<&str> {
        match self {
            Object::String { value, .. } => Some(value),
            Object::Function(_) | Object::Closure(_) => None,
            Object::NativeFunction(_) => None,
            Object::Upvalue(_) => None,
            Object::Class { .. } => None,
            Object::Instance { .. } => None,
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
