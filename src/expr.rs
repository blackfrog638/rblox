use crate::token::Token;
use std::fmt;

#[derive(Debug, Clone)]
pub enum Expr {
    Binary {
        left: Box<Expr>,
        operator: Token,
        right: Box<Expr>,
    },
    Grouping {
        expression: Box<Expr>,
    },
    Literal {
        value: Token,
    },
    Logical {
        left: Box<Expr>,
        operator: Token,
        right: Box<Expr>,
    },
    Assign {
        name: Token,
        value: Box<Expr>,
    },
    Unary {
        operator: Token,
        right: Box<Expr>,
    },
    Variable {
        name: Token,
    },
    This {
        keyword: Token,
    },
    Super {
        keyword: Token,
        method: Token,
    },
    Get {
        object: Box<Expr>,
        name: Token,
    },
    Set {
        object: Box<Expr>,
        name: Token,
        value: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        paren: Token,
        arguments: Vec<Expr>,
    },
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Binary {
                left,
                operator,
                right,
            } => write!(f, "({} {} {})", operator.lexeme, left, right),
            Expr::Grouping { expression } => {
                write!(f, "(group {})", expression)
            }
            Expr::Literal { value } => write!(f, "{}", value),
            Expr::Logical {
                left,
                operator,
                right,
            } => write!(f, "({} {} {})", operator.lexeme, left, right),
            Expr::Assign { name, value } => {
                write!(f, "(assign {} {})", name.lexeme, value)
            }
            Expr::Unary { operator, right } => {
                write!(f, "({} {})", operator.lexeme, right)
            }
            Expr::Variable { name } => write!(f, "{}", name.lexeme),
            Expr::This { keyword } => write!(f, "{}", keyword.lexeme),
            Expr::Super { keyword, method } => {
                write!(f, "({}.{})", keyword.lexeme, method.lexeme)
            }
            Expr::Get { object, name } => {
                write!(f, "(get {} {})", object, name.lexeme)
            }
            Expr::Set {
                object,
                name,
                value,
            } => write!(f, "(set {} {} {})", object, name.lexeme, value),
            Expr::Call {
                callee,
                paren: _,
                arguments,
            } => {
                let args = arguments
                    .iter()
                    .map(|arg| arg.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                write!(f, "(call {} {})", callee, args)
            }
        }
    }
}
