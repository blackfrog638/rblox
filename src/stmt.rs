// src/stmt.rs
use crate::expr::Expr;
use crate::token::Token;

#[derive(Debug)]
pub enum Stmt {
    Expression {
        expression: Expr,
    },
    Print {
        expression: Expr,
    },
    Block {
        statements: Vec<Stmt>,
    },
    Var {
        name: Token,
        initializer: Option<Expr>,
    },
}
