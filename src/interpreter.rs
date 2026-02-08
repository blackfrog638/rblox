use crate::expr::Expr;
use crate::token::Literal;
use crate::token_type::TokenType;
use crate::value::Value;

#[derive(Debug)]
pub enum RuntimeError {
    TypeMismatch(String),
    ZeroDivision,
}

pub struct Interpreter;

impl Interpreter {
    pub fn new() -> Self {
        Interpreter
    }

    pub fn evaluate(&self, expr: &Expr) -> Result<Value, RuntimeError> {
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
        }
    }
}
