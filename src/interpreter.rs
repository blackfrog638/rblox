use crate::environment::Environment;
use crate::expr::Expr;
use crate::lox_callable::NativeClock;
use crate::stmt::Stmt;
use crate::token::Literal;
use crate::token_type::TokenType;
use crate::value::Value;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug)]
pub enum RuntimeError {
    TypeMismatch(String),
    ZeroDivision,
    UndefinedVariable(String),
}

pub struct Interpreter {
    environment: Rc<RefCell<Environment>>,
}

impl Interpreter {
    pub fn new() -> Self {
        let environment = Rc::new(RefCell::new(Environment::new()));
        environment
            .borrow_mut()
            .define("clock".to_string(), Value::Callable(Rc::new(NativeClock)));
        Interpreter { environment }
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
        }
    }

    fn execute_block(
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
                    (Value::Number(n1), TokenType::Less, Value::Number(n2)) => {
                        Ok(Value::Boolean(n1 < n2))
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
            Expr::Variable { name } => match self.environment.borrow().get(&name.lexeme) {
                Some(value) => Ok(value),
                None => Err(RuntimeError::UndefinedVariable(name.lexeme.clone())),
            },
            Expr::Assign { name, value } => {
                let evaluated = self.evaluate(value)?;
                if self
                    .environment
                    .borrow_mut()
                    .assign(&name.lexeme, evaluated.clone())
                {
                    Ok(evaluated)
                } else {
                    Err(RuntimeError::UndefinedVariable(name.lexeme.clone()))
                }
            }
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
                paren,
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
                    _ => Err(RuntimeError::TypeMismatch(
                        "Can only call functions and classes.".into(),
                    )),
                }
            }
        }
    }
}
