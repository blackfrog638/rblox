use crate::lox_callable::LoxCallable;
use std::fmt;
use std::rc::Rc;

#[derive(Clone)]
pub enum Value {
    Number(f64),
    Str(String),
    Boolean(bool),
    Nil,
    Callable(Rc<dyn LoxCallable>),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(value) => write!(f, "Number({})", value),
            Value::Str(value) => write!(f, "Str({:?})", value),
            Value::Boolean(value) => write!(f, "Boolean({})", value),
            Value::Nil => write!(f, "Nil"),
            Value::Callable(_) => write!(f, "Callable(<dyn LoxCallable>)"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::Nil, Value::Nil) => true,
            (Value::Callable(a), Value::Callable(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Nil => false,
            Value::Boolean(value) => *value,
            _ => true,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(value) => write!(f, "{}", value),
            Value::Str(value) => write!(f, "{}", value),
            Value::Boolean(value) => write!(f, "{}", value),
            Value::Nil => write!(f, "nil"),
            Value::Callable(_) => write!(f, "<callable>"),
        }
    }
}
