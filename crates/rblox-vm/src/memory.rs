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
    marked: Vec<bool>,
    gray_stack: Vec<ObjId>,
}

impl Heap {
    pub fn new() -> Self {
        Self {
            id: NEXT_HEAP_ID.fetch_add(1, Ordering::Relaxed),
            objects: Vec::new(),
            marked: Vec::new(),
            gray_stack: Vec::new(),
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
        self.marked.push(false);
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

    pub(crate) fn mark_roots(&mut self, roots: impl IntoIterator<Item = Value>) {
        self.marked.fill(false);
        self.gray_stack.clear();

        let roots: Vec<_> = roots.into_iter().collect();
        crate::gc_log!("marking {} roots", roots.len());
        for root in roots {
            self.mark_value(root);
        }
    }

    fn mark_value(&mut self, value: Value) {
        if let Value::Obj(id) = value {
            self.mark_object(id);
        }
    }

    fn mark_object(&mut self, id: ObjId) {
        assert_eq!(id.heap, self.id, "Object belongs to a different heap.");
        let marked = self
            .marked
            .get_mut(id.index)
            .expect("Object handle points outside its heap.");
        if !*marked {
            *marked = true;
            self.gray_stack.push(id);
            crate::gc_log!("mark {:?}", id);
        }
    }

    #[cfg(test)]
    pub(crate) fn is_marked(&self, id: ObjId) -> bool {
        assert_eq!(id.heap, self.id, "Object belongs to a different heap.");
        self.marked[id.index]
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}
