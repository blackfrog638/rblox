use std::collections::HashMap;
use std::hash::Hash;

use crate::error;
use crate::token::Literal;
use crate::token::Token;
use crate::token_type::TokenType;

struct Scanner {
    source: Vec<char>,
    tokens: Vec<Token>,

    start: usize,
    current: usize,
    line: usize,

    keywords: HashMap<&'static str, TokenType>,
}

impl Scanner {
    pub fn new(source: &str) -> Self {
        Scanner {
            source: source.chars().collect(),
            tokens: Vec::new(),
            start: 0,
            current: 0,
            line: 1,
            keywords: {
                HashMap::from([
                    ("and", TokenType::And),
                    ("class", TokenType::Class),
                    ("else", TokenType::Else),
                    ("false", TokenType::False),
                    ("for", TokenType::For),
                    ("fun", TokenType::Fun),
                    ("if", TokenType::If),
                    ("nil", TokenType::Nil),
                    ("or", TokenType::Or),
                    ("print", TokenType::Print),
                    ("return", TokenType::Return),
                    ("super", TokenType::Super),
                    ("this", TokenType::This),
                    ("true", TokenType::True),
                    ("var", TokenType::Var),
                    ("while", TokenType::While),
                ])
            },
        }
    }

    pub fn scan_tokens(&mut self) -> &Vec<Token> {
        loop {
            if self.is_at_end() {
                break;
            }

            self.start = self.current;
            self.scan_token();
        }
        &self.tokens
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn scan_token(&mut self) {
        let c = self.advance();
        match c {
            '(' => self.add_token(TokenType::LeftParen, None),
            ')' => self.add_token(TokenType::RightParen, None),
            '{' => self.add_token(TokenType::LeftBrace, None),
            '}' => self.add_token(TokenType::RightBrace, None),
            ',' => self.add_token(TokenType::Comma, None),
            '.' => self.add_token(TokenType::Dot, None),
            '-' => self.add_token(TokenType::Minus, None),
            '+' => self.add_token(TokenType::Plus, None),
            ';' => self.add_token(TokenType::Semicolon, None),
            '*' => self.add_token(TokenType::Star, None),
            '!' => {
                if self.match_char('=') {
                    self.add_token(TokenType::BangEqual, None)
                } else {
                    self.add_token(TokenType::Bang, None)
                }
            }
            '=' => {
                if self.match_char('=') {
                    self.add_token(TokenType::EqualEqual, None)
                } else {
                    self.add_token(TokenType::Equal, None)
                }
            }
            '>' => {
                if self.match_char('=') {
                    self.add_token(TokenType::GreaterEqual, None)
                } else {
                    self.add_token(TokenType::Greater, None)
                }
            }
            '<' => {
                if self.match_char('=') {
                    self.add_token(TokenType::LessEqual, None)
                } else {
                    self.add_token(TokenType::Less, None)
                }
            }
            '/' => {
                if self.match_char('/') {
                    // A comment goes until the end of the line.
                    while self.peek() != '\n' && !self.is_at_end() {
                        self.advance();
                    }
                } else {
                    self.add_token(TokenType::Slash, None);
                }
            }
            ' ' | '\r' | '\t' => {
                // Ignore whitespace.
            }
            '\n' => {
                self.line += 1;
            }
            '\"' => self.string(),
            '0'..='9' => {
                self.number();
            }
            'A'..='Z' | 'a'..='z' | '_' => {
                self.identifier();
            }
            _ => {
                error(self.line, "Unexpected character");
            }
        }
    }

    fn advance(&mut self) -> char {
        let c = self.source[self.current];
        self.current += 1;
        c
    }

    fn add_token(&mut self, token_type: TokenType, literal: Option<Literal>) {
        let text: String = self.source[self.start..self.current].iter().collect();
        let token = Token::new(token_type, text, literal, self.line);
        self.tokens.push(token);
    }

    fn peek(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.source[self.current]
        }
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end() {
            return false;
        }
        if self.source[self.current] != expected {
            return false;
        }
        self.current += 1;
        true
    }

    fn string(&mut self) {
        loop {
            if self.peek() == '\"' || self.is_at_end() {
                break;
            }
            if self.peek() == '\n' {
                self.line += 1;
            }
            self.advance();
        }
        if self.is_at_end() {
            error(self.line, "Unterminated string.");
            return;
        }
        self.advance();
        let value: String = self.source[self.start + 1..self.current - 1]
            .iter()
            .collect();
        self.add_token(TokenType::String, Some(Literal::Str(value)));
    }

    fn number(&mut self) {
        while self.peek().is_ascii_digit() {
            self.advance();
        }
        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            self.advance();
            while self.peek().is_ascii_digit() {
                self.advance();
            }
        }
        let value: String = self.source[self.start..self.current].iter().collect();
        let number_value: f64 = value.parse().unwrap();
        self.add_token(TokenType::Number, Some(Literal::Number(number_value)));
    }

    fn peek_next(&self) -> char {
        if self.current + 1 >= self.source.len() {
            '\0'
        } else {
            self.source[self.current + 1]
        }
    }

    fn identifier(&mut self) {
        while self.peek().is_ascii_alphanumeric() || self.peek() == '_' {
            self.advance();
        }
        let text: String = self.source[self.start..self.current].iter().collect();
        let token_type: TokenType = match self.keywords.get(text.as_str()) {
            Some(token_type) => token_type.clone(),
            None => TokenType::Identifier,
        };
        self.add_token(token_type, None);
    }
}
