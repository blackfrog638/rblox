pub struct Scanner<'a> {
    source: &'a str,
    start: usize,
    current: usize,
    line: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenKind {
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Comma,
    Dot,
    Minus,
    Plus,
    Semicolon,
    Slash,
    Star,
    // One or two character tokens.
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    // Literals.
    Identifier,
    String,
    Number,
    // Keywords.
    And,
    Class,
    Else,
    False,
    For,
    Fun,
    If,
    Nil,
    Or,
    Print,
    Return,
    Super,
    This,
    True,
    Var,
    While,

    Error,
    Eof,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token<'a> {
    pub kind: TokenKind,
    pub lexeme: &'a str,
    pub line: usize,
}

impl<'a> Scanner<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            start: 0,
            current: 0,
            line: 1,
        }
    }

    pub fn scan_token(&mut self) -> Token<'a> {
        self.skip_whitespace();
        self.start = self.current;

        if self.is_at_end() {
            return self.make_token(TokenKind::Eof);
        }

        let c = self.advance();

        if is_alpha(c) {
            return self.identifier();
        }

        if c.is_ascii_digit() {
            return self.number();
        }

        match c {
            '(' => self.make_token(TokenKind::LeftParen),
            ')' => self.make_token(TokenKind::RightParen),
            '{' => self.make_token(TokenKind::LeftBrace),
            '}' => self.make_token(TokenKind::RightBrace),
            ';' => self.make_token(TokenKind::Semicolon),
            ',' => self.make_token(TokenKind::Comma),
            '.' => self.make_token(TokenKind::Dot),
            '-' => self.make_token(TokenKind::Minus),
            '+' => self.make_token(TokenKind::Plus),
            '/' => self.make_token(TokenKind::Slash),
            '*' => self.make_token(TokenKind::Star),
            '!' => {
                if self.match_char('=') {
                    self.make_token(TokenKind::BangEqual)
                } else {
                    self.make_token(TokenKind::Bang)
                }
            }
            '=' => {
                if self.match_char('=') {
                    self.make_token(TokenKind::EqualEqual)
                } else {
                    self.make_token(TokenKind::Equal)
                }
            }
            '<' => {
                if self.match_char('=') {
                    self.make_token(TokenKind::LessEqual)
                } else {
                    self.make_token(TokenKind::Less)
                }
            }
            '>' => {
                if self.match_char('=') {
                    self.make_token(TokenKind::GreaterEqual)
                } else {
                    self.make_token(TokenKind::Greater)
                }
            }
            '"' => self.string(),
            _ => self.error_token("Unexpected character."),
        }
    }

    fn skip_whitespace(&mut self) {
        loop {
            match self.peek() {
                Some(' ' | '\r' | '\t') => {
                    self.advance();
                }
                Some('\n') => {
                    self.line += 1;
                    self.advance();
                }
                Some('/') if self.peek_next() == Some('/') => {
                    while self.peek() != Some('\n') && !self.is_at_end() {
                        self.advance();
                    }
                }
                _ => return,
            }
        }
    }

    fn identifier(&mut self) -> Token<'a> {
        while self
            .peek()
            .is_some_and(|c| is_alpha(c) || c.is_ascii_digit())
        {
            self.advance();
        }

        self.make_token(self.identifier_kind())
    }

    fn identifier_kind(&self) -> TokenKind {
        match self.lexeme() {
            "and" => TokenKind::And,
            "class" => TokenKind::Class,
            "else" => TokenKind::Else,
            "false" => TokenKind::False,
            "for" => TokenKind::For,
            "fun" => TokenKind::Fun,
            "if" => TokenKind::If,
            "nil" => TokenKind::Nil,
            "or" => TokenKind::Or,
            "print" => TokenKind::Print,
            "return" => TokenKind::Return,
            "super" => TokenKind::Super,
            "this" => TokenKind::This,
            "true" => TokenKind::True,
            "var" => TokenKind::Var,
            "while" => TokenKind::While,
            _ => TokenKind::Identifier,
        }
    }

    fn number(&mut self) -> Token<'a> {
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
        }

        if self.peek() == Some('.') && self.peek_next().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();

            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
        }

        self.make_token(TokenKind::Number)
    }

    fn string(&mut self) -> Token<'a> {
        while self.peek() != Some('"') && !self.is_at_end() {
            if self.peek() == Some('\n') {
                self.line += 1;
            }
            self.advance();
        }

        if self.is_at_end() {
            return self.error_token("Unterminated string.");
        }

        self.advance();
        self.make_token(TokenKind::String)
    }

    fn make_token(&self, kind: TokenKind) -> Token<'a> {
        Token {
            kind,
            lexeme: self.lexeme(),
            line: self.line,
        }
    }

    fn error_token(&self, message: &'static str) -> Token<'a> {
        Token {
            kind: TokenKind::Error,
            lexeme: message,
            line: self.line,
        }
    }

    fn lexeme(&self) -> &'a str {
        &self.source[self.start..self.current]
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn advance(&mut self) -> char {
        let c = self.peek().expect("advance should not be called at end");
        self.current += c.len_utf8();
        c
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek() != Some(expected) {
            return false;
        }

        self.advance();
        true
    }

    fn peek(&self) -> Option<char> {
        self.source[self.current..].chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        let mut chars = self.source[self.current..].chars();
        chars.next()?;
        chars.next()
    }
}

