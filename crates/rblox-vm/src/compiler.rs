use crate::chunk::{
    Chunk, OP_ADD, OP_CONSTANT, OP_DEFINE_GLOBAL, OP_DIVIDE, OP_EQUAL, OP_FALSE, OP_GET_GLOBAL,
    OP_GET_LOCAL, OP_GREATER, OP_JUMP, OP_JUMP_IF_FALSE, OP_LESS, OP_LOOP, OP_MULTIPLY, OP_NEGATE,
    OP_NIL, OP_NOT, OP_POP, OP_PRINT, OP_RETURN, OP_SET_GLOBAL, OP_SET_LOCAL, OP_SUBTRACT, OP_TRUE,
    Value, allocate_string,
};
use crate::scanner::{Scanner, Token, TokenKind};

pub fn compile(source: &str) -> Result<Chunk, String> {
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

    let mut parser = Parser::new(&tokens);

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
    Ok(parser.chunk)
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
            Self::Unary => Self::Primary,
            Self::Primary => Self::Primary,
        }
    }
}

fn get_rule(kind: TokenKind) -> ParseRule {
    match kind {
        TokenKind::LeftParen => ParseRule {
            prefix: Some(PrefixRule::Grouping),
            infix: None,
            precedence: Precedence::None,
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
    chunk: Chunk,
    panic_mode: bool,
    scope_depth: usize,
    locals: Vec<Local<'a>>,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token<'a>]) -> Self {
        Self {
            tokens,
            current: 0,
            chunk: Chunk::new(),
            panic_mode: false,
            scope_depth: 0,
            locals: Vec::new(),
        }
    }

    fn declaration(&mut self) -> Result<(), String> {
        let result = if self.match_token(TokenKind::Var) {
            self.var_declaration()
        } else {
            let result = self.statement();
            result
        };

        if result.is_err() {
            self.panic_mode = true;
        }
        if self.panic_mode {
            self.synchronize();
        }
        result
    }

    fn var_declaration(&mut self) -> Result<(), String> {
        let name = self.consume_identifier("Expect variable name.")?;
        let global = (self.scope_depth == 0).then(|| {
            self.chunk
                .add_constant(allocate_string(name.lexeme.to_string()))
        });

        if self.scope_depth > 0 {
            self.declare_local(name)?;
        }

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

    fn define_variable(&mut self, global: u8) {
        self.chunk.write(OP_DEFINE_GLOBAL, 1);
        self.chunk.write(global, 1);
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
            TokenKind::If => {
                self.advance();
                self.if_statement()
            }
            TokenKind::While => {
                self.advance();
                self.while_statement()
            }
            _ => self.expression_statement(),
        }
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
        let loop_start = self.chunk.code.len();

        self.consume(TokenKind::LeftParen, "Expected '(' after 'while'.")?;
        self.expression()?;
        self.consume(TokenKind::RightParen, "Expected ')' after condition.")?;

        let exit_jump = self.emit_jump(OP_JUMP_IF_FALSE);
        self.emit(OP_POP);
        self.statement()?;
        self.emit_loop(loop_start);

        self.patch_jump(exit_jump);
        self.emit(OP_POP);
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
        let global = local
            .is_none()
            .then(|| self.chunk.add_constant(allocate_string(name.clone())));

        if can_assign && self.match_token(TokenKind::Equal) {
            self.expression()?;
            if let Some(slot) = local {
                self.emit(OP_SET_LOCAL);
                self.chunk.write(slot, 1);
            } else {
                self.emit(OP_SET_GLOBAL);
                self.chunk
                    .write(global.expect("global constant must exist"), 1);
            }
        } else {
            if let Some(slot) = local {
                self.emit(OP_GET_LOCAL);
                self.chunk.write(slot, 1);
            } else {
                self.emit(OP_GET_GLOBAL);
                self.chunk
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

    fn emit(&mut self, instruction: u8) {
        self.chunk.write(instruction, 1);
    }

    fn emit_constant(&mut self, value: Value) {
        let index = self.chunk.add_constant(value);
        self.chunk.write(OP_CONSTANT, 1);
        self.chunk.write(index, 1);
    }

    fn emit_jump(&mut self, instruction: u8) -> usize {
        self.emit(instruction);
        self.emit(0xff);
        self.emit(0xff);
        self.chunk.code.len() - 2
    }

    fn emit_loop(&mut self, loop_start: usize) {
        self.emit(OP_LOOP);
        let offset = self.chunk.code.len() - loop_start + 2;
        self.emit(((offset >> 8) & 0xff) as u8);
        self.emit((offset & 0xff) as u8);
    }

    fn patch_jump(&mut self, jump: usize) {
        let offset = self.chunk.code.len() - jump - 2;
        self.chunk.code[jump] = (offset >> 8) as u8;
        self.chunk.code[jump + 1] = (offset & 0xff) as u8;
    }

    fn emit_return(&mut self) {
        self.chunk.write(OP_RETURN, 1);
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
    fn compile_accepts_number_literal() {
        let chunk = compile("3.14;").expect("number literal should compile");
        assert_eq!(chunk.code.len(), 4);
    }

    #[test]
    fn compile_handles_simple_binary_expression() {
        let chunk = compile("1 + 2;").expect("simple expression should compile");
        assert_eq!(chunk.code, vec![0, 0, 0, 1, 9, 17, 16]);
    }

    #[test]
    fn compile_respects_operator_precedence() {
        let chunk = compile("1 + 2 * 3;").expect("precedence should compile");
        assert_eq!(chunk.code, vec![0, 0, 0, 1, 0, 2, 11, 9, 17, 16]);
    }

    #[test]
    fn compile_supports_boolean_and_nil_literals() {
        let chunk = compile("true and false or nil;").expect("boolean and nil should compile");
        assert!(chunk.code.len() >= 7);
        assert!(chunk.code.contains(&OP_JUMP_IF_FALSE));
    }

    #[test]
    fn compile_supports_short_circuit_logic() {
        let chunk = compile("var a = false and (1 / 0); var b = true or (1 / 0);")
            .expect("logical short-circuit should compile");

        assert!(chunk.code.contains(&OP_JUMP_IF_FALSE));
        assert!(chunk.code.contains(&OP_JUMP));
    }

    #[test]
    fn compile_supports_equality_and_comparison() {
        let chunk = compile("1 < 2 == true;").expect("comparison should compile");
        assert_eq!(chunk.code.len(), 9);
    }

    #[test]
    fn compile_defines_global_variable() {
        let chunk = compile("var breakfast = \"beignets\";").expect("variable should compile");
        assert_eq!(chunk.code, vec![0, 1, 18, 0, 16]);
        assert_eq!(chunk.constants.len(), 2);
    }

    #[test]
    fn compile_reads_and_assigns_global_variable() {
        let chunk = compile("var a = 1; a = 2; print a;").expect("variables should compile");
        assert_eq!(
            chunk.code,
            vec![0, 1, 18, 0, 0, 3, 20, 2, 17, 19, 4, 15, 16]
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
        let chunk = compile("{ var a = 1; print a; }").expect("local variable should compile");

        assert_eq!(
            chunk.code,
            vec![OP_CONSTANT, 0, OP_GET_LOCAL, 0, OP_PRINT, OP_POP, OP_RETURN]
        );
    }

    #[test]
    fn compile_resolves_outer_block_locals_and_shadowing() {
        let chunk = compile("{ var a = 1; { print a; var a = 2; print a; } print a; }")
            .expect("nested local variables should compile");

        assert_eq!(
            chunk.code,
            vec![
                OP_CONSTANT,
                0,
                OP_GET_LOCAL,
                0,
                OP_PRINT,
                OP_CONSTANT,
                1,
                OP_GET_LOCAL,
                1,
                OP_PRINT,
                OP_POP,
                OP_GET_LOCAL,
                0,
                OP_PRINT,
                OP_POP,
                OP_RETURN,
            ]
        );
    }

    #[test]
    fn compile_assigns_local_slots() {
        let chunk =
            compile("{ var a = 1; a = 2; print a; }").expect("local assignment should compile");

        assert_eq!(
            chunk.code,
            vec![
                OP_CONSTANT,
                0,
                OP_CONSTANT,
                1,
                OP_SET_LOCAL,
                0,
                OP_POP,
                OP_GET_LOCAL,
                0,
                OP_PRINT,
                OP_POP,
                OP_RETURN,
            ]
        );
    }

    #[test]
    fn compile_generates_loop_for_while_statement() {
        let chunk =
            compile("var i = 0; while (i < 3) { i = i + 1; }").expect("while loop should compile");

        assert!(chunk.code.contains(&OP_LOOP));
        assert!(chunk.code.contains(&OP_JUMP_IF_FALSE));
    }

    #[test]
    fn local_variable_cannot_be_read_in_its_initializer() {
        let error = compile("{ var a = a; }").expect_err("self initializer should fail");
        assert!(error.contains("own initializer"));
    }
}
