use std::sync::atomic::{AtomicUsize, Ordering};

use crate::object::{Object, hash_string};
use crate::value::Value;

static NEXT_HEAP_ID: AtomicUsize = AtomicUsize::new(0);

/// A non-owning reference into one Heap. Copying it does not keep an object alive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjId {
    heap: usize,
    index: usize,
}

/// Owns all Lox objects. Slots are append-only until garbage collection is added.
/// Rust still allocates and releases the Vec and each object's String/Vec fields.
#[derive(Debug)]
pub struct Heap {
    id: usize,
    objects: Vec<Object>,
}

impl Heap {
    pub fn new() -> Self {
        Self {
            id: NEXT_HEAP_ID.fetch_add(1, Ordering::Relaxed),
            objects: Vec::new(),
        }
    }

    pub fn alloc(&mut self, object: Object) -> ObjId {
        let id = ObjId {
            heap: self.id,
            index: self.objects.len(),
        };
        crate::gc_log!(
            "allocate {:?} ({}) - {} objects",
            id,
            match &object {
                Object::String { .. } => "string",
                Object::Function(_) => "function",
                Object::Closure(_) => "closure",
                Object::NativeFunction(_) => "native function",
                Object::Upvalue(_) => "upvalue",
            },
            self.objects.len() + 1,
        );
        self.objects.push(object);
        id
    }

    pub fn alloc_string(&mut self, value: String) -> Value {
        let hash = hash_string(&value);
        Value::Obj(self.alloc(Object::String { value, hash }))
    }

    pub fn get(&self, id: ObjId) -> &Object {
        assert_eq!(id.heap, self.id, "Object belongs to a different heap.");
        &self.objects[id.index]
    }

    pub fn get_mut(&mut self, id: ObjId) -> &mut Object {
        assert_eq!(id.heap, self.id, "Object belongs to a different heap.");
        &mut self.objects[id.index]
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}
