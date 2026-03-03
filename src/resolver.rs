use crate::expr::Expr;
use crate::interpreter::Interpreter;
use crate::stmt::Stmt;
use crate::token::Token;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
pub struct ResolveError {
    line: usize,
    message: String,
}

impl ResolveError {
    fn new(token: &Token, message: &str) -> Self {
        ResolveError {
            line: token.line,
            message: message.to_string(),
        }
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[line {}] Error: {}", self.line, self.message)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FunctionType {
    None,
    Function,
}

pub struct Resolver<'a> {
    interpreter: &'a mut Interpreter,
    scopes: Vec<HashMap<String, bool>>,
    current_function: FunctionType,
}

impl<'a> Resolver<'a> {
    pub fn new(interpreter: &'a mut Interpreter) -> Self {
        Resolver {
            interpreter,
            scopes: Vec::new(),
            current_function: FunctionType::None,
        }
    }

    pub fn resolve_statements(&mut self, statements: &[Stmt]) -> Result<(), ResolveError> {
        for statement in statements {
            self.resolve_stmt(statement)?;
        }
        Ok(())
    }

    fn resolve_stmt(&mut self, stmt: &Stmt) -> Result<(), ResolveError> {
        match stmt {
            Stmt::Block { statements } => {
                self.begin_scope();
                self.resolve_statements(statements)?;
                self.end_scope();
            }
            Stmt::Var { name, initializer } => {
                self.declare(name)?;
                if let Some(expr) = initializer {
                    self.resolve_expr(expr)?;
                }
                self.define(name);
            }
            Stmt::Class { name } => {
                self.declare(name)?;
                self.define(name);
            }
            Stmt::Function { name, params, body } => {
                self.declare(name)?;
                self.define(name);
                self.resolve_function(params, body)?;
            }
            Stmt::Expression { expression } => {
                self.resolve_expr(expression)?;
            }
            Stmt::Print { expression } => {
                self.resolve_expr(expression)?;
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.resolve_expr(condition)?;
                self.resolve_stmt(then_branch)?;
                if let Some(else_branch) = else_branch {
                    self.resolve_stmt(else_branch)?;
                }
            }
            Stmt::While { condition, body } => {
                self.resolve_expr(condition)?;
                self.resolve_stmt(body)?;
            }
            Stmt::Return { keyword, value } => {
                if self.current_function == FunctionType::None {
                    return Err(ResolveError::new(
                        keyword,
                        "Can't return from top-level code.",
                    ));
                }
                if let Some(expr) = value {
                    self.resolve_expr(expr)?;
                }
            }
        }
        Ok(())
    }

    fn resolve_expr(&mut self, expr: &Expr) -> Result<(), ResolveError> {
        match expr {
            Expr::Variable { name } => {
                if let Some(scope) = self.scopes.last() {
                    if matches!(scope.get(&name.lexeme), Some(false)) {
                        return Err(ResolveError::new(
                            name,
                            "Can't read local variable in its own initializer.",
                        ));
                    }
                }
                self.resolve_local(expr, name);
            }
            Expr::Assign { name, value } => {
                self.resolve_expr(value)?;
                self.resolve_local(expr, name);
            }
            Expr::Get { object, .. } => {
                self.resolve_expr(object)?;
            }
            Expr::Set { object, value, .. } => {
                self.resolve_expr(value)?;
                self.resolve_expr(object)?;
            }
            Expr::Binary { left, right, .. } => {
                self.resolve_expr(left)?;
                self.resolve_expr(right)?;
            }
            Expr::Call {
                callee, arguments, ..
            } => {
                self.resolve_expr(callee)?;
                for argument in arguments {
                    self.resolve_expr(argument)?;
                }
            }
            Expr::Grouping { expression } => {
                self.resolve_expr(expression)?;
            }
            Expr::Logical { left, right, .. } => {
                self.resolve_expr(left)?;
                self.resolve_expr(right)?;
            }
            Expr::Unary { right, .. } => {
                self.resolve_expr(right)?;
            }
            Expr::Literal { .. } => {}
        }
        Ok(())
    }

    fn resolve_function(&mut self, params: &[Token], body: &[Stmt]) -> Result<(), ResolveError> {
        let enclosing = self.current_function;
        self.current_function = FunctionType::Function;
        self.begin_scope();
        for param in params {
            self.declare(param)?;
            self.define(param);
        }
        self.resolve_statements(body)?;
        self.end_scope();
        self.current_function = enclosing;
        Ok(())
    }

    fn begin_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn end_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: &Token) -> Result<(), ResolveError> {
        if let Some(scope) = self.scopes.last_mut() {
            if scope.contains_key(&name.lexeme) {
                return Err(ResolveError::new(
                    name,
                    "Already a variable with this name in this scope.",
                ));
            }
            scope.insert(name.lexeme.clone(), false);
        }
        Ok(())
    }

    fn define(&mut self, name: &Token) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.lexeme.clone(), true);
        }
    }

    fn resolve_local(&mut self, expr: &Expr, name: &Token) {
        for (index, scope) in self.scopes.iter().rev().enumerate() {
            if scope.contains_key(&name.lexeme) {
                self.interpreter.resolve(expr, index);
                return;
            }
        }
    }
}
