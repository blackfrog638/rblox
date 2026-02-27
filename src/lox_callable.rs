use crate::environment::Environment;
use crate::interpreter::{Interpreter, RuntimeError};
use crate::stmt::Stmt;
use crate::token::Token;
use crate::value::Value;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

pub trait LoxCallable {
    fn arity(&self) -> usize;
    fn call(
        &self,
        interpreter: &mut Interpreter,
        arguments: Vec<Value>,
    ) -> Result<Value, RuntimeError>;
    fn name(&self) -> &str;
}

// --- Native functions ---

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

    fn name(&self) -> &str {
        "clock"
    }
}

// --- User-defined functions ---

pub struct LoxFunction {
    pub declaration_name: String,
    pub params: Vec<Token>,
    pub body: Vec<Stmt>,
    pub closure: Rc<RefCell<Environment>>,
}

impl LoxFunction {
    pub fn new(
        declaration_name: String,
        params: Vec<Token>,
        body: Vec<Stmt>,
        closure: Rc<RefCell<Environment>>,
    ) -> Self {
        LoxFunction {
            declaration_name,
            params,
            body,
            closure,
        }
    }
}

impl LoxCallable for LoxFunction {
    fn arity(&self) -> usize {
        self.params.len()
    }

    fn call(
        &self,
        interpreter: &mut Interpreter,
        arguments: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let env = Rc::new(RefCell::new(Environment::new_enclosed(self.closure.clone())));
        for (param, arg) in self.params.iter().zip(arguments) {
            env.borrow_mut().define(param.lexeme.clone(), arg);
        }

        match interpreter.execute_block(&self.body, env) {
            Ok(()) => Ok(Value::Nil),
            Err(RuntimeError::ReturnValue(value)) => Ok(value),
            Err(err) => Err(err),
        }
    }

    fn name(&self) -> &str {
        &self.declaration_name
    }
}

impl fmt::Display for LoxFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<fn {}>", self.declaration_name)
    }
}
