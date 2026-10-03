use rblox_vm::{Heap, VM, Value, compile};

#[test]
fn object_handles_survive_heap_growth_and_value_copies() {
    let mut heap = Heap::new();
    let original = heap.alloc_string("kept".into());
    let copy = original;
    for index in 0..4096 {
        heap.alloc_string(index.to_string());
    }
    assert_eq!(original.as_string(&heap).map(String::as_str), Some("kept"));
    assert_eq!(copy.as_string(&heap).map(String::as_str), Some("kept"));
    assert_eq!(heap.len(), 4097);
}

#[test]
#[should_panic(expected = "Object belongs to a different heap.")]
fn handles_cannot_silently_read_an_object_from_another_heap() {
    let mut first = Heap::new();
    let mut second = Heap::new();
    let Value::Obj(id) = first.alloc_string("first".into()) else {
        unreachable!()
    };
    second.alloc_string("second".into());
    second.get(id);
}

#[test]
fn externally_compiled_chunks_use_the_vms_heap() {
    let mut vm = VM::new();
    let function = compile("var message = \"hello\" + \" world\";", vm.heap_mut()).unwrap();
    vm.interpret_chunk(function.chunk).unwrap();
    vm.interpret("if (message != \"hello world\") nil();")
        .unwrap();
}

#[test]
fn returned_self_referencing_closures_survive_later_interpretations() {
    let mut vm = VM::new();
    vm.interpret("fun make() { fun f() { return f; } return f; } var saved = make();")
        .unwrap();
    // Allocate more objects in the same heap before following the closed capture.
    for _ in 0..128 {
        vm.interpret("var temporary = \"new\" + \" value\";")
            .unwrap();
    }
    vm.interpret("if (saved() != saved) nil();").unwrap();
    assert!(!vm.heap().is_empty());
}

#[test]
fn runtime_error_closes_captures_before_the_stack_is_reset() {
    let mut vm = VM::new();
    assert!(vm.interpret(
        "var saved; fun make() { var text = \"kept\"; fun get() { return text; } saved = get; nil(); } make();"
    ).is_err());
    vm.interpret("if (saved() != \"kept\") nil();").unwrap();
    assert!(vm.interpret("fun broken( {").is_err());
    vm.interpret("if (saved() != \"kept\") nil();").unwrap();
}
