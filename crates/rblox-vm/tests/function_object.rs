use std::rc::Rc;

use rblox_vm::chunk::{OP_CONSTANT, OP_RETURN};
use rblox_vm::{
    Chunk, Closure, ObjFunction, ObjType, Object, Table, Value, allocate_string, disassemble_chunk,
};

fn function(name: Option<&str>) -> Value {
    Value::Obj(Rc::new(Object::Closure(Closure::new(ObjFunction {
        name: name.map(str::to_owned),
        ..ObjFunction::new()
    }))))
}

#[test]
fn functions_compare_by_identity_and_strings_by_content() {
    let first = function(Some("example"));
    assert_eq!(first, first.clone());
    assert_ne!(first, function(Some("example")));
    assert_ne!(first, allocate_string("example".into()));
    assert_eq!(
        allocate_string("same".into()),
        allocate_string("same".into())
    );
    assert!(first.is_obj_type(ObjType::Closure));
    assert!(first.as_string().is_none());
    assert!(allocate_string("example".into()).as_function().is_none());
}

#[test]
fn function_constants_display_in_disassembly() {
    let script = function(None);
    assert_eq!(script.to_string(), "<script>");
    let data = script.as_function().unwrap();
    assert_eq!(data.arity, 0);
    assert!(data.name.is_none());
    assert!(data.chunk.code.is_empty());

    let mut chunk = Chunk::new();
    let index = chunk.add_constant(function(Some("example")));
    chunk.write(OP_CONSTANT, 1);
    chunk.write(index, 1);
    chunk.write(OP_RETURN, 1);
    assert!(disassemble_chunk(&chunk, "functions").contains("<fn example>"));
}

#[test]
fn table_stores_functions_as_values_but_cannot_lookup_function_keys() {
    let mut table = Table::new();
    let Value::Obj(key) = allocate_string("example".into()) else {
        unreachable!()
    };
    let value = function(Some("example"));
    table.set(key.clone(), value.clone());
    assert_eq!(table.get(&key), Some(&value));
    let Value::Obj(invalid_key) = value else {
        unreachable!()
    };
    assert_eq!(invalid_key.string_hash(), None);
    assert_eq!(table.get(&invalid_key), None);
    assert!(!table.delete(&invalid_key));
    assert_eq!(table.len(), 1);
}

#[test]
#[should_panic(expected = "Table keys must be strings.")]
fn table_rejects_function_keys() {
    let Value::Obj(key) = function(None) else {
        unreachable!()
    };
    Table::new().set(key, Value::Nil);
}
