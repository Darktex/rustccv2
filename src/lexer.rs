//! Lexer/tokenizer for a C language subset.

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    IntLiteral(i64),
    StringLiteral(String),

    // Identifiers and keywords
    Identifier(String),
    Int,
    Char,
    Void,
    Return,
    If,
    Else,
    While,
    Do,
    For,
    Break,
    Continue,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Ampersand,
    Pipe,
    Caret,
    Tilde,
    Bang,
    AmpersandAmpersand,
    PipePipe,
    EqualEqual,
    BangEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Equal,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    PercentEqual,
    PlusPlus,
    MinusMinus,
    LessLess,
    GreaterGreater,

    // Delimiters
    OpenParen,
    CloseParen,
    OpenBrace,
    CloseBrace,
    OpenBracket,
    CloseBracket,
    Semicolon,
    Comma,
    Question,
    Colon,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::IntLiteral(n) => write!(f, "{n}"),
            Token::StringLiteral(s) => write!(f, "\"{s}\""),
            Token::Identifier(name) => write!(f, "{name}"),
            Token::Int => write!(f, "int"),
            Token::Char => write!(f, "char"),
            Token::Void => write!(f, "void"),
            Token::Return => write!(f, "return"),
            Token::If => write!(f, "if"),
            Token::Else => write!(f, "else"),
            Token::While => write!(f, "while"),
            Token::Do => write!(f, "do"),
            Token::For => write!(f, "for"),
            Token::Break => write!(f, "break"),
            Token::Continue => write!(f, "continue"),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Percent => write!(f, "%"),
            Token::Ampersand => write!(f, "&"),
            Token::Pipe => write!(f, "|"),
            Token::Caret => write!(f, "^"),
            Token::Tilde => write!(f, "~"),
            Token::Bang => write!(f, "!"),
            Token::AmpersandAmpersand => write!(f, "&&"),
            Token::PipePipe => write!(f, "||"),
            Token::EqualEqual => write!(f, "=="),
            Token::BangEqual => write!(f, "!="),
            Token::Less => write!(f, "<"),
            Token::Greater => write!(f, ">"),
            Token::LessEqual => write!(f, "<="),
            Token::GreaterEqual => write!(f, ">="),
            Token::Equal => write!(f, "="),
            Token::PlusEqual => write!(f, "+="),
            Token::MinusEqual => write!(f, "-="),
            Token::StarEqual => write!(f, "*="),
            Token::SlashEqual => write!(f, "/="),
            Token::PercentEqual => write!(f, "%="),
            Token::PlusPlus => write!(f, "++"),
            Token::MinusMinus => write!(f, "--"),
            Token::LessLess => write!(f, "<<"),
            Token::GreaterGreater => write!(f, ">>"),
            Token::OpenParen => write!(f, "("),
            Token::CloseParen => write!(f, ")"),
            Token::OpenBrace => write!(f, "{{"),
            Token::CloseBrace => write!(f, "}}"),
            Token::OpenBracket => write!(f, "["),
            Token::CloseBracket => write!(f, "]"),
            Token::Semicolon => write!(f, ";"),
            Token::Comma => write!(f, ","),
            Token::Question => write!(f, "?"),
            Token::Colon => write!(f, ":"),
        }
    }
}

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        while self.pos < self.input.len() {
            self.skip_whitespace_and_comments();
            if self.pos >= self.input.len() {
                break;
            }
            let token = self.next_token()?;
            tokens.push(token);
        }
        Ok(tokens)
    }

    fn current(&self) -> char {
        self.input[self.pos]
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> char {
        let c = self.input[self.pos];
        self.pos += 1;
        c
    }

    fn skip_whitespace_and_comments(&mut self) {
        while self.pos < self.input.len() {
            let c = self.current();
            if c.is_ascii_whitespace() {
                self.pos += 1;
            } else if c == '/' && self.peek() == Some('/') {
                while self.pos < self.input.len() && self.current() != '\n' {
                    self.pos += 1;
                }
            } else if c == '/' && self.peek() == Some('*') {
                self.pos += 2;
                while self.pos + 1 < self.input.len() {
                    if self.current() == '*' && self.peek() == Some('/') {
                        self.pos += 2;
                        break;
                    }
                    self.pos += 1;
                }
            } else if c == '#' {
                // Skip preprocessor directives (simple handling)
                while self.pos < self.input.len() && self.current() != '\n' {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, String> {
        let c = self.current();

        if c.is_ascii_digit() {
            return self.lex_number();
        }
        if c.is_ascii_alphabetic() || c == '_' {
            return Ok(self.lex_identifier());
        }
        if c == '"' {
            return self.lex_string();
        }
        if c == '\'' {
            return self.lex_char_as_int();
        }

        self.advance();
        match c {
            '+' => self.two_char_token(
                &[('=', Token::PlusEqual), ('+', Token::PlusPlus)],
                Token::Plus,
            ),
            '-' => self.two_char_token(
                &[('=', Token::MinusEqual), ('-', Token::MinusMinus)],
                Token::Minus,
            ),
            '*' => self.two_char_token(&[('=', Token::StarEqual)], Token::Star),
            '/' => self.two_char_token(&[('=', Token::SlashEqual)], Token::Slash),
            '%' => self.two_char_token(&[('=', Token::PercentEqual)], Token::Percent),
            '&' => self.two_char_token(&[('&', Token::AmpersandAmpersand)], Token::Ampersand),
            '|' => self.two_char_token(&[('|', Token::PipePipe)], Token::Pipe),
            '^' => Ok(Token::Caret),
            '~' => Ok(Token::Tilde),
            '!' => self.two_char_token(&[('=', Token::BangEqual)], Token::Bang),
            '=' => self.two_char_token(&[('=', Token::EqualEqual)], Token::Equal),
            '<' => self.two_char_token(
                &[('=', Token::LessEqual), ('<', Token::LessLess)],
                Token::Less,
            ),
            '>' => self.two_char_token(
                &[('=', Token::GreaterEqual), ('>', Token::GreaterGreater)],
                Token::Greater,
            ),
            '(' => Ok(Token::OpenParen),
            ')' => Ok(Token::CloseParen),
            '{' => Ok(Token::OpenBrace),
            '}' => Ok(Token::CloseBrace),
            '[' => Ok(Token::OpenBracket),
            ']' => Ok(Token::CloseBracket),
            ';' => Ok(Token::Semicolon),
            ',' => Ok(Token::Comma),
            '?' => Ok(Token::Question),
            ':' => Ok(Token::Colon),
            _ => Err(format!("Unexpected character: '{c}'")),
        }
    }

    fn two_char_token(
        &mut self,
        options: &[(char, Token)],
        default: Token,
    ) -> Result<Token, String> {
        if self.pos < self.input.len() {
            for (ch, tok) in options {
                if self.current() == *ch {
                    self.advance();
                    return Ok(tok.clone());
                }
            }
        }
        Ok(default)
    }

    fn lex_number(&mut self) -> Result<Token, String> {
        let mut num_str = String::new();

        if self.current() == '0' && self.pos + 1 < self.input.len() {
            match self.input[self.pos + 1] {
                'x' | 'X' => {
                    self.advance();
                    self.advance();
                    while self.pos < self.input.len() && self.current().is_ascii_hexdigit() {
                        num_str.push(self.advance());
                    }
                    self.skip_int_suffix();
                    let val = i64::from_str_radix(&num_str, 16)
                        .map_err(|e| format!("Invalid hex literal: {e}"))?;
                    return Ok(Token::IntLiteral(val));
                }
                '0'..='7' => {
                    while self.pos < self.input.len()
                        && self.current().is_ascii_digit()
                        && self.current() <= '7'
                    {
                        num_str.push(self.advance());
                    }
                    self.skip_int_suffix();
                    let val = i64::from_str_radix(&num_str, 8)
                        .map_err(|e| format!("Invalid octal literal: {e}"))?;
                    return Ok(Token::IntLiteral(val));
                }
                _ => {}
            }
        }

        while self.pos < self.input.len() && self.current().is_ascii_digit() {
            num_str.push(self.advance());
        }
        self.skip_int_suffix();

        let val: i64 = num_str
            .parse()
            .map_err(|e| format!("Invalid integer literal: {e}"))?;
        Ok(Token::IntLiteral(val))
    }

    fn skip_int_suffix(&mut self) {
        while self.pos < self.input.len() && matches!(self.current(), 'l' | 'L' | 'u' | 'U') {
            self.advance();
        }
    }

    fn lex_identifier(&mut self) -> Token {
        let mut name = String::new();
        while self.pos < self.input.len()
            && (self.current().is_ascii_alphanumeric() || self.current() == '_')
        {
            name.push(self.advance());
        }
        match name.as_str() {
            "int" => Token::Int,
            "char" => Token::Char,
            "void" => Token::Void,
            "return" => Token::Return,
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            "do" => Token::Do,
            "for" => Token::For,
            "break" => Token::Break,
            "continue" => Token::Continue,
            _ => Token::Identifier(name),
        }
    }

    fn lex_string(&mut self) -> Result<Token, String> {
        self.advance(); // consume opening "
        let mut s = String::new();
        while self.pos < self.input.len() && self.current() != '"' {
            if self.current() == '\\' {
                self.advance();
                if self.pos >= self.input.len() {
                    return Err("Unterminated string literal".to_string());
                }
                s.push(self.decode_escape());
            } else {
                s.push(self.advance());
            }
        }
        if self.pos >= self.input.len() {
            return Err("Unterminated string literal".to_string());
        }
        self.advance(); // consume closing "
        Ok(Token::StringLiteral(s))
    }

    fn lex_char_as_int(&mut self) -> Result<Token, String> {
        self.advance(); // consume '
        let c = if self.current() == '\\' {
            self.advance();
            self.decode_escape()
        } else {
            self.advance()
        };
        if self.pos >= self.input.len() || self.current() != '\'' {
            return Err("Unterminated char literal".to_string());
        }
        self.advance(); // consume '
        Ok(Token::IntLiteral(c as i64))
    }

    fn decode_escape(&mut self) -> char {
        match self.advance() {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            '\\' => '\\',
            '\'' => '\'',
            '"' => '"',
            '0' => '\0',
            'a' => '\x07',
            'b' => '\x08',
            'f' => '\x0C',
            'v' => '\x0B',
            c => c,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let mut lexer = Lexer::new("int main() { return 42; }");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Int,
                Token::Identifier("main".to_string()),
                Token::OpenParen,
                Token::CloseParen,
                Token::OpenBrace,
                Token::Return,
                Token::IntLiteral(42),
                Token::Semicolon,
                Token::CloseBrace,
            ]
        );
    }

    #[test]
    fn test_operators() {
        let mut lexer = Lexer::new("+ - * / && || == != <= >=");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Plus,
                Token::Minus,
                Token::Star,
                Token::Slash,
                Token::AmpersandAmpersand,
                Token::PipePipe,
                Token::EqualEqual,
                Token::BangEqual,
                Token::LessEqual,
                Token::GreaterEqual,
            ]
        );
    }

    #[test]
    fn test_comments() {
        let mut lexer = Lexer::new("42 // line\n43 /* block */ 44");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::IntLiteral(42),
                Token::IntLiteral(43),
                Token::IntLiteral(44),
            ]
        );
    }

    #[test]
    fn test_string_literal() {
        let mut lexer = Lexer::new("\"hello\\n\"");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::StringLiteral("hello\n".to_string())]);
    }
}
