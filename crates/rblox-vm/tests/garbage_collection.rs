use rblox_vm::{Heap, VM, compile};

#[test]
fn temporary_scripts_and_strings_do_not_accumulate_between_interpretations() {
    let mut vm = VM::new();
    let baseline = vm.heap().len();
    for index in 0..100 {
        vm.interpret(&format!(
            r#"{{ var text = "temporary-{index}" + " result"; }}"#
        ))
        .unwrap();
        assert_eq!(vm.heap().len(), baseline);
    }
}

#[test]
fn globals_closed_captures_and_cycles_survive_until_their_last_root_is_gone() {
    let mut vm = VM::new();
    let baseline = vm.heap().len();
    vm.interpret(
        r#"
        var saved;
        {
            var text = "hel" + "lo";
            fun recurse() { if (text != "hello") nil(); return recurse; }
            saved = recurse;
        }
    "#,
    )
    .unwrap();
    vm.collect_garbage();
    vm.interpret("if (saved() != saved) nil(); saved = nil;")
        .unwrap();
    // Only the global's key remains, alongside the original clock binding.
    assert_eq!(vm.heap().len(), baseline + 1);
}

#[test]
fn nested_captures_recursion_and_temporary_concatenations_survive_safe_points() {
    let mut vm = VM::new();
    vm.interpret(
        r#"
        fun outer() {
            var text = "a" + "b";
            fun middle() {
                fun inner() { return text; }
                return inner;
            }
            return middle();
        }
        var saved = outer();
        fun recurse(n) { if (n == 0) return saved(); return recurse(n - 1); }
        if (recurse(20) != "ab") nil();
        if (("x" + "y") + ("a" + "b") != "xyab") nil();
    "#,
    )
    .unwrap();
    vm.collect_garbage();
    vm.interpret("if (saved() != \"ab\") nil(); if (clock() < 0) nil();")
        .unwrap();
}

#[test]
fn failed_compilations_release_partial_functions_and_strings() {
    let mut vm = VM::new();
    let baseline = vm.heap().len();
    for index in 0..50 {
        assert!(
            vm.interpret(&format!(r#"fun broken() {{ var text = "unused-{index}";"#))
                .is_err()
        );
        assert_eq!(vm.heap().len(), baseline);
    }
}

#[test]
fn runtime_errors_keep_escaped_captures_alive_and_release_other_temporaries() {
    let mut vm = VM::new();
    assert!(
        vm.interpret(
            r#"
        var saved;
        fun fail() {
            var text = "keep" + " me";
            fun read() { return text; }
            saved = read;
            nil();
        }
        fail();
    "#
        )
        .is_err()
    );
    vm.collect_garbage();
    vm.interpret("if (saved() != \"keep me\") nil();").unwrap();
}

#[test]
fn externally_compiled_function_constants_are_rooted_before_execution() {
    let mut vm = VM::new();
    let script = compile(
        r#"fun read() { return "compiled"; } var saved = read();"#,
        vm.heap_mut(),
    )
    .unwrap();
    vm.interpret_chunk(script.chunk).unwrap();
    vm.collect_garbage();
    vm.interpret("if (saved != \"compiled\") nil();").unwrap();
}

#[test]
fn standalone_heap_allocation_does_not_collect_uninstalled_builders() {
    let mut heap = Heap::new();
    let first = heap.alloc_string("first".into());
    heap.alloc_string("x".repeat(2 * 1024 * 1024));
    assert_eq!(first.as_string(&heap).unwrap(), "first");
}
