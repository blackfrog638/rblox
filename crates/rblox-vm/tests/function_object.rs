use rblox_vm::chunk::{OP_CONSTANT, OP_RETURN};
use rblox_vm::{Chunk, Closure, Heap, ObjFunction, Object, Table, Value, disassemble_chunk};

fn function(name: Option<&str>, heap: &mut Heap) -> Value {
    let prototype = heap.alloc(Object::Function(ObjFunction {
        name: name.map(str::to_owned),
        ..ObjFunction::new()
    }));
    Value::Obj(heap.alloc(Object::Closure(Closure::new(prototype, 0))))
}

#[test]
fn functions_compare_by_identity_and_strings_by_content() {
    let mut heap = Heap::new();
    let first = function(Some("example"), &mut heap);
    let second = function(Some("example"), &mut heap);
    let text = heap.alloc_string("example".into());
    assert!(first.equals(&first, &heap));
    assert!(!first.equals(&second, &heap));
    assert!(!first.equals(&text, &heap));
    let left = heap.alloc_string("same".into());
    let right = heap.alloc_string("same".into());
    assert!(left.equals(&right, &heap));
    assert!(matches!(first.as_obj(&heap), Some(Object::Closure(_))));
    assert!(first.as_string(&heap).is_none());
    assert!(text.as_function(&heap).is_none());
}

#[test]
fn function_constants_display_in_disassembly() {
    let mut heap = Heap::new();
    let script = function(None, &mut heap);
    assert_eq!(script.display(&heap).to_string(), "<script>");
    let data = script.as_function(&heap).unwrap();
    assert_eq!(data.arity, 0);
    assert!(data.name.is_none());
    assert!(data.chunk.code.is_empty());
    let mut chunk = Chunk::new();
    let index = chunk.add_constant(function(Some("example"), &mut heap));
    chunk.write(OP_CONSTANT, 1);
    chunk.write(index, 1);
    chunk.write(OP_RETURN, 1);
    assert!(disassemble_chunk(&chunk, "functions", &heap).contains("<fn example>"));
}

#[test]
fn table_stores_functions_as_values_but_cannot_lookup_function_keys() {
    let mut heap = Heap::new();
    let mut table = Table::new();
    let Value::Obj(key) = heap.alloc_string("example".into()) else {
        unreachable!()
    };
    let value = function(Some("example"), &mut heap);
    table.set(key, value, &heap);
    assert_eq!(table.get(&key, &heap), Some(&value));
    let Value::Obj(invalid_key) = value else {
        unreachable!()
    };
    assert_eq!(heap.get(invalid_key).string_hash(), None);
    assert_eq!(table.get(&invalid_key, &heap), None);
    assert!(!table.delete(&invalid_key, &heap));
    assert_eq!(table.len(), 1);
}

#[test]
#[should_panic(expected = "Table keys must be strings.")]
fn table_rejects_function_keys() {
    let mut heap = Heap::new();
    let Value::Obj(key) = function(None, &mut heap) else {
        unreachable!()
    };
    Table::new().set(key, Value::Nil, &heap);
}
