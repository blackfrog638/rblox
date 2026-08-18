use crate::chunk::{
    Chunk, OP_ADD, OP_CONSTANT, OP_DIVIDE, OP_MULTIPLY, OP_NEGATE, OP_RETURN, OP_SUBTRACT, Value,
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
    parser.expression()?;
    parser.consume(TokenKind::Eof, "Expected end of expression.")?;
    parser.emit_return();
    Ok(parser.chunk)
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

    fn expression(&mut self) -> Result<(), String> {
        self.parse_additive()
    }

    fn parse_additive(&mut self) -> Result<(), String> {
        self.parse_multiplicative()?;

        while matches!(self.peek().kind, TokenKind::Plus | TokenKind::Minus) {
            let operator = self.advance().kind;
            self.parse_multiplicative()?;

            match operator {
                TokenKind::Plus => self.emit(OP_ADD),
                TokenKind::Minus => self.emit(OP_SUBTRACT),
                _ => unreachable!(),
            }
        }

        Ok(())
    }

    fn parse_multiplicative(&mut self) -> Result<(), String> {
        self.parse_unary()?;

        while matches!(self.peek().kind, TokenKind::Star | TokenKind::Slash) {
            let operator = self.advance().kind;
            self.parse_unary()?;

            match operator {
                TokenKind::Star => self.emit(OP_MULTIPLY),
                TokenKind::Slash => self.emit(OP_DIVIDE),
                _ => unreachable!(),
            }
        }

        Ok(())
    }

    fn parse_unary(&mut self) -> Result<(), String> {
        if matches!(self.peek().kind, TokenKind::Minus) {
            self.advance();
            self.parse_unary()?;
            self.emit(OP_NEGATE);
            return Ok(());
        }

        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<(), String> {
        match self.peek().kind {
            TokenKind::Number => {
                let number = self.advance();
                let value = number
                    .lexeme
                    .parse::<f64>()
                    .map_err(|_| format!("Compile error: invalid number '{}'.", number.lexeme))?;
                self.emit_constant(Value::Number(value));
                Ok(())
            }
            TokenKind::LeftParen => {
                self.advance();
                self.expression()?;
                self.consume(TokenKind::RightParen, "Expected ')' after expression.")?;
                Ok(())
            }
            _ => Err(format!(
                "Compile error: expected expression, found '{}'.",
                self.peek().lexeme
            )),
        }
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
        assert_eq!(chunk.code, vec![0, 0, 0, 1, 1, 6]);
    }

    #[test]
    fn compile_respects_operator_precedence() {
        let chunk = compile("1 + 2 * 3").expect("precedence should compile");
        assert_eq!(chunk.code, vec![0, 0, 0, 1, 0, 2, 3, 1, 6]);
    }
}
