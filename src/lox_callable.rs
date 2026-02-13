use crate::interpreter::{Interpreter, RuntimeError};
use crate::value::Value;
use std::time::{SystemTime, UNIX_EPOCH};

pub trait LoxCallable {
    fn arity(&self) -> usize;
    fn call(
        &self,
        interpreter: &mut Interpreter,
        arguments: Vec<Value>,
    ) -> Result<Value, RuntimeError>;
}

pub struct NativeClock;

impl LoxCallable for NativeClock {
    fn arity(&self) -> usize {
        0
    }

    fn call(
        &self,
        _interpreter: &mut Interpreter,
        _arguments: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Ok(Value::Number(now.as_secs_f64()))
    }
}
