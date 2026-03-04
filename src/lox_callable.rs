use crate::environment::Environment;
use crate::interpreter::{Interpreter, RuntimeError};
use crate::stmt::Stmt;
use crate::token::Token;
use crate::value::Value;
use std::cell::RefCell;
use std::collections::HashMap;
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
    pub body: Rc<Vec<Stmt>>,
    pub closure: Rc<RefCell<Environment>>,
    pub is_initializer: bool,
}

impl LoxFunction {
    pub fn new(
        declaration_name: String,
        params: Vec<Token>,
        body: Rc<Vec<Stmt>>,
        closure: Rc<RefCell<Environment>>,
        is_initializer: bool,
    ) -> Self {
        LoxFunction {
            declaration_name,
            params,
            body,
            closure,
            is_initializer,
        }
    }

    pub fn bind(&self, instance: Rc<RefCell<LoxInstance>>) -> LoxFunction {
        let env = Rc::new(RefCell::new(Environment::new_enclosed(
            self.closure.clone(),
        )));
        env.borrow_mut()
            .define("this".to_string(), Value::Instance(instance));
        LoxFunction {
            declaration_name: self.declaration_name.clone(),
            params: self.params.clone(),
            body: self.body.clone(),
            closure: env,
            is_initializer: self.is_initializer,
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
        let env = Rc::new(RefCell::new(Environment::new_enclosed(
            self.closure.clone(),
        )));
        for (param, arg) in self.params.iter().zip(arguments) {
            env.borrow_mut().define(param.lexeme.clone(), arg);
        }

        match interpreter.execute_block(self.body.as_slice(), env) {
            Ok(()) => {
                if self.is_initializer {
                    self.closure
                        .borrow()
                        .get_at(0, "this")
                        .ok_or_else(|| RuntimeError::UndefinedVariable("this".into()))
                } else {
                    Ok(Value::Nil)
                }
            }
            Err(RuntimeError::ReturnValue(value)) => {
                if self.is_initializer {
                    self.closure
                        .borrow()
                        .get_at(0, "this")
                        .ok_or_else(|| RuntimeError::UndefinedVariable("this".into()))
                } else {
                    Ok(value)
                }
            }
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

// --- Classes and instances ---

#[derive(Clone)]
pub struct LoxClass {
    pub name: String,
    superclass: Option<Rc<LoxClass>>,
    methods: HashMap<String, Rc<LoxFunction>>,
    static_methods: HashMap<String, Rc<LoxFunction>>,
}

impl LoxClass {
    pub fn new(
        name: String,
        superclass: Option<Rc<LoxClass>>,
        methods: HashMap<String, Rc<LoxFunction>>,
        static_methods: HashMap<String, Rc<LoxFunction>>,
    ) -> Self {
        LoxClass {
            name,
            superclass,
            methods,
            static_methods,
        }
    }

    pub fn find_method(&self, name: &str) -> Option<Rc<LoxFunction>> {
        if let Some(method) = self.methods.get(name) {
            Some(method.clone())
        } else if let Some(superclass) = &self.superclass {
            superclass.find_method(name)
        } else {
            None
        }
    }

    pub fn find_static_method(&self, name: &str) -> Option<Rc<LoxFunction>> {
        if let Some(method) = self.static_methods.get(name) {
            Some(method.clone())
        } else if let Some(superclass) = &self.superclass {
            superclass.find_static_method(name)
        } else {
            None
        }
    }
}

impl LoxCallable for LoxClass {
    fn arity(&self) -> usize {
        self.find_method("init")
            .map(|method| method.arity())
            .unwrap_or(0)
    }

    fn call(
        &self,
        _interpreter: &mut Interpreter,
        _arguments: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let class = Rc::new(self.clone());
        let instance = Rc::new(RefCell::new(LoxInstance::new(class.clone())));
        if let Some(initializer) = class.find_method("init") {
            let bound = initializer.bind(instance.clone());
            bound.call(_interpreter, _arguments)?;
        }
        Ok(Value::Instance(instance))
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct LoxInstance {
    class: Rc<LoxClass>,
    fields: HashMap<String, Value>,
}

impl LoxInstance {
    pub fn new(class: Rc<LoxClass>) -> Self {
        LoxInstance {
            class,
            fields: HashMap::new(),
        }
    }

    pub fn get(instance: Rc<RefCell<LoxInstance>>, name: &str) -> Option<Value> {
        if let Some(value) = instance.borrow().fields.get(name) {
            return Some(value.clone());
        }
        let class = instance.borrow().class.clone();
        if let Some(method) = class.find_method(name) {
            let bound = method.bind(instance);
            return Some(Value::Callable(Rc::new(bound)));
        }
        None
    }

    pub fn set(&mut self, name: String, value: Value) {
        self.fields.insert(name, value);
    }

    pub fn class_name(&self) -> &str {
        &self.class.name
    }
}
