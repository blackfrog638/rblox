use crate::environment::Environment;
use crate::expr::Expr;
use crate::lox_callable::LoxCallable;
use crate::lox_callable::LoxClass;
use crate::lox_callable::NativeClock;
use crate::stmt::Stmt;
use crate::token::Literal;
use crate::token::Token;
use crate::token_type::TokenType;
use crate::value::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::lox_callable::LoxFunction;
use std::collections::HashMap as StdHashMap;

#[derive(Debug)]
pub enum RuntimeError {
    TypeMismatch(String),
    ZeroDivision,
    UndefinedVariable(String),
    UndefinedProperty(String),
    ReturnValue(Value),
}

pub struct Interpreter {
    environment: Rc<RefCell<Environment>>,
    locals: HashMap<usize, usize>,
}

impl Interpreter {
    pub fn new() -> Self {
        let environment = Rc::new(RefCell::new(Environment::new()));
        environment
            .borrow_mut()
            .define("clock".to_string(), Value::Callable(Rc::new(NativeClock)));
        Interpreter {
            environment,
            locals: HashMap::new(),
        }
    }

    pub fn interpret(&mut self, statements: &[Stmt]) -> Result<(), RuntimeError> {
        for statement in statements {
            self.execute(statement)?;
        }
        Ok(())
    }

    fn execute(&mut self, statement: &Stmt) -> Result<(), RuntimeError> {
        match statement {
            Stmt::Expression { expression } => {
                self.evaluate(expression)?;
                Ok(())
            }
            Stmt::Print { expression } => {
                let value = self.evaluate(expression)?;
                println!("{}", value);
                Ok(())
            }
            Stmt::Block { statements } => {
                let new_environment = Rc::new(RefCell::new(Environment::new_enclosed(
                    self.environment.clone(),
                )));
                self.execute_block(statements, new_environment)
            }
            Stmt::Var { name, initializer } => {
                let value = match initializer {
                    Some(expr) => self.evaluate(expr)?,
                    None => Value::Nil,
                };
                self.environment
                    .borrow_mut()
                    .define(name.lexeme.clone(), value);
                Ok(())
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition_value = self.evaluate(condition)?;
                if condition_value.is_truthy() {
                    self.execute(then_branch)
                } else if let Some(else_branch) = else_branch {
                    self.execute(else_branch)
                } else {
                    Ok(())
                }
            }
            Stmt::While { condition, body } => {
                while self.evaluate(condition)?.is_truthy() {
                    self.execute(body)?;
                }
                Ok(())
            }
            Stmt::Function { name, params, body } => {
                let function = LoxFunction::new(
                    name.lexeme.clone(),
                    params.clone(),
                    body.clone(),
                    self.environment.clone(),
                    false,
                );
                self.environment
                    .borrow_mut()
                    .define(name.lexeme.clone(), Value::Callable(Rc::new(function)));
                Ok(())
            }
            Stmt::Class {
                name,
                superclass,
                methods,
                static_methods,
            } => {
                let superclass_class = if let Some(superclass_expr) = superclass {
                    let value = self.evaluate(superclass_expr)?;
                    match value {
                        Value::Class(class) => Some(class),
                        _ => {
                            return Err(RuntimeError::TypeMismatch(
                                "Superclass must be a class.".into(),
                            ));
                        }
                    }
                } else {
                    None
                };

                self.environment
                    .borrow_mut()
                    .define(name.lexeme.clone(), Value::Nil);
                let mut method_map: StdHashMap<String, Rc<LoxFunction>> = StdHashMap::new();
                let mut static_method_map: StdHashMap<String, Rc<LoxFunction>> = StdHashMap::new();
                for method in methods {
                    if let Stmt::Function {
                        name: method_name,
                        params,
                        body,
                    } = method
                    {
                        let is_initializer = method_name.lexeme == "init";
                        let function = LoxFunction::new(
                            method_name.lexeme.clone(),
                            params.clone(),
                            body.clone(),
                            self.environment.clone(),
                            is_initializer,
                        );
                        method_map.insert(method_name.lexeme.clone(), Rc::new(function));
                    }
                }
                for method in static_methods {
                    if let Stmt::Function {
                        name: method_name,
                        params,
                        body,
                    } = method
                    {
                        let function = LoxFunction::new(
                            method_name.lexeme.clone(),
                            params.clone(),
                            body.clone(),
                            self.environment.clone(),
                            false,
                        );
                        static_method_map.insert(method_name.lexeme.clone(), Rc::new(function));
                    }
                }
                let class = LoxClass::new(
                    name.lexeme.clone(),
                    superclass_class,
                    method_map,
                    static_method_map,
                );
                self.environment
                    .borrow_mut()
                    .assign(&name.lexeme, Value::Class(Rc::new(class)));
                Ok(())
            }
            Stmt::Return { keyword: _, value } => {
                let return_value = match value {
                    Some(expr) => self.evaluate(expr)?,
                    None => Value::Nil,
                };
                Err(RuntimeError::ReturnValue(return_value))
            }
        }
    }