fn is_alpha(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_all(source: &str) -> Vec<TokenKind> {
        let mut scanner = Scanner::new(source);
        let mut kinds = Vec::new();

        loop {
            let token = scanner.scan_token();
            kinds.push(token.kind);
            if token.kind == TokenKind::Eof {
                break;
            }
        }

        kinds
    }

    #[test]
    fn scans_single_character_tokens() {
        let kinds = scan_all("(){},.-+;/*");

        assert_eq!(
            kinds,
            vec![
                TokenKind::LeftParen,
                TokenKind::RightParen,
                TokenKind::LeftBrace,
                TokenKind::RightBrace,
                TokenKind::Comma,
                TokenKind::Dot,
                TokenKind::Minus,
                TokenKind::Plus,
                TokenKind::Semicolon,
                TokenKind::Slash,
                TokenKind::Star,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn scans_one_or_two_character_tokens() {
        let kinds = scan_all("! != = == > >= < <=");

        assert_eq!(
            kinds,
            vec![
                TokenKind::Bang,
                TokenKind::BangEqual,
                TokenKind::Equal,
                TokenKind::EqualEqual,
                TokenKind::Greater,
                TokenKind::GreaterEqual,
                TokenKind::Less,
                TokenKind::LessEqual,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn skips_whitespace_and_comments() {
        let mut scanner = Scanner::new(" \t\r\n// comment\nvar");

        let token = scanner.scan_token();

        assert_eq!(token.kind, TokenKind::Var);
        assert_eq!(token.lexeme, "var");
        assert_eq!(token.line, 3);
    }

    #[test]
    fn scans_strings_and_numbers() {
        let mut scanner = Scanner::new("\"hello\" 123 45.67");

        let string = scanner.scan_token();
        let int = scanner.scan_token();
        let float = scanner.scan_token();

        assert_eq!(string.kind, TokenKind::String);
        assert_eq!(string.lexeme, "\"hello\"");
        assert_eq!(int.kind, TokenKind::Number);
        assert_eq!(int.lexeme, "123");
        assert_eq!(float.kind, TokenKind::Number);
        assert_eq!(float.lexeme, "45.67");
    }

    #[test]
    fn scans_identifiers_and_keywords() {
        let kinds = scan_all(
            "andy and class false for fun if nil or print return super this true var while _x x2",
        );

        assert_eq!(
            kinds,
            vec![
                TokenKind::Identifier,
                TokenKind::And,
                TokenKind::Class,
                TokenKind::False,
                TokenKind::For,
                TokenKind::Fun,
                TokenKind::If,
                TokenKind::Nil,
                TokenKind::Or,
                TokenKind::Print,
                TokenKind::Return,
                TokenKind::Super,
                TokenKind::This,
                TokenKind::True,
                TokenKind::Var,
                TokenKind::While,
                TokenKind::Identifier,
                TokenKind::Identifier,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn reports_errors() {
        let mut scanner = Scanner::new("@ \"unterminated");

        let unexpected = scanner.scan_token();
        let unterminated = scanner.scan_token();

        assert_eq!(unexpected.kind, TokenKind::Error);
        assert_eq!(unexpected.lexeme, "Unexpected character.");
        assert_eq!(unterminated.kind, TokenKind::Error);
        assert_eq!(unterminated.lexeme, "Unterminated string.");
    }
}
