use std::sync::atomic::{AtomicUsize, Ordering};

use crate::object::{Object, hash_string};
use crate::table::Table;
use crate::value::Value;

static NEXT_HEAP_ID: AtomicUsize = AtomicUsize::new(0);
const INITIAL_GC_THRESHOLD: usize = 1024 * 1024;

/// A non-owning reference into one Heap. Copying it does not keep an object alive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjId {
    heap: usize,
    index: usize,
    generation: u64,
}

#[derive(Debug)]
struct Slot {
    object: Option<Object>,
    generation: u64,
    marked: bool,
}

/// Owns all Lox objects in stable, reusable slots. Rust drops an unreachable
/// object's String/Vec buffers; the collector decides *when* to drop it.
#[derive(Debug)]
pub struct Heap {
    id: usize,
    objects: Vec<Slot>,
    free_slots: Vec<usize>,
    gray_stack: Vec<ObjId>,

    strings: Table,
    bytes_allocated: usize,
    next_gc: usize,
}

impl Heap {
    pub fn new() -> Self {
        Self {
            id: NEXT_HEAP_ID.fetch_add(1, Ordering::Relaxed),
            objects: Vec::new(),
            free_slots: Vec::new(),
            gray_stack: Vec::new(),
            strings: Table::new(),
            bytes_allocated: 0,
            next_gc: INITIAL_GC_THRESHOLD,
        }
    }

    pub fn alloc(&mut self, object: Object) -> ObjId {
        let index = self.free_slots.pop().unwrap_or_else(|| {
            self.objects.push(Slot {
                object: None,
                generation: 0,
                marked: false,
            });
            self.objects.len() - 1
        });
        let id = ObjId {
            heap: self.id,
            index,
            generation: self.objects[index].generation,
        };
        crate::gc_log!(
            "allocate {:?} ({}) - {} objects",
            id,
            match &object {
                Object::String { .. } => "string",
                Object::Function(_) => "function",
                Object::Closure(_) => "closure",
                Object::BoundMethod { .. } => "bound method",
                Object::NativeFunction(_) => "native function",
                Object::Upvalue(_) => "upvalue",
                Object::Class { .. } => "class",
                Object::Instance { .. } => "instance",
            },
            self.len(),
        );
        self.bytes_allocated = self.bytes_allocated.saturating_add(object.managed_size());
        self.objects[index].object = Some(object);
        id
    }

    pub fn alloc_string(&mut self, value: String) -> Value {
        let hash = hash_string(&value);
        if let Some(id) = self.strings.find_string(&value, hash, self) {
            return Value::Obj(id);
        }
        let id = self.alloc(Object::String { value, hash });
        let mut strings = std::mem::take(&mut self.strings);
        strings.set(id, Value::Nil, self);
        self.strings = strings;
        Value::Obj(id)
    }

    pub fn get(&self, id: ObjId) -> &Object {
        self.slot(id)
            .object
            .as_ref()
            .expect("Object was collected.")
    }

