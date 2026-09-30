use crate::chunk::{
    Chunk, OP_ADD, OP_CALL, OP_CONSTANT, OP_DEFINE_GLOBAL, OP_DIVIDE, OP_EQUAL, OP_FALSE,
    OP_GET_GLOBAL, OP_GET_LOCAL, OP_GET_UPVALUE, OP_GREATER, OP_JUMP, OP_JUMP_IF_FALSE, OP_LESS,
    OP_LOOP, OP_MULTIPLY, OP_NEGATE, OP_NIL, OP_NOT, OP_POP, OP_PRINT, OP_RETURN, OP_SET_GLOBAL,
    OP_SET_LOCAL, OP_SET_UPVALUE, OP_SUBTRACT, OP_TRUE,
};
use std::{cell::RefCell, rc::Rc};

use crate::object::{Closure, ObjFunction, Object, UpvalueDesc, allocate_string};
use crate::scanner::{Scanner, Token, TokenKind};
use crate::value::Value;

pub fn compile(source: &str) -> Result<ObjFunction, String> {
    let mut scanner = Scanner::new(source);
    let mut tokens = Vec::new();

    loop {
        let token = scanner.scan_token();
        tokens.push(token);

        if matches!(token.kind, TokenKind::Eof | TokenKind::Error) {
            break;
        }
    }

    if let Some(token) = tokens.iter().find(|token| token.kind == TokenKind::Error) {
        return Err(format!("Compile error: {}", token.lexeme));
    }

    let mut parser = Parser::new(&tokens, FunctionType::Script, None);

    let mut first_error = None;
    while !matches!(parser.peek().kind, TokenKind::Eof) {
        if let Err(error) = parser.declaration() {
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
    }

    if let Some(error) = first_error {
        return Err(error);
    }

    parser.emit_return();
    Ok(parser.function)
}

struct ParseRule {
    prefix: Option<PrefixRule>,
    infix: Option<InfixRule>,
    precedence: Precedence,
}

struct Local<'a> {
    name: Token<'a>,
    depth: usize,
}

#[derive(Clone)]
struct LocalInfo {
    name: String,
    depth: usize,
}

struct EnclosingCompiler {
    locals: Vec<LocalInfo>,
    upvalues: Rc<RefCell<Vec<UpvalueDesc>>>,
    enclosing: Option<Rc<EnclosingCompiler>>,
}

#[derive(Clone, Copy)]
enum EnclosingBinding {
    Local(u8),
    Upvalue(u8),
}

struct LoopContext {
    continue_target: usize,
    scope_depth: usize,
    break_jumps: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FunctionType {
    Function,
    Script,
}

enum PrefixRule {
    Grouping,
    Unary,
    Number,
    String,
    Literal,
    Identifier,
}

enum InfixRule {
    Binary,
    Call,
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Precedence {
    None,
    Assignment,
    Or,
    And,
    Equality,
    Comparison,
    Term,
    Factor,
    Unary,
    Call,
    Primary,
}

impl Precedence {
    fn next(self) -> Self {
        match self {
            Self::None => Self::Assignment,
            Self::Assignment => Self::Or,
            Self::Or => Self::And,
            Self::And => Self::Equality,
            Self::Equality => Self::Comparison,
            Self::Comparison => Self::Term,
            Self::Term => Self::Factor,
            Self::Factor => Self::Unary,
            Self::Unary => Self::Call,
            Self::Call => Self::Primary,
            Self::Primary => Self::Primary,
        }
    }
}

fn get_rule(kind: TokenKind) -> ParseRule {
    match kind {
        TokenKind::LeftParen => ParseRule {
            prefix: Some(PrefixRule::Grouping),
            infix: Some(InfixRule::Call),
            precedence: Precedence::Call,
        },
        TokenKind::Minus => ParseRule {
            prefix: Some(PrefixRule::Unary),
            infix: Some(InfixRule::Binary),
            precedence: Precedence::Term,
        },
        TokenKind::Plus => ParseRule {
            prefix: None,
            infix: Some(InfixRule::Binary),
            precedence: Precedence::Term,
        },
        TokenKind::Slash | TokenKind::Star => ParseRule {
            prefix: None,
            infix: Some(InfixRule::Binary),
            precedence: Precedence::Factor,
        },
        TokenKind::Bang => ParseRule {
            prefix: Some(PrefixRule::Unary),
            infix: None,
            precedence: Precedence::None,
        },
        TokenKind::BangEqual | TokenKind::EqualEqual => ParseRule {
            prefix: None,
            infix: Some(InfixRule::Binary),
            precedence: Precedence::Equality,
        },
        TokenKind::Greater | TokenKind::GreaterEqual | TokenKind::Less | TokenKind::LessEqual => {
            ParseRule {
                prefix: None,
                infix: Some(InfixRule::Binary),
                precedence: Precedence::Comparison,
            }
        }
        TokenKind::And => ParseRule {
            prefix: None,
            infix: Some(InfixRule::Binary),
            precedence: Precedence::And,
        },
        TokenKind::Or => ParseRule {
            prefix: None,
            infix: Some(InfixRule::Binary),
            precedence: Precedence::Or,
        },
        TokenKind::Number => ParseRule {
            prefix: Some(PrefixRule::Number),
            infix: None,
            precedence: Precedence::None,
        },
        TokenKind::True | TokenKind::False | TokenKind::Nil => ParseRule {
            prefix: Some(PrefixRule::Literal),
            infix: None,
            precedence: Precedence::None,
        },
        TokenKind::String => ParseRule {
            prefix: Some(PrefixRule::String),
            infix: None,
            precedence: Precedence::None,
        },
        TokenKind::Identifier => ParseRule {
            prefix: Some(PrefixRule::Identifier),
            infix: None,
            precedence: Precedence::None,
        },
        _ => ParseRule {
            prefix: None,
            infix: None,
            precedence: Precedence::None,
        },
    }
}

struct Parser<'a> {
    tokens: &'a [Token<'a>],
    current: usize,
    function: ObjFunction,
    function_type: FunctionType,
    panic_mode: bool,
    scope_depth: usize,
    locals: Vec<Local<'a>>,
    upvalues: Rc<RefCell<Vec<UpvalueDesc>>>,
    enclosing: Option<Rc<EnclosingCompiler>>,
    loop_stack: Vec<LoopContext>,
}

