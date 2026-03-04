// src/stmt.rs
use crate::expr::Expr;
use crate::token::Token;
use std::rc::Rc;

#[derive(Debug, Clone)]
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
    Class {
        name: Token,
        superclass: Option<Expr>,
        methods: Rc<Vec<Stmt>>,
        static_methods: Rc<Vec<Stmt>>,
    },
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    While {
        condition: Expr,
        body: Box<Stmt>,
    },
    Function {
        name: Token,
        params: Vec<Token>,
        body: Rc<Vec<Stmt>>,
    },
    Return {
        keyword: Token,
        value: Option<Expr>,
    },
}
