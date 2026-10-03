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

    #[allow(dead_code)] // Called after root marking when collection is wired in.
    pub(crate) fn trace_references(&mut self) {
        while let Some(id) = self.gray_stack.pop() {
            crate::gc_log!("trace {:?} ({:?})", id, self.get(id));
            let references = match self.get(id) {
                Object::String { .. } | Object::NativeFunction(_) => Vec::new(),
                Object::Function(function) => function
                    .chunk
                    .constants
                    .iter()
                    .filter_map(|value| match value {
                        Value::Obj(id) => Some(*id),
                        Value::Number(_) | Value::Bool(_) | Value::Nil => None,
                    })
                    .collect(),
                Object::Closure(closure) => std::iter::once(closure.function)
                    .chain(closure.upvalues.iter().copied())
                    .collect(),
                Object::Upvalue(upvalue) => upvalue
                    .closed
                    .iter()
                    .filter_map(|value| match value {
                        Value::Obj(id) => Some(*id),
                        Value::Number(_) | Value::Bool(_) | Value::Nil => None,
                    })
                    .chain(upvalue.next)
                    .collect(),
            };

            for reference in references {
                self.mark_object(reference);
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::{Closure, ObjFunction, Upvalue};

    #[test]
    fn traces_function_closure_and_upvalue_references() {
        let mut heap = Heap::new();
        let Value::Obj(literal) = heap.alloc_string("literal".to_string()) else {
            unreachable!()
        };
        let Value::Obj(orphan) = heap.alloc_string("orphan".to_string()) else {
            unreachable!()
        };

        let next_upvalue = heap.alloc(Object::Upvalue(Upvalue {
            location: 0,
            closed: None,
            next: None,
        }));
        let upvalue = heap.alloc(Object::Upvalue(Upvalue {
            location: 1,
            closed: Some(Value::Obj(literal)),
            next: Some(next_upvalue),
        }));
        let mut function = ObjFunction::new();
        function.chunk.add_constant(Value::Obj(literal));
        function.chunk.add_constant(Value::Number(42.0));
        let function = heap.alloc(Object::Function(function));
        let closure = heap.alloc(Object::Closure(Closure {
            function,
            upvalues: vec![upvalue],
        }));

        heap.mark_roots([Value::Obj(closure)]);
        heap.trace_references();

        for reachable in [literal, next_upvalue, upvalue, function, closure] {
            assert!(
                heap.is_marked(reachable),
                "expected {reachable:?} to be marked"
            );
        }
        assert!(!heap.is_marked(orphan));
        assert!(heap.gray_stack.is_empty());
    }

    #[test]
    fn tracing_cyclic_references_marks_each_object_once() {
        let mut heap = Heap::new();
        let function = heap.alloc(Object::Function(ObjFunction::new()));
        let closure = heap.alloc(Object::Closure(Closure {
            function,
            upvalues: Vec::new(),
        }));
        let upvalue = heap.alloc(Object::Upvalue(Upvalue {
            location: 0,
            closed: Some(Value::Obj(closure)),
            next: None,
        }));
        let Object::Closure(closure_object) = heap.get_mut(closure) else {
            unreachable!()
        };
        closure_object.upvalues.push(upvalue);

        heap.mark_roots([Value::Obj(closure)]);
        heap.trace_references();

        assert!(heap.is_marked(function));
        assert!(heap.is_marked(closure));
        assert!(heap.is_marked(upvalue));
        assert!(heap.gray_stack.is_empty());
    }
}
