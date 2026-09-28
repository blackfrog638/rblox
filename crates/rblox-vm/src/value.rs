use std::{fmt, rc::Rc};

use crate::object::{ObjFunction, ObjType, Object};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Nil,
    Obj(Rc<Object>),
}

impl Value {
    pub fn as_function(&self) -> Option<&ObjFunction> {
        match self.as_obj()? {
            Object::Function(function) => Some(function),
            Object::String { .. } => None,
        }
    }

    pub fn is_obj_type(&self, obj_type: ObjType) -> bool {
        match self {
            Value::Obj(obj) => obj.obj_type() == obj_type,
            _ => false,
        }
    }

    pub fn as_obj(&self) -> Option<&Object> {
        match self {
            Value::Obj(obj) => Some(obj.as_ref()),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&String> {
        match self.as_obj()? {
            Object::String { value, .. } => Some(value),
            Object::Function(_) => None,
        }
    }
}

pub fn is_obj_type(value: &Value, obj_type: ObjType) -> bool {
    value.is_obj_type(obj_type)
}

pub fn as_string(value: &Value) -> Option<&String> {
    value.as_string()
}

pub fn as_str(value: &Value) -> Option<&str> {
    value.as_string().map(String::as_str)
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(value) => write!(f, "{}", value),
            Value::Bool(value) => write!(f, "{}", value),
            Value::Nil => write!(f, "nil"),
            Value::Obj(value) => write!(f, "{}", value),
        }
    }
}