impl<'a> Parser<'a> {
    fn new(
        tokens: &'a [Token<'a>],
        function_type: FunctionType,
        enclosing: Option<Rc<EnclosingCompiler>>,
    ) -> Self {
        let mut parser = Self {
            tokens,
            current: 0,
            function: ObjFunction::new(),
            function_type,
            panic_mode: false,
            scope_depth: 0,
            locals: Vec::new(),
            upvalues: Rc::new(RefCell::new(Vec::new())),
            enclosing,
            loop_stack: Vec::new(),
        };
        parser.locals.push(Local {
            name: Token {
                kind: TokenKind::Identifier,
                lexeme: "",
                line: 0,
            },
            depth: 0,
        });
        parser
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.function.chunk
    }

    fn declaration(&mut self) -> Result<(), String> {
        let result = if self.match_token(TokenKind::Fun) {
            self.fun_declaration()
        } else if self.match_token(TokenKind::Var) {
            self.var_declaration()
        } else {
            self.statement()
        };

        if result.is_err() {
            self.panic_mode = true;
        }
        if self.panic_mode {
            self.synchronize();
        }
        result
    }

    fn fun_declaration(&mut self) -> Result<(), String> {
        let name = self.consume_identifier("Expect function name.")?;
        let global = self.declare_variable(name)?;

        if global.is_none() {
            self.mark_initialized();
        }

        let function = self.compile_function(name)?;
        self.emit_constant(Value::Obj(Rc::new(Object::Closure(Closure::new(function)))));

        if let Some(global) = global {
            self.define_variable(global);
        }
        Ok(())
    }

    fn compile_function(&mut self, name: Token<'a>) -> Result<ObjFunction, String> {
        let enclosing = Rc::new(EnclosingCompiler {
            locals: self
                .locals
                .iter()
                .map(|local| LocalInfo {
                    name: local.name.lexeme.to_string(),
                    depth: local.depth,
                })
                .collect(),
            upvalues: Rc::clone(&self.upvalues),
            enclosing: self.enclosing.clone(),
        });
        let mut function_compiler =
            Parser::new(self.tokens, FunctionType::Function, Some(enclosing));
        function_compiler.current = self.current;
        function_compiler.function.name = Some(name.lexeme.to_string());
        function_compiler.begin_scope();

        let result = (|| {
            function_compiler.consume(TokenKind::LeftParen, "Expect '(' after function name.")?;
            if !function_compiler.match_token(TokenKind::RightParen) {
                loop {
                    if function_compiler.function.arity == u8::MAX as usize {
                        return Err(
                            "Compile error: Can't have more than 255 parameters.".to_string()
                        );
                    }
                    function_compiler.function.arity += 1;
                    let parameter =
                        function_compiler.consume_identifier("Expect parameter name.")?;
                    function_compiler.declare_local(parameter)?;
                    function_compiler.mark_initialized();

                    if !function_compiler.match_token(TokenKind::Comma) {
                        break;
                    }
                }
                function_compiler.consume(TokenKind::RightParen, "Expect ')' after parameters.")?;
            }
            function_compiler.consume(TokenKind::LeftBrace, "Expect '{' before function body.")?;
            function_compiler.block()?;
            function_compiler.emit_return();
            Ok(())
        })();

        self.current = function_compiler.current;
        result.map(|()| {
            function_compiler.function.upvalues = function_compiler.upvalues.borrow().clone();
            function_compiler.function.upvalue_count = function_compiler.function.upvalues.len();
            function_compiler.function
        })
    }

    fn var_declaration(&mut self) -> Result<(), String> {
        let name = self.consume_identifier("Expect variable name.")?;
        let global = self.declare_variable(name)?;

        if self.match_token(TokenKind::Equal) {
            self.expression()?;
        } else {
            self.emit(OP_NIL);
        }

        self.consume(
            TokenKind::Semicolon,
            "Expect ';' after variable declaration.",
        )?;

        if let Some(global) = global {
            self.define_variable(global);
        } else {
            self.mark_initialized();
        }
        Ok(())
    }

    fn declare_variable(&mut self, name: Token<'a>) -> Result<Option<u8>, String> {
        if self.scope_depth == 0 {
            Ok(Some(
                self.current_chunk()
                    .add_constant(allocate_string(name.lexeme.to_string())),
            ))
        } else {
            self.declare_local(name)?;
            Ok(None)
        }
    }

    fn consume_identifier(&mut self, message: &str) -> Result<Token<'a>, String> {
        if self.peek().kind != TokenKind::Identifier {
            return Err(format!("Compile error: {}", message));
        }

        Ok(self.advance())
    }

    fn declare_local(&mut self, name: Token<'a>) -> Result<(), String> {
        for local in self.locals.iter().rev() {
            if local.depth < self.scope_depth {
                break;
            }
            if local.name.lexeme == name.lexeme {
                return Err(
                    "Compile error: Already a variable with this name in this scope.".to_string(),
                );
            }
        }

        self.locals.push(Local {
            name,
            depth: usize::MAX,
        });
        Ok(())
    }

    fn mark_initialized(&mut self) {
        if self.scope_depth == 0 {
            return;
        }

        let local = self.locals.last_mut().expect("local must be declared");
        local.depth = self.scope_depth;
    }

    fn resolve_local(&self, name: &str) -> Result<Option<u8>, String> {
        for (index, local) in self.locals.iter().enumerate().rev() {
            if local.name.lexeme == name {
                if local.depth == usize::MAX {
                    return Err(
                        "Compile error: Can't read local variable in its own initializer."
                            .to_string(),
                    );
                }
                let slot = u8::try_from(index).map_err(|_| {
                    "Compile error: Too many local variables in function.".to_string()
                })?;
                return Ok(Some(slot));
            }
        }

        Ok(None)
    }

    fn add_upvalue(
        upvalues: &Rc<RefCell<Vec<UpvalueDesc>>>,
        index: u8,
        is_local: bool,
    ) -> Result<u8, String> {
        let mut upvalues = upvalues.borrow_mut();
        if let Some(existing) = upvalues
            .iter()
            .position(|upvalue| upvalue.index == index && upvalue.is_local == is_local)
        {
            return Ok(existing as u8);
        }

        let slot = u8::try_from(upvalues.len())
            .map_err(|_| "Compile error: Too many closure variables in function.".to_string())?;
        upvalues.push(UpvalueDesc { index, is_local });
        Ok(slot)
    }

    fn resolve_enclosing_binding(
        compiler: &EnclosingCompiler,
        name: &str,
    ) -> Result<Option<EnclosingBinding>, String> {
        for (index, local) in compiler.locals.iter().enumerate().rev() {
            if local.name == name {
                if local.depth == usize::MAX {
                    return Err(
                        "Compile error: Can't read local variable in its own initializer."
                            .to_string(),
                    );
                }
                let slot = u8::try_from(index).map_err(|_| {
                    "Compile error: Too many local variables in function.".to_string()
                })?;
                return Ok(Some(EnclosingBinding::Local(slot)));
            }
        }

        let Some(enclosing) = &compiler.enclosing else {
            return Ok(None);
        };
        let Some(binding) = Self::resolve_enclosing_binding(enclosing, name)? else {
            return Ok(None);
        };
        let (index, is_local) = match binding {
            EnclosingBinding::Local(index) => (index, true),
            EnclosingBinding::Upvalue(index) => (index, false),
        };
        let upvalue = Self::add_upvalue(&compiler.upvalues, index, is_local)?;
        Ok(Some(EnclosingBinding::Upvalue(upvalue)))
    }

    fn resolve_upvalue(&self, name: &str) -> Result<Option<u8>, String> {
        let Some(enclosing) = &self.enclosing else {
            return Ok(None);
        };
        let Some(binding) = Self::resolve_enclosing_binding(enclosing, name)? else {
            return Ok(None);
        };
        let (index, is_local) = match binding {
            EnclosingBinding::Local(index) => (index, true),
            EnclosingBinding::Upvalue(index) => (index, false),
        };
        Self::add_upvalue(&self.upvalues, index, is_local).map(Some)
    }

    fn define_variable(&mut self, global: u8) {
        self.current_chunk().write(OP_DEFINE_GLOBAL, 1);
        self.current_chunk().write(global, 1);
    }

    fn synchronize(&mut self) {
        self.panic_mode = false;

        while !matches!(self.peek().kind, TokenKind::Eof) {
            if self.match_token(TokenKind::Semicolon) {
                return;
            }

            match self.peek().kind {
                TokenKind::Class
                | TokenKind::Fun
                | TokenKind::Var
                | TokenKind::For
                | TokenKind::If
                | TokenKind::Print
                | TokenKind::Return
                | TokenKind::While => return,
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn statement(&mut self) -> Result<(), String> {
        match self.peek().kind {
            TokenKind::Print => {
                self.advance();
                self.print_statement()
            }
            TokenKind::LeftBrace => {
                self.advance();
                self.begin_scope();
                let result = self.block();
                self.end_scope();
                result
            }
            TokenKind::Continue => {
                self.advance();
                self.continue_statement()
            }
            TokenKind::Break => {
                self.advance();
                self.break_statement()
            }
            TokenKind::Return => {
                self.advance();
                self.return_statement()
            }
            TokenKind::If => {
                self.advance();
                self.if_statement()
            }
            TokenKind::While => {
                self.advance();
                self.while_statement()
            }
            TokenKind::For => {
                self.advance();
                self.for_statement()
            }
            _ => self.expression_statement(),
        }
    }

    fn return_statement(&mut self) -> Result<(), String> {
        if self.function_type == FunctionType::Script {
            return Err("Compile error: Can't return from top-level code.".to_string());
        }

        if self.match_token(TokenKind::Semicolon) {
            self.emit(OP_NIL);
        } else {
            self.expression()?;
            self.consume(TokenKind::Semicolon, "Expected ';' after return value.")?;
        }

        self.emit(OP_RETURN);
        Ok(())
    }

    fn if_statement(&mut self) -> Result<(), String> {
        self.consume(TokenKind::LeftParen, "Expected '(' after 'if'.")?;
        self.expression()?;
        self.consume(TokenKind::RightParen, "Expected ')' after condition.")?;

        let then_jump = self.emit_jump(OP_JUMP_IF_FALSE);
        self.emit(OP_POP);
        self.statement()?;

        let else_jump = self.emit_jump(OP_JUMP);

        self.patch_jump(then_jump);
        self.emit(OP_POP);

        if self.match_token(TokenKind::Else) {
            self.statement()?;
        }

        self.patch_jump(else_jump);
        Ok(())
    }

    fn while_statement(&mut self) -> Result<(), String> {
        let loop_start = self.current_chunk().code.len();
        self.loop_stack.push(LoopContext {
            continue_target: loop_start,
            scope_depth: self.scope_depth,
            break_jumps: Vec::new(),
        });

        self.consume(TokenKind::LeftParen, "Expected '(' after 'while'.")?;
        self.expression()?;
        self.consume(TokenKind::RightParen, "Expected ')' after condition.")?;

        let exit_jump = self.emit_jump(OP_JUMP_IF_FALSE);
        self.emit(OP_POP);
        self.statement()?;
        self.emit_loop(loop_start);

        self.patch_jump(exit_jump);
        self.emit(OP_POP);
        let break_jumps = self
            .loop_stack
            .pop()
            .expect("loop context must exist")
            .break_jumps;
        for break_jump in break_jumps {
            self.patch_jump(break_jump);
        }
        Ok(())
    }

    fn continue_statement(&mut self) -> Result<(), String> {
        let Some((scope_depth, continue_target)) = self
            .loop_stack
            .last()
            .map(|loop_context| (loop_context.scope_depth, loop_context.continue_target))
        else {
            return Err("Compile error: 'continue' outside of a loop.".to_string());
        };

        self.consume(TokenKind::Semicolon, "Expected ';' after 'continue'.")?;
        self.emit_loop_cleanup(scope_depth);
        self.emit_loop(continue_target);
        Ok(())
    }

    fn break_statement(&mut self) -> Result<(), String> {
        let Some(loop_scope_depth) = self
            .loop_stack
            .last()
            .map(|loop_context| loop_context.scope_depth)
        else {
            return Err("Compile error: 'break' outside of a loop.".to_string());
        };

        self.consume(TokenKind::Semicolon, "Expected ';' after 'break'.")?;
        self.emit_loop_cleanup(loop_scope_depth);
        let jump = self.emit_jump(OP_JUMP);
        self.loop_stack
            .last_mut()
            .expect("break targets must exist")
            .break_jumps
            .push(jump);
        Ok(())
    }

    fn for_statement(&mut self) -> Result<(), String> {
        self.begin_scope();

        self.consume(TokenKind::LeftParen, "Expected '(' after 'for'.")?;

        if !self.match_token(TokenKind::Semicolon) {
            if self.match_token(TokenKind::Var) {
                self.var_declaration()?;
            } else {
                self.expression()?;
                self.consume(TokenKind::Semicolon, "Expected ';' after initializer.")?;
            }
        }

        let mut loop_start = self.current_chunk().code.len();
        let mut exit_jump = None;
        self.loop_stack.push(LoopContext {
            continue_target: loop_start,
            scope_depth: self.scope_depth,
            break_jumps: Vec::new(),
        });

        if !self.match_token(TokenKind::Semicolon) {
            self.expression()?;
            self.consume(TokenKind::Semicolon, "Expected ';' after loop condition.")?;
            exit_jump = Some(self.emit_jump(OP_JUMP_IF_FALSE));
            self.emit(OP_POP);
        }

        if !self.match_token(TokenKind::RightParen) {
            let body_jump = self.emit_jump(OP_JUMP);
            let increment_start = self.current_chunk().code.len();
            self.expression()?;
            self.emit(OP_POP);
            self.consume(TokenKind::RightParen, "Expected ')' after for clauses.")?;
            self.emit_loop(loop_start);
            self.patch_jump(body_jump);
            loop_start = increment_start;
            self.loop_stack
                .last_mut()
                .expect("loop context must exist")
                .continue_target = loop_start;
        } else {
            self.consume(TokenKind::RightParen, "Expected ')' after for clauses.")?;
        }

        self.statement()?;
        self.emit_loop(loop_start);

        if let Some(exit_jump) = exit_jump {
            self.patch_jump(exit_jump);
            self.emit(OP_POP);
        }

        let break_jumps = self
            .loop_stack
            .pop()
            .expect("loop context must exist")
            .break_jumps;
        for break_jump in break_jumps {
            self.patch_jump(break_jump);
        }
        self.end_scope();
        Ok(())
    }

    fn print_statement(&mut self) -> Result<(), String> {
        self.expression()?;
        self.consume(TokenKind::Semicolon, "Expected ';' after value.")?;
        self.emit(OP_PRINT);
        Ok(())
    }

    fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn block(&mut self) -> Result<(), String> {
        while !matches!(self.peek().kind, TokenKind::RightBrace | TokenKind::Eof) {
            self.declaration()?;
        }

        self.consume(TokenKind::RightBrace, "Expected '}' after block.")
    }

    fn end_scope(&mut self) {
        self.scope_depth -= 1;
        while self
            .locals
            .last()
            .is_some_and(|local| local.depth > self.scope_depth)
        {
            self.emit(OP_POP);
            self.locals.pop();
        }
    }

    fn emit_loop_cleanup(&mut self, target_scope_depth: usize) {
        let cleanup_count = self
            .locals
            .iter()
            .rev()
            .take_while(|local| local.depth > target_scope_depth)
            .count();
        for _ in 0..cleanup_count {
            self.emit(OP_POP);
        }
    }

    fn expression_statement(&mut self) -> Result<(), String> {
        self.expression()?;
        self.consume(TokenKind::Semicolon, "Expected ';' after expression.")?;
        self.emit(OP_POP);
        Ok(())
    }

    fn expression(&mut self) -> Result<(), String> {
        self.parse_precedence(Precedence::Assignment)
    }

    fn parse_precedence(&mut self, precedence: Precedence) -> Result<(), String> {
        let can_assign = precedence <= Precedence::Assignment;
        let token = self.advance();
        let prefix = get_rule(token.kind).prefix.ok_or_else(|| {
            format!(
                "Compile error: expected expression, found '{}'.",
                token.lexeme
            )
        })?;
        match prefix {
            PrefixRule::Grouping => self.parse_grouping()?,
            PrefixRule::Unary => self.parse_unary()?,
            PrefixRule::Number => self.parse_number()?,
            PrefixRule::Literal => self.parse_literal()?,
            PrefixRule::String => self.parse_string()?,
            PrefixRule::Identifier => self.parse_variable_expression(can_assign)?,
        }

        while precedence <= get_rule(self.peek().kind).precedence {
            let infix = self.advance();
            let rule = get_rule(infix.kind);
            match rule.infix.expect("infix rule must have a parser") {
                InfixRule::Binary => self.parse_binary()?,
                InfixRule::Call => self.parse_call()?,
            }
        }

        if can_assign && self.match_token(TokenKind::Equal) {
            self.expression()?;
            return Err("Compile error: Invalid assignment target.".to_string());
        }

        Ok(())
    }

    fn parse_variable_expression(&mut self, can_assign: bool) -> Result<(), String> {
        let name = self.previous().lexeme.to_string();
        let local = self.resolve_local(&name)?;
        let upvalue = if local.is_none() {
            self.resolve_upvalue(&name)?
        } else {
            None
        };
        let global = (local.is_none() && upvalue.is_none()).then(|| {
            self.current_chunk()
                .add_constant(allocate_string(name.clone()))
        });

        if can_assign && self.match_token(TokenKind::Equal) {
            self.expression()?;
            if let Some(slot) = local {
                self.emit(OP_SET_LOCAL);
                self.current_chunk().write(slot, 1);
            } else if let Some(slot) = upvalue {
                self.emit(OP_SET_UPVALUE);
                self.current_chunk().write(slot, 1);
            } else {
                self.emit(OP_SET_GLOBAL);
                self.current_chunk()
                    .write(global.expect("global constant must exist"), 1);
            }
        } else {
            if let Some(slot) = local {
                self.emit(OP_GET_LOCAL);
                self.current_chunk().write(slot, 1);
            } else if let Some(slot) = upvalue {
                self.emit(OP_GET_UPVALUE);
                self.current_chunk().write(slot, 1);
            } else {
                self.emit(OP_GET_GLOBAL);
                self.current_chunk()
                    .write(global.expect("global constant must exist"), 1);
            }
        }

        Ok(())
    }

    fn parse_number(&mut self) -> Result<(), String> {
        let number = self.previous();
        let value = number
            .lexeme
            .parse::<f64>()
            .map_err(|_| format!("Compile error: invalid number '{}'.", number.lexeme))?;
        self.emit_constant(Value::Number(value));
        Ok(())
    }

    fn parse_literal(&mut self) -> Result<(), String> {
        match self.previous().kind {
            TokenKind::True => self.emit(OP_TRUE),
            TokenKind::False => self.emit(OP_FALSE),
            TokenKind::Nil => self.emit(OP_NIL),
            _ => unreachable!(),
        }
        Ok(())
    }

    fn parse_string(&mut self) -> Result<(), String> {
        let string_token = self.previous();
        let string_value =
            string_token.lexeme.to_string()[1..string_token.lexeme.len() - 1].to_string(); // Remove the surrounding quotes
        self.emit_constant(allocate_string(string_value));
        Ok(())
    }

    fn parse_grouping(&mut self) -> Result<(), String> {
        self.expression()?;
        self.consume(TokenKind::RightParen, "Expected ')' after expression.")
    }

    fn parse_unary(&mut self) -> Result<(), String> {
        let operator = self.previous().kind;
        self.parse_precedence(Precedence::Unary)?;
        match operator {
            TokenKind::Minus => self.emit(OP_NEGATE),
            TokenKind::Bang => self.emit(OP_NOT),
            _ => unreachable!(),
        }
        Ok(())
    }

    fn parse_and(&mut self) -> Result<(), String> {
        let end_jump = self.emit_jump(OP_JUMP_IF_FALSE);
        self.emit(OP_POP);
        self.parse_precedence(Precedence::And)?;
        self.patch_jump(end_jump);
        Ok(())
    }

    fn parse_or(&mut self) -> Result<(), String> {
        let else_jump = self.emit_jump(OP_JUMP_IF_FALSE);
        let end_jump = self.emit_jump(OP_JUMP);

        self.patch_jump(else_jump);
        self.emit(OP_POP);
        self.parse_precedence(Precedence::Or)?;

        self.patch_jump(end_jump);
        Ok(())
    }

    fn parse_binary(&mut self) -> Result<(), String> {
        let operator = self.previous().kind;

        match operator {
            TokenKind::Or => return self.parse_or(),
            TokenKind::And => return self.parse_and(),
            _ => {
                let precedence = get_rule(operator).precedence;
                self.parse_precedence(precedence.next())?;
            }
        }

        match operator {
            TokenKind::Plus => self.emit(OP_ADD),
            TokenKind::Minus => self.emit(OP_SUBTRACT),
            TokenKind::Star => self.emit(OP_MULTIPLY),
            TokenKind::Slash => self.emit(OP_DIVIDE),
            TokenKind::EqualEqual => self.emit(OP_EQUAL),
            TokenKind::BangEqual => {
                self.emit(OP_EQUAL);
                self.emit(OP_NOT);
            }
            TokenKind::Greater => self.emit(OP_GREATER),
            TokenKind::GreaterEqual => {
                self.emit(OP_LESS);
                self.emit(OP_NOT);
            }
            TokenKind::Less => self.emit(OP_LESS),
            TokenKind::LessEqual => {
                self.emit(OP_GREATER);
                self.emit(OP_NOT);
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn parse_call(&mut self) -> Result<(), String> {
        let arg_count = self.argument_list()?;
        self.emit(OP_CALL);
        self.emit(arg_count as u8);
        Ok(())
    }

    fn argument_list(&mut self) -> Result<usize, String> {
        let mut arg_count = 0;
        if !self.match_token(TokenKind::RightParen) {
            loop {
                self.expression()?;
                arg_count += 1;
                if arg_count > 255 {
                    return Err("Compile error: Can't have more than 255 arguments.".to_string());
                }
                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
            self.consume(TokenKind::RightParen, "Expected ')' after arguments.")?;
        }
        Ok(arg_count)
    }

    fn emit(&mut self, instruction: u8) {
        self.current_chunk().write(instruction, 1);
    }

    fn emit_constant(&mut self, value: Value) {
        let index = self.current_chunk().add_constant(value);
        self.current_chunk().write(OP_CONSTANT, 1);
        self.current_chunk().write(index, 1);
    }

    fn emit_jump(&mut self, instruction: u8) -> usize {
        self.emit(instruction);
        self.emit(0xff);
        self.emit(0xff);
        self.current_chunk().code.len() - 2
    }

    fn emit_loop(&mut self, loop_start: usize) {
        self.emit(OP_LOOP);
        let offset = self.current_chunk().code.len() - loop_start + 2;
        self.emit(((offset >> 8) & 0xff) as u8);
        self.emit((offset & 0xff) as u8);
    }

    fn patch_jump(&mut self, jump: usize) {
        let offset = self.current_chunk().code.len() - jump - 2;
        self.current_chunk().code[jump] = (offset >> 8) as u8;
        self.current_chunk().code[jump + 1] = (offset & 0xff) as u8;
    }

    fn emit_return(&mut self) {
        self.emit(OP_NIL);
        self.emit(OP_RETURN);
    }

    fn peek(&self) -> &Token<'a> {
        self.tokens.get(self.current).unwrap_or(&Token {
            kind: TokenKind::Eof,
            lexeme: "",
            line: 1,
        })
    }

    fn advance(&mut self) -> Token<'a> {
        let token = *self.peek();
        self.current += 1;
        token
    }

    fn previous(&self) -> &Token<'a> {
        &self.tokens[self.current - 1]
    }

    fn match_token(&mut self, expected: TokenKind) -> bool {
        if self.peek().kind != expected {
            return false;
        }

        self.advance();
        true
    }

    fn consume(&mut self, expected: TokenKind, message: &str) -> Result<(), String> {
        if self.match_token(expected) {
            return Ok(());
        }

        Err(format!("Compile error: {}", message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_returns_top_level_script_function() {
        let function = compile("nil;").expect("script should compile");

        assert_eq!(function.arity, 0);
        assert_eq!(function.name, None);
        assert_eq!(function.chunk.code.last(), Some(&OP_RETURN));
    }

    #[test]
    fn compile_declares_named_function_as_global() {
        let script = compile("fun breakfast() { print \"beignets\"; }")
            .expect("function declaration should compile");

        assert_eq!(
            script.chunk.code,
            vec![OP_CONSTANT, 1, OP_DEFINE_GLOBAL, 0, OP_NIL, OP_RETURN]
        );
        let Value::Obj(function) = &script.chunk.constants[1] else {
            panic!("second constant should be the declared function");
        };
        let Object::Closure(closure) = function.as_ref() else {
            panic!("function constant should be a Closure");
        };
        let function = &closure.function;
        assert_eq!(function.name.as_deref(), Some("breakfast"));
        assert_eq!(function.arity, 0);
        assert_eq!(
            function.chunk.code,
            vec![OP_CONSTANT, 0, OP_PRINT, OP_NIL, OP_RETURN]
        );
    }

    #[test]
    fn compile_assigns_parameter_slots_after_the_function_slot() {
        let script =
            compile("fun sum(a, b) { print a + b; }").expect("function declaration should compile");

        let Value::Obj(function) = &script.chunk.constants[1] else {
            panic!("second constant should be the declared function");
        };
        let Object::Closure(closure) = function.as_ref() else {
            panic!("function constant should be a Closure");
        };
        let function = &closure.function;
        assert_eq!(function.arity, 2);
        assert_eq!(
            function.chunk.code,
            vec![
                OP_GET_LOCAL,
                1,
                OP_GET_LOCAL,
                2,
                OP_ADD,
                OP_PRINT,
                OP_NIL,
                OP_RETURN
            ]
        );
    }

    #[test]
    fn compile_accepts_number_literal() {
        let chunk = compile("3.14;")
            .expect("number literal should compile")
            .chunk;
        assert_eq!(chunk.code.len(), 5);
    }

    #[test]
    fn compile_handles_simple_binary_expression() {
        let chunk = compile("1 + 2;")
            .expect("simple expression should compile")
            .chunk;
        assert_eq!(chunk.code, vec![0, 0, 0, 1, 9, 17, OP_NIL, OP_RETURN]);
    }

    #[test]
    fn compile_respects_operator_precedence() {
        let chunk = compile("1 + 2 * 3;")
            .expect("precedence should compile")
            .chunk;
        assert_eq!(
            chunk.code,
            vec![0, 0, 0, 1, 0, 2, 11, 9, 17, OP_NIL, OP_RETURN]
        );
    }

    #[test]
    fn compile_supports_boolean_and_nil_literals() {
        let chunk = compile("true and false or nil;")
            .expect("boolean and nil should compile")
            .chunk;
        assert!(chunk.code.len() >= 7);
        assert!(chunk.code.contains(&OP_JUMP_IF_FALSE));
    }

    #[test]
    fn compile_supports_continue_statement() {
        let chunk = compile("while (true) { continue; }")
            .expect("continue should compile")
            .chunk;
        assert!(chunk.code.contains(&OP_LOOP));
    }

    #[test]
    fn compile_supports_break_statement() {
        let chunk = compile("while (true) { break; }")
            .expect("break should compile")
            .chunk;
        assert!(chunk.code.contains(&OP_JUMP));
    }

    #[test]
    fn compile_rejects_break_outside_loop() {
        let result = compile("break;");
        assert!(result.is_err());
    }

    #[test]
    fn compile_supports_short_circuit_logic() {
        let chunk = compile("var a = false and (1 / 0); var b = true or (1 / 0);")
            .expect("logical short-circuit should compile")
            .chunk;

        assert!(chunk.code.contains(&OP_JUMP_IF_FALSE));
        assert!(chunk.code.contains(&OP_JUMP));
    }

    #[test]
    fn compile_generates_loop_for_for_statement() {
        let chunk = compile("for (var i = 0; i < 3; i = i + 1) { print i; }")
            .expect("for loop should compile")
            .chunk;

        assert!(chunk.code.contains(&OP_LOOP));
        assert!(chunk.code.contains(&OP_JUMP_IF_FALSE));
    }

    #[test]
    fn compile_supports_equality_and_comparison() {
        let chunk = compile("1 < 2 == true;")
            .expect("comparison should compile")
            .chunk;
        assert_eq!(chunk.code.len(), 10);
    }

    #[test]
    fn compile_defines_global_variable() {
        let chunk = compile("var breakfast = \"beignets\";")
            .expect("variable should compile")
            .chunk;
        assert_eq!(chunk.code, vec![0, 1, 18, 0, OP_NIL, OP_RETURN]);
        assert_eq!(chunk.constants.len(), 2);
    }

    #[test]
    fn compile_reads_and_assigns_global_variable() {
        let chunk = compile("var a = 1; a = 2; print a;")
            .expect("variables should compile")
            .chunk;
        assert_eq!(
            chunk.code,
            vec![0, 1, 18, 0, 0, 3, 20, 2, 17, 19, 4, 15, OP_NIL, OP_RETURN]
        );
    }

    #[test]
    fn assignment_has_lower_precedence_and_is_right_associative() {
        assert!(compile("var a; var b; a * (b = a * b);").is_ok());
        assert!(compile("var a; var b; a = b = 3;").is_ok());
    }

    #[test]
    fn multiplication_cannot_be_an_assignment_target() {
        let error = compile("var a; var b; a * b = 3;").expect_err("invalid target should fail");
        assert!(error.contains("Invalid assignment target."));
    }

    #[test]
    fn compile_uses_local_slots_inside_blocks() {
        let chunk = compile("{ var a = 1; print a; }")
            .expect("local variable should compile")
            .chunk;

        assert_eq!(
            chunk.code,
            vec![
                OP_CONSTANT,
                0,
                OP_GET_LOCAL,
                1,
                OP_PRINT,
                OP_POP,
                OP_NIL,
                OP_RETURN
            ]
        );
    }

    #[test]
    fn compile_resolves_outer_block_locals_and_shadowing() {
        let chunk = compile("{ var a = 1; { print a; var a = 2; print a; } print a; }")
            .expect("nested local variables should compile")
            .chunk;

        assert_eq!(
            chunk.code,
            vec![
                OP_CONSTANT,
                0,
                OP_GET_LOCAL,
                1,
                OP_PRINT,
                OP_CONSTANT,
                1,
                OP_GET_LOCAL,
                2,
                OP_PRINT,
                OP_POP,
                OP_GET_LOCAL,
                1,
                OP_PRINT,
                OP_POP,
                OP_NIL,
                OP_RETURN,
            ]
        );
    }

    #[test]
    fn compile_assigns_local_slots() {
        let chunk = compile("{ var a = 1; a = 2; print a; }")
            .expect("local assignment should compile")
            .chunk;

        assert_eq!(
            chunk.code,
            vec![
                OP_CONSTANT,
                0,
                OP_CONSTANT,
                1,
                OP_SET_LOCAL,
                1,
                OP_POP,
                OP_GET_LOCAL,
                1,
                OP_PRINT,
                OP_POP,
                OP_NIL,
                OP_RETURN,
            ]
        );
    }

    #[test]
    fn compile_generates_loop_for_while_statement() {
        let chunk = compile("var i = 0; while (i < 3) { i = i + 1; }")
            .expect("while loop should compile")
            .chunk;

        assert!(chunk.code.contains(&OP_LOOP));
        assert!(chunk.code.contains(&OP_JUMP_IF_FALSE));
    }

    #[test]
    fn local_variable_cannot_be_read_in_its_initializer() {
        let error = compile("{ var a = a; }").expect_err("self initializer should fail");
        assert!(error.contains("own initializer"));
    }
}
