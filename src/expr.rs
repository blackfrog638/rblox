use crate::token::Token;

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

impl Expr {
    pub fn to_string(&self) -> String {
        match self {
            Expr::Binary {
                left,
                operator,
                right,
            } => {
                format!(
                    "({} {} {})",
                    operator.lexeme,
                    left.to_string(),
                    right.to_string()
                )
            }
            Expr::Grouping { expression } => {
                format!("(group {})", expression.to_string())
            }
            Expr::Literal { value } => value.to_string(),
            Expr::Logical {
                left,
                operator,
                right,
            } => {
                format!(
                    "({} {} {})",
                    operator.lexeme,
                    left.to_string(),
                    right.to_string()
                )
            }
            Expr::Assign { name, value } => {
                format!("(assign {} {})", name.lexeme, value.to_string())
            }
            Expr::Unary { operator, right } => {
                format!("({} {})", operator.lexeme, right.to_string())
            }
            Expr::Variable { name } => name.lexeme.clone(),
            Expr::This { keyword } => keyword.lexeme.clone(),
            Expr::Super { keyword, method } => {
                format!("({}.{})", keyword.lexeme, method.lexeme)
            }
            Expr::Get { object, name } => {
                format!("(get {} {})", object.to_string(), name.lexeme)
            }
            Expr::Set {
                object,
                name,
                value,
            } => format!(
                "(set {} {} {})",
                object.to_string(),
                name.lexeme,
                value.to_string()
            ),
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
                format!("(call {} {})", callee.to_string(), args)
            }
        }
    }
}