    pub fn execute_block(
        &mut self,
        statements: &[Stmt],
        environment: Rc<RefCell<Environment>>,
    ) -> Result<(), RuntimeError> {
        let previous = self.environment.clone();
        self.environment = environment;

        let result = statements
            .iter()
            .try_for_each(|statement| self.execute(statement));

        self.environment = previous;
        result
    }

    pub fn evaluate(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        let expr_ptr = expr as *const Expr as usize;
        match expr {
            Expr::Binary {
                left,
                operator,
                right,
            } => {
                let left_value = self.evaluate(left)?;
                let right_value = self.evaluate(right)?;
                match (left_value, operator.token_type.clone(), right_value) {
                    (Value::Number(n1), TokenType::Minus, Value::Number(n2)) => {
                        Ok(Value::Number(n1 - n2))
                    }
                    (Value::Number(n1), TokenType::Slash, Value::Number(n2)) => {
                        if n2 == 0.0 {
                            return Err(RuntimeError::ZeroDivision);
                        }
                        Ok(Value::Number(n1 / n2))
                    }
                    (Value::Number(n1), TokenType::Star, Value::Number(n2)) => {
                        Ok(Value::Number(n1 * n2))
                    }

                    (Value::Number(n1), TokenType::Plus, Value::Number(n2)) => {
                        Ok(Value::Number(n1 + n2))
                    }
                    (Value::Str(s1), TokenType::Plus, Value::Str(s2)) => {
                        Ok(Value::Str(format!("{}{}", s1, s2)))
                    }

                    (Value::Number(n1), TokenType::Greater, Value::Number(n2)) => {
                        Ok(Value::Boolean(n1 > n2))
                    }
                    (Value::Number(n1), TokenType::GreaterEqual, Value::Number(n2)) => {
                        Ok(Value::Boolean(n1 >= n2))
                    }
                    (Value::Number(n1), TokenType::Less, Value::Number(n2)) => {
                        Ok(Value::Boolean(n1 < n2))
                    }
                    (Value::Number(n1), TokenType::LessEqual, Value::Number(n2)) => {
                        Ok(Value::Boolean(n1 <= n2))
                    }

                    (v1, TokenType::EqualEqual, v2) => Ok(Value::Boolean(v1 == v2)),
                    (v1, TokenType::BangEqual, v2) => Ok(Value::Boolean(v1 != v2)),

                    (_, TokenType::Plus, _) => Err(RuntimeError::TypeMismatch(
                        "Operands must be two numbers or two strings.".into(),
                    )),
                    (_, TokenType::Minus, _)
                    | (_, TokenType::Star, _)
                    | (_, TokenType::Slash, _) => Err(RuntimeError::TypeMismatch(
                        "Operands must be numbers.".into(),
                    )),

                    _ => unreachable!(),
                }
            }
            Expr::Grouping { expression } => self.evaluate(expression),
            Expr::Literal { value } => match &value.literal {
                Some(Literal::Number(number)) => Ok(Value::Number(*number)),
                Some(Literal::Str(text)) => Ok(Value::Str(text.clone())),
                Some(Literal::Bool(value)) => Ok(Value::Boolean(*value)),
                Some(Literal::Nil) => Ok(Value::Nil),
                Some(Literal::Identifier(name)) => Ok(Value::Str(name.clone())),
                None => match value.token_type {
                    TokenType::True => Ok(Value::Boolean(true)),
                    TokenType::False => Ok(Value::Boolean(false)),
                    TokenType::Nil => Ok(Value::Nil),
                    _ => Err(RuntimeError::TypeMismatch("Expected literal value.".into())),
                },
            },
            Expr::Logical {
                left,
                operator,
                right,
            } => {
                let left_value = self.evaluate(left)?;

                match operator.token_type {
                    TokenType::Or => {
                        if left_value.is_truthy() {
                            Ok(left_value)
                        } else {
                            self.evaluate(right)
                        }
                    }
                    TokenType::And => {
                        if !left_value.is_truthy() {
                            Ok(left_value)
                        } else {
                            self.evaluate(right)
                        }
                    }
                    _ => unreachable!(),
                }
            }
            Expr::Variable { name } => self.lookup_variable(name, expr_ptr),
            Expr::Assign { name, value } => {
                let evaluated = self.evaluate(value)?;
                if let Some(distance) = self.locals.get(&expr_ptr) {
                    if self.environment.borrow_mut().assign_at(
                        *distance,
                        &name.lexeme,
                        evaluated.clone(),
                    ) {
                        Ok(evaluated)
                    } else {
                        Err(RuntimeError::UndefinedVariable(name.lexeme.clone()))
                    }
                } else if self
                    .environment
                    .borrow_mut()
                    .assign(&name.lexeme, evaluated.clone())
                {
                    Ok(evaluated)
                } else {
                    Err(RuntimeError::UndefinedVariable(name.lexeme.clone()))
                }
            }
            Expr::Get { object, name } => {
                let object_value = self.evaluate(object)?;
                match object_value {
                    Value::Instance(instance) => {
                        crate::lox_callable::LoxInstance::get(instance, &name.lexeme)
                            .ok_or_else(|| RuntimeError::UndefinedProperty(name.lexeme.clone()))
                    }
                    Value::Class(class) => class
                        .find_static_method(&name.lexeme)
                        .map(|method| Value::Callable(method))
                        .ok_or_else(|| RuntimeError::UndefinedProperty(name.lexeme.clone())),
                    _ => Err(RuntimeError::TypeMismatch(
                        "Only instances and classes have properties.".into(),
                    )),
                }
            }
            Expr::Set {
                object,
                name,
                value,
            } => {
                let object_value = self.evaluate(object)?;
                match object_value {
                    Value::Instance(instance) => {
                        let evaluated = self.evaluate(value)?;
                        instance
                            .borrow_mut()
                            .set(name.lexeme.clone(), evaluated.clone());
                        Ok(evaluated)
                    }
                    _ => Err(RuntimeError::TypeMismatch(
                        "Only instances have fields.".into(),
                    )),
                }
            }
            Expr::This { keyword } => self.lookup_variable(keyword, expr_ptr),
            Expr::Unary { operator, right } => {
                let right_value = self.evaluate(right)?;
                match operator.token_type {
                    TokenType::Minus => match right_value {
                        Value::Number(number) => Ok(Value::Number(-number)),
                        _ => Err(RuntimeError::TypeMismatch(
                            "Operand must be a number.".into(),
                        )),
                    },
                    TokenType::Bang => Ok(Value::Boolean(!right_value.is_truthy())),
                    _ => unreachable!(),
                }
            }
            Expr::Call {
                callee,
                paren: _,
                arguments,
            } => {
                let callee_value = self.evaluate(callee)?;

                let mut evaluated_args = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    evaluated_args.push(self.evaluate(argument)?);
                }

                match callee_value {
                    Value::Callable(callable) => {
                        let expected = callable.arity();
                        let got = evaluated_args.len();
                        if expected != got {
                            return Err(RuntimeError::TypeMismatch(format!(
                                "Expected {} arguments but got {}.",
                                expected, got
                            )));
                        }
                        callable.call(self, evaluated_args)
                    }
                    Value::Class(class) => {
                        let expected = class.arity();
                        let got = evaluated_args.len();
                        if expected != got {
                            return Err(RuntimeError::TypeMismatch(format!(
                                "Expected {} arguments but got {}.",
                                expected, got
                            )));
                        }
                        class.call(self, evaluated_args)
                    }
                    _ => Err(RuntimeError::TypeMismatch(
                        "Can only call functions and classes.".into(),
                    )),
                }
            }
        }
    }

    pub fn resolve(&mut self, expr: &Expr, depth: usize) {
        let expr_ptr = expr as *const Expr as usize;
        self.locals.insert(expr_ptr, depth);
    }

    fn lookup_variable(&self, name: &Token, expr_ptr: usize) -> Result<Value, RuntimeError> {
        if let Some(distance) = self.locals.get(&expr_ptr) {
            self.environment
                .borrow()
                .get_at(*distance, &name.lexeme)
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.lexeme.clone()))
        } else {
            self.environment
                .borrow()
                .get(&name.lexeme)
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.lexeme.clone()))
        }
    }
}
