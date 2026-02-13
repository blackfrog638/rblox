use crate::token::Token;

#[derive(Debug)]
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
            Expr::Call {
                callee,
                paren,
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
