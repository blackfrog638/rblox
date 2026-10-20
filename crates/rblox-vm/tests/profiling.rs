#![cfg(feature = "profiling")]

use std::time::Duration;

use rblox_vm::VM;
use rblox_vm::chunk::{OP_ADD, OP_CALL, OP_RETURN};

fn assert_balanced(vm: &VM) {
    let profile = vm.profile().unwrap();
    assert_eq!(
        profile
            .functions()
            .iter()
            .map(|function| function.instructions)
            .sum::<u64>(),
        profile.opcode_counts().iter().sum::<u64>()
    );
    assert_eq!(
        profile
            .functions()
            .iter()
            .map(|function| function.self_time)
            .sum::<Duration>(),
        profile.elapsed()
    );
    for function in profile.functions() {
        assert!(function.inclusive_time >= function.self_time);
    }
}

#[test]
fn counts_dispatches_not_operands_and_includes_script_and_function_returns() {
    let mut vm = VM::new();
    vm.set_profiling(true);
    vm.interpret("fun add(a, b) { return a + b; } add(1, 2); add(3, 4);")
        .unwrap();
    let profile = vm.profile().unwrap();
    let add = profile
        .functions()
        .iter()
        .find(|f| f.name == "add")
        .unwrap();
    assert_eq!(add.calls, 2);
    assert_eq!(add.instructions, 8); // GET_LOCAL, GET_LOCAL, ADD, RETURN per call.
    assert_eq!(profile.opcode_counts()[OP_ADD as usize], 2);
    assert_eq!(profile.opcode_counts()[OP_CALL as usize], 2);
    assert_eq!(profile.opcode_counts()[OP_RETURN as usize], 3);
    assert!(profile.collections() >= 1);
    assert_balanced(&vm);
}

#[test]
fn recursion_closures_same_names_and_super_calls_keep_distinct_prototypes() {
    let mut vm = VM::new();
    vm.set_profiling(true);
    vm.interpret(
        r#"
        fun recurse(n) { if (n > 0) recurse(n - 1); }
        recurse(3);
        fun factory() { fun same() {} return same; }
        var a = factory();
        var b = factory();
        a(); b();
        { fun same() {} same(); }
        class A { method() {} }
        class B < A { method() { super.method(); } }
        B().method();
    "#,
    )
    .unwrap();
    let functions = vm.profile().unwrap().functions();
    assert_eq!(
        functions
            .iter()
            .find(|f| f.name == "recurse")
            .unwrap()
            .calls,
        4
    );
    let mut same_calls: Vec<_> = functions
        .iter()
        .filter(|f| f.name == "same")
        .map(|f| f.calls)
        .collect();
    same_calls.sort_unstable();
    assert_eq!(same_calls, [1, 2]);
    assert_eq!(functions.iter().filter(|f| f.name == "method").count(), 2);
    assert_balanced(&vm);
}

#[test]
fn errors_unwind_profiles_and_collected_functions_do_not_alias_new_runs() {
    let mut vm = VM::new();
    vm.set_profiling(true);
    assert!(vm.interpret("fun fail() { nil(); } fail();").is_err());
    assert_balanced(&vm);
    assert!(vm.profile().unwrap().to_string().contains("fail"));
    assert!(vm.interpret("var = ;").is_err());
    vm.interpret("fun ok() {} ok();").unwrap();
    assert_balanced(&vm);
    assert!(
        vm.profile()
            .unwrap()
            .functions()
            .iter()
            .any(|f| f.name == "ok")
    );

    vm.set_profiling(true);
    for _ in 0..5 {
        vm.interpret("{ fun local() {} local(); }").unwrap();
        vm.collect_garbage();
    }
    assert_eq!(
        vm.profile()
            .unwrap()
            .functions()
            .iter()
            .filter(|f| f.name == "local")
            .count(),
        5
    );
    assert_balanced(&vm);
    vm.set_profiling(false);
    assert!(vm.profile().is_none());
    vm.interpret("1 + 2;").unwrap();
}

#[test]
fn instrumentation_does_not_keep_lox_objects_alive() {
    let mut vm = VM::new();
    let baseline = vm.heap().len();
    vm.set_profiling(true);
    vm.interpret("{ class A {} fun f() { var a = A(); a.self = a; } f(); }")
        .unwrap();
    vm.collect_garbage();
    assert_eq!(vm.heap().len(), baseline);
    assert!(
        vm.profile()
            .unwrap()
            .to_string()
            .contains("OP_SET_PROPERTY")
    );
}
