/// Token types for the C lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    IntLiteral(i64),
    StringLiteral(String),
    CharLiteral(char),

    // Identifier
    Ident(String),

    // Keywords
    Int,
    Char,
    Void,
    Return,
    If,
    Else,
    While,
    For,
    Do,
    Break,
    Continue,
    Long,
    Short,
    Unsigned,
    Signed,
    Struct,
    Union,
    Enum,
    Typedef,
    Sizeof,
    Static,
    Extern,
    Const,

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
    Assign,
    Eq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    Shl,
    Shr,
    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,
    PercentAssign,
    AmpAssign,
    PipeAssign,
    CaretAssign,
    ShlAssign,
    ShrAssign,
    Increment,
    Decrement,
    Arrow,
    Dot,
    Question,
    Colon,

    // Delimiters
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Semicolon,
    Comma,

    // End of file
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

pub fn lex(source: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut pos = 0;
    let mut line = 1;
    let mut col = 1;

    while pos < chars.len() {
        let ch = chars[pos];

        // Skip whitespace
        if ch.is_ascii_whitespace() {
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
            pos += 1;
            continue;
        }

        // Skip line comments
        if ch == '/' && pos + 1 < chars.len() && chars[pos + 1] == '/' {
            pos += 2;
            while pos < chars.len() && chars[pos] != '\n' {
                pos += 1;
            }
            continue;
        }

        // Skip block comments
        if ch == '/' && pos + 1 < chars.len() && chars[pos + 1] == '*' {
            pos += 2;
            col += 2;
            loop {
                if pos >= chars.len() {
                    return Err(format!("Unterminated block comment at line {line}"));
                }
                if chars[pos] == '*' && pos + 1 < chars.len() && chars[pos + 1] == '/' {
                    pos += 2;
                    col += 2;
                    break;
                }
                if chars[pos] == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
                pos += 1;
            }
            continue;
        }

        // Skip preprocessor lines (minimal: just skip the whole line)
        if ch == '#' {
            // Skip entire preprocessor line (including continuation lines)
            while pos < chars.len() && chars[pos] != '\n' {
                pos += 1;
            }
            continue;
        }

        let tok_line = line;
        let tok_col = col;

        // Integer literals
        if ch.is_ascii_digit() {
            let start = pos;
            while pos < chars.len() && chars[pos].is_ascii_digit() {
                pos += 1;
                col += 1;
            }
            // Skip integer suffixes (L, U, LL, UL, etc.)
            while pos < chars.len()
                && (chars[pos] == 'l'
                    || chars[pos] == 'L'
                    || chars[pos] == 'u'
                    || chars[pos] == 'U')
            {
                pos += 1;
                col += 1;
            }
            let num_str: String = chars[start..pos]
                .iter()
                .filter(|c| c.is_ascii_digit())
                .collect();
            let val: i64 = num_str
                .parse()
                .map_err(|e| format!("Invalid integer: {e}"))?;
            tokens.push(Token {
                kind: TokenKind::IntLiteral(val),
                line: tok_line,
                col: tok_col,
            });
            continue;
        }

        // Identifiers and keywords
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = pos;
            while pos < chars.len() && (chars[pos].is_ascii_alphanumeric() || chars[pos] == '_') {
                pos += 1;
                col += 1;
            }
            let word: String = chars[start..pos].iter().collect();
            let kind = match word.as_str() {
                "int" => TokenKind::Int,
                "char" => TokenKind::Char,
                "void" => TokenKind::Void,
                "return" => TokenKind::Return,
                "if" => TokenKind::If,
                "else" => TokenKind::Else,
                "while" => TokenKind::While,
                "for" => TokenKind::For,
                "do" => TokenKind::Do,
                "break" => TokenKind::Break,
                "continue" => TokenKind::Continue,
                "long" => TokenKind::Long,
                "short" => TokenKind::Short,
                "unsigned" => TokenKind::Unsigned,
                "signed" => TokenKind::Signed,
                "struct" => TokenKind::Struct,
                "union" => TokenKind::Union,
                "enum" => TokenKind::Enum,
                "typedef" => TokenKind::Typedef,
                "sizeof" => TokenKind::Sizeof,
                "static" => TokenKind::Static,
                "extern" => TokenKind::Extern,
                "const" => TokenKind::Const,
                _ => TokenKind::Ident(word),
            };
            tokens.push(Token {
                kind,
                line: tok_line,
                col: tok_col,
            });
            continue;
        }

        // String literals
        if ch == '"' {
            pos += 1;
            col += 1;
            let mut s = String::new();
            while pos < chars.len() && chars[pos] != '"' {
                if chars[pos] == '\\' {
                    pos += 1;
                    col += 1;
                    if pos >= chars.len() {
                        return Err(format!("Unterminated string at line {tok_line}"));
                    }
                    match chars[pos] {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        '\\' => s.push('\\'),
                        '"' => s.push('"'),
                        '0' => s.push('\0'),
                        'r' => s.push('\r'),
                        'a' => s.push('\x07'),
                        'b' => s.push('\x08'),
                        'f' => s.push('\x0c'),
                        'v' => s.push('\x0b'),
                        '\'' => s.push('\''),
                        c => return Err(format!("Unknown escape '\\{c}' at line {tok_line}")),
                    }
                } else {
                    if chars[pos] == '\n' {
                        line += 1;
                        col = 0;
                    }
                    s.push(chars[pos]);
                }
                pos += 1;
                col += 1;
            }
            if pos >= chars.len() {
                return Err(format!("Unterminated string at line {tok_line}"));
            }
            pos += 1; // skip closing "
            col += 1;
            tokens.push(Token {
                kind: TokenKind::StringLiteral(s),
                line: tok_line,
                col: tok_col,
            });
            continue;
        }

        // Char literals
        if ch == '\'' {
            pos += 1;
            col += 1;
            let c = if pos < chars.len() && chars[pos] == '\\' {
                pos += 1;
                col += 1;
                if pos >= chars.len() {
                    return Err(format!("Unterminated char literal at line {tok_line}"));
                }
                let esc = match chars[pos] {
                    'n' => '\n',
                    't' => '\t',
                    '\\' => '\\',
                    '\'' => '\'',
                    '0' => '\0',
                    'r' => '\r',
                    c => {
                        return Err(format!(
                            "Unknown escape '\\{c}' in char literal at line {tok_line}"
                        ))
                    }
                };
                pos += 1;
                col += 1;
                esc
            } else if pos < chars.len() {
                let c = chars[pos];
                pos += 1;
                col += 1;
                c
            } else {
                return Err(format!("Unterminated char literal at line {tok_line}"));
            };

            if pos >= chars.len() || chars[pos] != '\'' {
                return Err(format!("Unterminated char literal at line {tok_line}"));
            }
            pos += 1;
            col += 1;
            tokens.push(Token {
                kind: TokenKind::CharLiteral(c),
                line: tok_line,
                col: tok_col,
            });
            continue;
        }

        // Multi-character operators
        let next = if pos + 1 < chars.len() {
            Some(chars[pos + 1])
        } else {
            None
        };

        let (kind, advance) = match (ch, next) {
            ('+', Some('+')) => (TokenKind::Increment, 2),
            ('+', Some('=')) => (TokenKind::PlusAssign, 2),
            ('-', Some('-')) => (TokenKind::Decrement, 2),
            ('-', Some('=')) => (TokenKind::MinusAssign, 2),
            ('-', Some('>')) => (TokenKind::Arrow, 2),
            ('*', Some('=')) => (TokenKind::StarAssign, 2),
            ('/', Some('=')) => (TokenKind::SlashAssign, 2),
            ('%', Some('=')) => (TokenKind::PercentAssign, 2),
            ('=', Some('=')) => (TokenKind::Eq, 2),
            ('!', Some('=')) => (TokenKind::Neq, 2),
            ('<', Some('=')) => (TokenKind::Le, 2),
            ('>', Some('=')) => (TokenKind::Ge, 2),
            ('<', Some('<')) => {
                if pos + 2 < chars.len() && chars[pos + 2] == '=' {
                    (TokenKind::ShlAssign, 3)
                } else {
                    (TokenKind::Shl, 2)
                }
            }
            ('>', Some('>')) => {
                if pos + 2 < chars.len() && chars[pos + 2] == '=' {
                    (TokenKind::ShrAssign, 3)
                } else {
                    (TokenKind::Shr, 2)
                }
            }
            ('&', Some('&')) => (TokenKind::And, 2),
            ('&', Some('=')) => (TokenKind::AmpAssign, 2),
            ('|', Some('|')) => (TokenKind::Or, 2),
            ('|', Some('=')) => (TokenKind::PipeAssign, 2),
            ('^', Some('=')) => (TokenKind::CaretAssign, 2),
            // Single-character operators
            ('+', _) => (TokenKind::Plus, 1),
            ('-', _) => (TokenKind::Minus, 1),
            ('*', _) => (TokenKind::Star, 1),
            ('/', _) => (TokenKind::Slash, 1),
            ('%', _) => (TokenKind::Percent, 1),
            ('&', _) => (TokenKind::Ampersand, 1),
            ('|', _) => (TokenKind::Pipe, 1),
            ('^', _) => (TokenKind::Caret, 1),
            ('~', _) => (TokenKind::Tilde, 1),
            ('!', _) => (TokenKind::Bang, 1),
            ('=', _) => (TokenKind::Assign, 1),
            ('<', _) => (TokenKind::Lt, 1),
            ('>', _) => (TokenKind::Gt, 1),
            ('(', _) => (TokenKind::LParen, 1),
            (')', _) => (TokenKind::RParen, 1),
            ('{', _) => (TokenKind::LBrace, 1),
            ('}', _) => (TokenKind::RBrace, 1),
            ('[', _) => (TokenKind::LBracket, 1),
            (']', _) => (TokenKind::RBracket, 1),
            (';', _) => (TokenKind::Semicolon, 1),
            (',', _) => (TokenKind::Comma, 1),
            ('?', _) => (TokenKind::Question, 1),
            (':', _) => (TokenKind::Colon, 1),
            ('.', _) => (TokenKind::Dot, 1),
            _ => return Err(format!("Unexpected character '{ch}' at line {line}:{col}")),
        };

        tokens.push(Token {
            kind,
            line: tok_line,
            col: tok_col,
        });
        pos += advance;
        col += advance;
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        line,
        col,
    });
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let tokens = lex("").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Eof);
    }

    #[test]
    fn test_return_42() {
        let tokens = lex("int main() { return 42; }").unwrap();
        let kinds: Vec<_> = tokens.iter().map(|t| &t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                &TokenKind::Int,
                &TokenKind::Ident("main".into()),
                &TokenKind::LParen,
                &TokenKind::RParen,
                &TokenKind::LBrace,
                &TokenKind::Return,
                &TokenKind::IntLiteral(42),
                &TokenKind::Semicolon,
                &TokenKind::RBrace,
                &TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_operators() {
        let tokens = lex("a + b - c * d / e % f").unwrap();
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Plus)));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Minus)));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Star)));
    }

    #[test]
    fn test_comments() {
        let tokens = lex("int // comment\nmain").unwrap();
        let kinds: Vec<_> = tokens
            .iter()
            .filter(|t| !matches!(t.kind, TokenKind::Eof))
            .map(|t| &t.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![&TokenKind::Int, &TokenKind::Ident("main".into())]
        );
    }

    #[test]
    fn test_string_literal() {
        let tokens = lex(r#""Hello, World!\n""#).unwrap();
        assert!(matches!(
            &tokens[0].kind,
            TokenKind::StringLiteral(s) if s == "Hello, World!\n"
        ));
    }

    #[test]
    fn test_comparison_operators() {
        let tokens = lex("== != < > <= >=").unwrap();
        let kinds: Vec<_> = tokens
            .iter()
            .filter(|t| !matches!(t.kind, TokenKind::Eof))
            .map(|t| &t.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                &TokenKind::Eq,
                &TokenKind::Neq,
                &TokenKind::Lt,
                &TokenKind::Gt,
                &TokenKind::Le,
                &TokenKind::Ge,
            ]
        );
    }

    #[test]
    fn test_preprocessor_skip() {
        let tokens = lex("#include <stdio.h>\nint main() {}").unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Int));
    }
}
