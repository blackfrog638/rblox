//! Optional Lox instrumentation. Times are wall time, including instrumentation
//! overhead; use a native sampling profiler to locate Rust/CPU hotspots.
use std::collections::HashMap;
use std::fmt::{self, Write};
use std::time::{Duration, Instant};

use crate::ObjId;
use crate::chunk::*;

#[derive(Debug, Default)]
pub struct FunctionProfile {
    pub name: String,
    pub calls: u64,
    pub instructions: u64,
    pub self_time: Duration,
    pub inclusive_time: Duration,
}

struct ActiveCall {
    function: usize,
    started: Instant,
    children: Duration,
}

/// Accumulates runs until `VM::set_profiling` resets or disables it.
/// Function entries identify prototypes, so closures share an entry while
/// unrelated functions with the same name remain separate.
pub struct Profile {
    functions: Vec<FunctionProfile>,
    ids: HashMap<ObjId, usize>,
    active: Vec<ActiveCall>,
    opcodes: [u64; 256],
    elapsed: Duration,
    gc_time: Duration,
    collections: u64,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            functions: Vec::new(),
            ids: HashMap::new(),
            active: Vec::new(),
            opcodes: [0; 256],
            elapsed: Duration::ZERO,
            gc_time: Duration::ZERO,
            collections: 0,
        }
    }
}

impl Profile {
    pub fn functions(&self) -> &[FunctionProfile] {
        &self.functions
    }

    pub fn opcode_counts(&self) -> &[u64; 256] {
        &self.opcodes
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub fn gc_time(&self) -> Duration {
        self.gc_time
    }

    pub fn collections(&self) -> u64 {
        self.collections
    }

    pub(crate) fn enter(&mut self, prototype: ObjId, name: &str) {
        let function = *self.ids.entry(prototype).or_insert_with(|| {
            let index = self.functions.len();
            self.functions.push(FunctionProfile {
                name: name.to_string(),
                ..FunctionProfile::default()
            });
            index
        });
        self.functions[function].calls += 1;
        self.active.push(ActiveCall {
            function,
            started: Instant::now(),
            children: Duration::ZERO,
        });
    }

    pub(crate) fn instruction(&mut self, opcode: u8) {
        self.opcodes[opcode as usize] += 1;
        if let Some(call) = self.active.last() {
            self.functions[call.function].instructions += 1;
        }
    }

    pub(crate) fn exit(&mut self) {
        self.exit_at(Instant::now());
    }

    fn exit_at(&mut self, now: Instant) {
        let Some(call) = self.active.pop() else {
            return;
        };
        let elapsed = now.duration_since(call.started);
        let function = &mut self.functions[call.function];
        function.inclusive_time += elapsed;
        function.self_time += elapsed.saturating_sub(call.children);
        if let Some(parent) = self.active.last_mut() {
            parent.children += elapsed;
        } else {
            self.elapsed += elapsed;
        }
    }

    pub(crate) fn unwind(&mut self) {
        let now = Instant::now();
        while !self.active.is_empty() {
            self.exit_at(now);
        }
    }

    pub(crate) fn collected(&mut self, elapsed: Duration) {
        self.collections += 1;
        self.gc_time += elapsed;
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(output, "Lox profile (instrumented wall time)")?;
        writeln!(
            output,
            "Execution: {:.3} ms; GC: {} collections, {:.3} ms",
            self.elapsed.as_secs_f64() * 1000.0,
            self.collections,
            self.gc_time.as_secs_f64() * 1000.0
        )?;
        writeln!(
            output,
            "GC overlaps function times; final/manual GC is counted separately."
        )?;
        writeln!(
            output,
            "Self excludes child Lox calls; native calls count toward the caller."
        )?;
        writeln!(
            output,
            "\n{:>4} {:<24} {:>10} {:>14} {:>12} {:>14}",
            "id", "function", "calls", "instructions", "self ms", "inclusive ms"
        )?;
        let mut functions: Vec<_> = self.functions.iter().enumerate().collect();
        functions.sort_by_key(|(id, function)| (std::cmp::Reverse(function.self_time), *id));
        for (id, function) in functions {
            writeln!(
                output,
                "{id:>4} {:<24} {:>10} {:>14} {:>12.3} {:>14.3}",
                function.name,
                function.calls,
                function.instructions,
                function.self_time.as_secs_f64() * 1000.0,
                function.inclusive_time.as_secs_f64() * 1000.0
            )?;
        }
        let total: u64 = self.opcodes.iter().sum();
        writeln!(
            output,
            "\nBytecodes: {total} dispatches (including failing instructions)"
        )?;
        writeln!(output, "{:<24} {:>14} {:>10}", "opcode", "count", "share")?;
        let mut opcodes: Vec<_> = self
            .opcodes
            .iter()
            .enumerate()
            .filter(|(_, count)| **count > 0)
            .collect();
        opcodes.sort_by_key(|(opcode, count)| (std::cmp::Reverse(**count), *opcode));
        for (opcode, count) in opcodes {
            writeln!(
                output,
                "{:<24} {count:>14} {:>9.2}%",
                opcode_name(opcode as u8),
                *count as f64 * 100.0 / total as f64
            )?;
        }
        output.write_char('\n')
    }
}

fn opcode_name(opcode: u8) -> &'static str {
    match opcode {
        OP_CONSTANT => "OP_CONSTANT",
        OP_NIL => "OP_NIL",
        OP_TRUE => "OP_TRUE",
        OP_FALSE => "OP_FALSE",
        OP_EQUAL => "OP_EQUAL",
        OP_GREATER => "OP_GREATER",
        OP_LESS => "OP_LESS",
        OP_AND => "OP_AND",
        OP_OR => "OP_OR",
        OP_ADD => "OP_ADD",
        OP_SUBTRACT => "OP_SUBTRACT",
        OP_MULTIPLY => "OP_MULTIPLY",
        OP_DIVIDE => "OP_DIVIDE",
        OP_NOT => "OP_NOT",
        OP_NEGATE => "OP_NEGATE",
        OP_PRINT => "OP_PRINT",
        OP_RETURN => "OP_RETURN",
        OP_POP => "OP_POP",
        OP_DEFINE_GLOBAL => "OP_DEFINE_GLOBAL",
        OP_GET_GLOBAL => "OP_GET_GLOBAL",
        OP_SET_GLOBAL => "OP_SET_GLOBAL",
        OP_GET_LOCAL => "OP_GET_LOCAL",
        OP_SET_LOCAL => "OP_SET_LOCAL",
        OP_JUMP => "OP_JUMP",
        OP_JUMP_IF_FALSE => "OP_JUMP_IF_FALSE",
        OP_LOOP => "OP_LOOP",
        OP_CALL => "OP_CALL",
        OP_GET_UPVALUE => "OP_GET_UPVALUE",
        OP_SET_UPVALUE => "OP_SET_UPVALUE",
        OP_CLOSURE => "OP_CLOSURE",
        OP_CLOSE_UPVALUE => "OP_CLOSE_UPVALUE",
        OP_CLASS => "OP_CLASS",
        OP_GET_PROPERTY => "OP_GET_PROPERTY",
        OP_SET_PROPERTY => "OP_SET_PROPERTY",
        OP_METHOD => "OP_METHOD",
        OP_INVOKE => "OP_INVOKE",
        OP_INHERIT => "OP_INHERIT",
        OP_GET_SUPER => "OP_GET_SUPER",
        OP_SUPER_INVOKE => "OP_SUPER_INVOKE",
        _ => "UNKNOWN",
    }
}