    pub(crate) fn set_instance_field(
        &mut self,
        instance: ObjId,
        name: ObjId,
        value: Value,
    ) -> Result<(), &'static str> {
        self.set_object_table_entry(instance, name, value, |object| match object {
            Object::Instance { fields, .. } => Ok(fields),
            _ => Err("Only instances have fields."),
        })
    }

    pub(crate) fn set_class_method(
        &mut self,
        class: ObjId,
        name: ObjId,
        method: Value,
    ) -> Result<(), &'static str> {
        self.set_object_table_entry(class, name, method, |object| match object {
            Object::Class { methods, .. } => Ok(methods),
            _ => Err("Method declaration requires a class."),
        })
    }

    fn set_object_table_entry(
        &mut self,
        object: ObjId,
        name: ObjId,
        value: Value,
        select: fn(&mut Object) -> Result<&mut Table, &'static str>,
    ) -> Result<(), &'static str> {
        // Detach the table while its insertion reads keys from the heap.
        // Collection only runs between instructions, after the table is restored.
        let mut table = std::mem::take(select(self.get_mut(object))?);
        let old_capacity = table.capacity();
        table.set(name, value, self);
        self.bytes_allocated = self.bytes_allocated.saturating_add(
            (table.capacity() - old_capacity) * std::mem::size_of::<crate::table::Entry>(),
        );
        *select(self.get_mut(object))? = table;
        Ok(())
    }

    pub fn get_mut(&mut self, id: ObjId) -> &mut Object {
        self.slot(id);
        self.objects[id.index]
            .object
            .as_mut()
            .expect("Object was collected.")
    }

    fn slot(&self, id: ObjId) -> &Slot {
        assert_eq!(id.heap, self.id, "Object belongs to a different heap.");
        let slot = self
            .objects
            .get(id.index)
            .expect("Object handle points outside its heap.");
        assert_eq!(slot.generation, id.generation, "Object was collected.");
        assert!(slot.object.is_some(), "Object was collected.");
        slot
    }

    pub fn len(&self) -> usize {
        self.objects.len() - self.free_slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Estimated managed object bytes, including owned buffers. This excludes
    /// allocator overhead and collector/table capacity, and is not process RSS.
    /// Mutations through `get_mut` are reconciled at the next collection.
    pub fn bytes_allocated(&self) -> usize {
        self.bytes_allocated
    }

    pub(crate) fn should_collect(&self) -> bool {
        cfg!(feature = "stress-gc") || self.bytes_allocated > self.next_gc
    }

    pub(crate) fn finish_collection(&mut self) {
        // Weak entries must disappear BEFORE their target slots are freed.
        let mut strings = std::mem::take(&mut self.strings);
        strings.remove_white(self);
        self.strings = strings;
        self.sweep();
        self.next_gc = self
            .bytes_allocated
            .saturating_mul(2)
            .max(INITIAL_GC_THRESHOLD);
    }

    pub(crate) fn mark_roots(&mut self, roots: impl IntoIterator<Item = Value>) {
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
        self.slot(id);
        if !self.objects[id.index].marked {
            self.objects[id.index].marked = true;
            self.gray_stack.push(id);
            crate::gc_log!("mark {:?}", id);
        }
    }

    fn sweep(&mut self) {
        self.bytes_allocated = 0;
        for (index, slot) in self.objects.iter_mut().enumerate() {
            let Some(object) = &slot.object else { continue };
            if slot.marked {
                slot.marked = false;
                self.bytes_allocated = self.bytes_allocated.saturating_add(object.managed_size());
            } else {
                crate::gc_log!(
                    "free slot {} generation {} ({:?})",
                    index,
                    slot.generation,
                    object
                );
                // Never remove/swap_remove: every ObjId must retain its index.
                slot.object = None;
                slot.generation = slot
                    .generation
                    .checked_add(1)
                    .expect("Object generation overflow.");
                self.free_slots.push(index);
            }
        }
    }

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
                Object::BoundMethod { receiver, method } => {
                    let mut references = vec![*method];
                    if let Value::Obj(instance) = receiver {
                        references.push(*instance);
                    }
                    references
                }
                Object::Upvalue(upvalue) => upvalue
                    .closed
                    .iter()
                    .filter_map(|value| match value {
                        Value::Obj(id) => Some(*id),
                        Value::Number(_) | Value::Bool(_) | Value::Nil => None,
                    })
                    .chain(upvalue.next)
                    .collect(),
                Object::Class { methods, .. } => methods
                    .gc_roots()
                    .filter_map(|value| match value {
                        Value::Obj(id) => Some(id),
                        _ => None,
                    })
                    .collect(),
                Object::Instance { class, fields } => fields
                    .gc_roots()
                    .filter_map(|value| match value {
                        Value::Obj(id) => Some(id),
                        Value::Number(_) | Value::Bool(_) | Value::Nil => None,
                    })
                    .chain(std::iter::once(*class))
                    .collect(),
            };

            for reference in references {
                self.mark_object(reference);
            }
        }
    }

    pub(crate) fn is_marked(&self, id: ObjId) -> bool {
        self.slot(id).marked
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
    fn field_table_growth_is_accounted_for_before_collection() {
        let mut heap = Heap::new();
        let class = heap.alloc(Object::Class {
            name: "Box".into(),
            methods: Table::new(),
        });
        let instance = heap.alloc(Object::Instance {
            class,
            fields: Table::new(),
        });
        for index in 0..40 {
            let Value::Obj(name) = heap.alloc_string(format!("field{index}")) else {
                unreachable!()
            };
            heap.set_instance_field(instance, name, Value::Number(index as f64))
                .unwrap();
            heap.set_instance_field(instance, name, Value::Nil).unwrap();
            let expected: usize = heap
                .objects
                .iter()
                .filter_map(|slot| slot.object.as_ref())
                .map(Object::managed_size)
                .sum();
            assert_eq!(heap.bytes_allocated(), expected);
        }
        heap.mark_roots([Value::Obj(instance)]);
        heap.trace_references();
        heap.finish_collection();
        let Object::Instance { fields, .. } = heap.get(instance) else {
            unreachable!()
        };
        assert_eq!(fields.len(), 40);
    }

    #[test]
    fn instances_keep_their_class_field_keys_and_values_alive() {
        let mut heap = Heap::new();
        let class = heap.alloc(Object::Class {
            name: "Box".into(),
            methods: Table::new(),
        });
        let Value::Obj(key) = heap.alloc_string("item".into()) else {
            unreachable!()
        };
        let value = heap.alloc_string("contents".into());
        let mut fields = Table::new();
        fields.set(key, value, &heap);
        let instance = heap.alloc(Object::Instance { class, fields });

        heap.mark_roots([Value::Obj(instance)]);
        heap.trace_references();
        heap.finish_collection();

        assert_eq!(heap.len(), 4);
        assert_eq!(
            Value::Obj(instance).display(&heap).to_string(),
            "Box instance"
        );
        assert_eq!(heap.alloc_string("item".into()), Value::Obj(key));
        let Object::Instance { fields, .. } = heap.get(instance) else {
            unreachable!()
        };
        assert_eq!(fields.get(&key, &heap), Some(&value));
        assert_eq!(value.as_string(&heap).unwrap(), "contents");

        heap.mark_roots([]);
        heap.trace_references();
        heap.finish_collection();
        assert!(heap.is_empty());
    }

    #[test]
    fn sweeping_does_not_move_surviving_objects() {
        let mut heap = Heap::new();
        heap.alloc_string("garbage before survivor".into());
        let survivor = heap.alloc_string("survivor".into());
        heap.mark_roots([survivor]);
        heap.trace_references();
        heap.finish_collection();

        assert_eq!(survivor.as_string(&heap).unwrap(), "survivor");
        assert_eq!(heap.len(), 1);
    }

    #[test]
    fn equal_strings_share_an_interned_object() {
        let mut heap = Heap::new();
        let first = heap.alloc_string("same".into());
        let second = heap.alloc_string(["sa", "me"].concat());
        assert_eq!(first, second);
        assert_eq!(heap.len(), 1);
    }

    #[test]
    fn weak_strings_are_removed_before_reusing_slots_and_probe_chains_survive() {
        let mut heap = Heap::new();
        // Three keys in one bucket force lookup past a deleted weak key.
        let names: Vec<_> = (0..1000)
            .map(|i| format!("collision-{i}"))
            .filter(|s| hash_string(s).is_multiple_of(16))
            .take(3)
            .collect();
        let dead = heap.alloc_string(names[0].clone());
        let live = heap.alloc_string(names[1].clone());
        heap.alloc_string(names[2].clone());
        heap.mark_roots([live]);
        heap.trace_references();
        heap.finish_collection();

        assert_eq!(heap.len(), 1);
        assert_eq!(heap.strings.len(), 1);
        assert_eq!(heap.alloc_string(names[1].clone()), live);
        let recreated = heap.alloc_string(names[0].clone());
        assert_ne!(recreated, dead);
        assert_eq!(recreated.as_string(&heap).unwrap(), &names[0]);
        assert_eq!(heap.alloc_string(names[0].clone()), recreated);
    }

    #[test]
    #[should_panic(expected = "Object was collected.")]
    fn old_handles_cannot_alias_reused_slots() {
        let mut heap = Heap::new();
        let old = heap.alloc_string("old".into());
        heap.mark_roots([]);
        heap.trace_references();
        heap.finish_collection();
        heap.alloc_string("new".into());
        old.as_string(&heap);
    }

    #[test]
    fn repeated_collections_reset_marks_and_reuse_slots() {
        let mut heap = Heap::new();
        let live = heap.alloc_string("kept".into());
        for index in 0..100 {
            heap.alloc_string(format!("temporary-{index}"));
            heap.mark_roots([live]);
            heap.trace_references();
            heap.finish_collection();
            assert_eq!(heap.len(), 1);
            assert_eq!(heap.objects.len(), 2);
            assert_eq!(heap.strings.len(), 1);
            assert_eq!(live.as_string(&heap).unwrap(), "kept");
        }
        heap.mark_roots([]);
        heap.trace_references();
        heap.finish_collection();
        assert!(heap.is_empty());
        assert_eq!(heap.bytes_allocated(), 0);
    }

    #[test]
    fn weak_string_cache_reuses_tombstones_without_growing_forever() {
        let mut heap = Heap::new();
        for index in 0..1000 {
            heap.alloc_string(format!("temporary-{index}"));
            heap.mark_roots([]);
            heap.trace_references();
            heap.finish_collection();
        }
        assert_eq!(heap.strings.capacity(), 16);
    }

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
