use crate::chunk::{
    Chunk, OP_ADD, OP_AND, OP_CONSTANT, OP_DIVIDE, OP_EQUAL, OP_FALSE, OP_GREATER, OP_LESS,
    OP_MULTIPLY, OP_NEGATE, OP_NIL, OP_NOT, OP_OR, OP_PRINT, OP_RETURN, OP_SUBTRACT, OP_TRUE,
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

    loop {
        parser.declaration()?;
        if matches!(parser.peek().kind, TokenKind::Eof) {
            break;
        }
    }

    parser.emit_return();
    Ok(parser.chunk)
}

struct ParseRule {
    prefix: Option<PrefixRule>,
    infix: Option<InfixRule>,
    precedence: Precedence,
}

enum PrefixRule {
    Grouping,
    Unary,
    Number,
    String,
    Literal,
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
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token<'a>]) -> Self {
        Self {
            tokens,
            current: 0,
            chunk: Chunk::new(),
        }
    }

    fn declaration(&mut self) -> Result<(), String> {
        self.statement()
    }

    fn statement(&mut self) -> Result<(), String> {
        let token = self.peek();
        match token.kind {
            TokenKind::Print => {
                self.advance();
                self.print_statement()
            }
            _ => self.expression_statement(),
        }
    }

    fn print_statement(&mut self) -> Result<(), String> {
        self.expression()?;
        self.consume(TokenKind::Semicolon, "Expected ';' after value.")?;
        self.emit(OP_PRINT);
        Ok(())
    }

    fn expression_statement(&mut self) -> Result<(), String> {
        self.expression()?;
        self.consume(TokenKind::Semicolon, "Expected ';' after expression.")?;
        Ok(())
    }

    fn expression(&mut self) -> Result<(), String> {
        self.parse_precedence(Precedence::Assignment)
    }

    fn parse_precedence(&mut self, precedence: Precedence) -> Result<(), String> {
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
        }

        while precedence <= get_rule(self.peek().kind).precedence {
            let infix = self.advance();
            let rule = get_rule(infix.kind);
            match rule.infix.expect("infix rule must have a parser") {
                InfixRule::Binary => self.parse_binary()?,
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

    fn parse_binary(&mut self) -> Result<(), String> {
        let operator = self.previous().kind;
        let precedence = get_rule(operator).precedence;
        self.parse_precedence(precedence.next())?;

        match operator {
            TokenKind::Or => self.emit(OP_OR),
            TokenKind::And => self.emit(OP_AND),
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

    fn emit_constant(&mut self, value: Value) {
        let index = self.chunk.add_constant(value);
        self.chunk.write(OP_CONSTANT, 1);
        self.chunk.write(index, 1);
    }

    fn emit_return(&mut self) {
        self.chunk.write(OP_RETURN, 1);
    }

    fn emit(&mut self, instruction: u8) {
        self.chunk.write(instruction, 1);
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

    fn consume(&mut self, expected: TokenKind, message: &str) -> Result<(), String> {
        if self.peek().kind == expected {
            self.current += 1;
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
        let chunk = compile("3.14").expect("number literal should compile");
        assert_eq!(chunk.code.len(), 3);
    }

    #[test]
    fn compile_handles_simple_binary_expression() {
        let chunk = compile("1 + 2").expect("simple expression should compile");
        assert_eq!(chunk.code, vec![0, 0, 0, 1, 9, 16]);
    }

    #[test]
    fn compile_respects_operator_precedence() {
        let chunk = compile("1 + 2 * 3").expect("precedence should compile");
        assert_eq!(chunk.code, vec![0, 0, 0, 1, 0, 2, 11, 9, 16]);
    }

    #[test]
    fn compile_supports_boolean_and_nil_literals() {
        let chunk = compile("true and false or nil").expect("boolean and nil should compile");
        assert_eq!(chunk.code.len(), 6);
    }

    #[test]
    fn compile_supports_equality_and_comparison() {
        let chunk = compile("1 < 2 == true").expect("comparison should compile");
        assert_eq!(chunk.code.len(), 8);
    }
}
