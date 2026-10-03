use rblox_vm::chunk::{
    OP_ADD, OP_GET_GLOBAL, OP_GET_LOCAL, OP_GET_UPVALUE, OP_NIL, OP_POP, OP_PRINT, OP_RETURN,
    OP_SET_UPVALUE,
};
use rblox_vm::{Heap, ObjFunction, UpvalueDesc, compile};

fn child<'a>(parent: &ObjFunction, name: &str, heap: &'a Heap) -> &'a ObjFunction {
    parent
        .chunk
        .constants
        .iter()
        .filter_map(|value| value.as_function(heap))
        .find(|function| function.name.as_deref() == Some(name))
        .expect("named function must be compiled into its parent")
}

fn captures(function: &ObjFunction) -> Vec<(u8, bool)> {
    assert_eq!(function.upvalue_count, function.upvalues.len());
    function
        .upvalues
        .iter()
        .map(|UpvalueDesc { index, is_local }| (*index, *is_local))
        .collect()
}

#[test]
fn deep_capture_forwards_through_middle_and_reuses_slots_for_assignment() {
    let mut heap = Heap::new();
    let script = compile(
        "fun outer() {
            var a = 1; var b = 2;
            fun middle() {
                var c = 3; var d = 4;
                fun inner() {
                    print a + c + b + d;
                    a = a + d;
                }
            }
        }",
        &mut heap,
    )
    .unwrap();
    let outer = child(&script, "outer", &heap);
    let middle = child(outer, "middle", &heap);
    let inner = child(middle, "inner", &heap);

    assert!(captures(outer).is_empty());
    assert_eq!(captures(middle), vec![(1, true), (2, true)]);
    assert_eq!(
        captures(inner),
        vec![(0, false), (1, true), (1, false), (2, true)]
    );
    assert_eq!(
        inner.chunk.code,
        vec![
            OP_GET_UPVALUE,
            0,
            OP_GET_UPVALUE,
            1,
            OP_ADD,
            OP_GET_UPVALUE,
            2,
            OP_ADD,
            OP_GET_UPVALUE,
            3,
            OP_ADD,
            OP_PRINT,
            OP_GET_UPVALUE,
            0,
            OP_GET_UPVALUE,
            3,
            OP_ADD,
            OP_SET_UPVALUE,
            0,
            OP_POP,
            OP_NIL,
            OP_RETURN,
        ]
    );
}

#[test]
fn shadowing_declaration_order_and_sibling_functions_keep_separate_bindings() {
    let mut heap = Heap::new();
    let script = compile(
        "fun outer() {
            var x = 1;
            fun middle() {
                fun before() { print x; }
                var x = 2;
                fun after() { print x; }
                fun parameter(x) { print x; }
            }
            fun sibling() { print x; print later; }
            var later = 3;
        }",
        &mut heap,
    )
    .unwrap();
    let outer = child(&script, "outer", &heap);
    let middle = child(outer, "middle", &heap);
    let parameter = child(middle, "parameter", &heap);
    let sibling = child(outer, "sibling", &heap);

    assert_eq!(captures(middle), vec![(1, true)]);
    assert_eq!(captures(child(middle, "before", &heap)), vec![(0, false)]);
    assert_eq!(captures(child(middle, "after", &heap)), vec![(2, true)]);
    assert!(captures(parameter).is_empty());
    assert_eq!(
        parameter.chunk.code,
        vec![OP_GET_LOCAL, 1, OP_PRINT, OP_NIL, OP_RETURN]
    );
    assert_eq!(captures(sibling), vec![(1, true)]);
    // A declaration after the function cannot become its lexical capture.
    assert_eq!(
        sibling.chunk.code,
        vec![
            OP_GET_UPVALUE,
            0,
            OP_PRINT,
            OP_GET_GLOBAL,
            0,
            OP_PRINT,
            OP_NIL,
            OP_RETURN
        ]
    );
    assert_eq!(
        sibling.chunk.constants[0]
            .as_string(&heap)
            .map(String::as_str),
        Some("later")
    );
}

#[test]
fn nested_functions_have_their_own_loop_and_return_contexts() {
    let mut heap = Heap::new();
    for statement in ["break;", "continue;"] {
        let source = format!("while (true) {{ fun inner() {{ {statement} }} }}");
        assert!(
            compile(&source, &mut heap)
                .unwrap_err()
                .contains("outside of a loop")
        );
    }
    assert!(
        compile("fun inner() { return; } return;", &mut heap)
            .unwrap_err()
            .contains("top-level")
    );
    assert!(
        compile(
            "while (true) {
            fun inner() { while (true) { break; } return; }
            continue;
        }",
            &mut heap
        )
        .is_ok()
    );
}
