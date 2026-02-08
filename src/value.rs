use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Str(String),
    Boolean(bool),
    Nil,
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
        }
    }
}
