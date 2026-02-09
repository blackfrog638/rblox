// src/stmt.rs
use crate::expr::Expr;

#[derive(Debug)]
pub enum Stmt {
    Expression { expression: Expr },
    Print { expression: Expr },
}
