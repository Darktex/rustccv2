/// Token types for the C lexer.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum TokenKind {
    // Literals
    IntLiteral(i64),
    StringLiteral(String),
    CharLiteral(char),

    // Identifier
    Identifier(String),

    // Keywords
    Int,
    Char,
    Short,
    Long,
    Unsigned,
    Signed,
    Void,
    Return,
    If,
    Else,
    While,
    For,
    Do,
    Break,
    Continue,
    Struct,
    Union,
    Enum,
    Typedef,
    Extern,
    Static,
    Const,
    Sizeof,
    Switch,
    Case,
    Default,

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
    Less,
    Greater,
    Assign,

    // Compound operators
    PlusPlus,
    MinusMinus,
    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,
    PercentAssign,
    AmpAssign,
    PipeAssign,
    CaretAssign,
    LessLess,
    GreaterGreater,
    LessLessAssign,
    GreaterGreaterAssign,
    EqualEqual,
    BangEqual,
    LessEqual,
    GreaterEqual,
    AmpAmp,
    PipePipe,
    Arrow,

    // Punctuation
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Semicolon,
    Comma,
    Dot,
    Colon,
    Question,
    Ellipsis,
    Hash,

    // Special
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
        if ch.is_whitespace() {
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
            let start_line = line;
            pos += 2;
            col += 2;
            loop {
                if pos >= chars.len() {
                    return Err(format!(
                        "Unterminated block comment starting at line {}",
                        start_line
                    ));
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

        // Skip preprocessor directives (lines starting with #)
        if ch == '#' {
            // For now, skip the entire line (preprocessor not yet implemented)
            let tok_col = col;
            // Check if it's #include - we may need to handle it specially later
            pos += 1;
            col += 1;
            while pos < chars.len() && chars[pos] != '\n' {
                pos += 1;
            }
            // Don't emit any token for preprocessor directives
            let _ = tok_col;
            continue;
        }

        let tok_line = line;
        let tok_col = col;

        // Numbers
        if ch.is_ascii_digit() {
            let start = pos;
            while pos < chars.len() && chars[pos].is_ascii_digit() {
                pos += 1;
                col += 1;
            }
            // Handle hex literals
            if pos - start == 1
                && chars[start] == '0'
                && pos < chars.len()
                && (chars[pos] == 'x' || chars[pos] == 'X')
            {
                pos += 1;
                col += 1;
                while pos < chars.len() && chars[pos].is_ascii_hexdigit() {
                    pos += 1;
                    col += 1;
                }
                let hex_str: String = chars[start..pos].iter().collect();
                let val = i64::from_str_radix(&hex_str[2..], 16)
                    .map_err(|e| format!("Invalid hex literal '{}': {}", hex_str, e))?;
                // Skip integer suffixes (U, L, UL, LL, ULL, etc.)
                while pos < chars.len() && matches!(chars[pos], 'u' | 'U' | 'l' | 'L') {
                    pos += 1;
                    col += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::IntLiteral(val),
                    line: tok_line,
                    col: tok_col,
                });
            } else {
                let num_str: String = chars[start..pos].iter().collect();
                let val: i64 = num_str
                    .parse()
                    .map_err(|e| format!("Invalid integer '{}': {}", num_str, e))?;
                // Skip integer suffixes
                while pos < chars.len() && matches!(chars[pos], 'u' | 'U' | 'l' | 'L') {
                    pos += 1;
                    col += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::IntLiteral(val),
                    line: tok_line,
                    col: tok_col,
                });
            }
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
                "short" => TokenKind::Short,
                "long" => TokenKind::Long,
                "unsigned" => TokenKind::Unsigned,
                "signed" => TokenKind::Signed,
                "void" => TokenKind::Void,
                "return" => TokenKind::Return,
                "if" => TokenKind::If,
                "else" => TokenKind::Else,
                "while" => TokenKind::While,
                "for" => TokenKind::For,
                "do" => TokenKind::Do,
                "break" => TokenKind::Break,
                "continue" => TokenKind::Continue,
                "struct" => TokenKind::Struct,
                "union" => TokenKind::Union,
                "enum" => TokenKind::Enum,
                "typedef" => TokenKind::Typedef,
                "extern" => TokenKind::Extern,
                "static" => TokenKind::Static,
                "const" => TokenKind::Const,
                "sizeof" => TokenKind::Sizeof,
                "switch" => TokenKind::Switch,
                "case" => TokenKind::Case,
                "default" => TokenKind::Default,
                _ => TokenKind::Identifier(word),
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
                        return Err(format!("Unterminated string at line {}", tok_line));
                    }
                    match chars[pos] {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        'r' => s.push('\r'),
                        '\\' => s.push('\\'),
                        '"' => s.push('"'),
                        '0' => s.push('\0'),
                        'a' => s.push('\x07'),
                        'b' => s.push('\x08'),
                        'f' => s.push('\x0C'),
                        'v' => s.push('\x0B'),
                        'x' => {
                            // Hex escape
                            pos += 1;
                            col += 1;
                            let mut hex = String::new();
                            while pos < chars.len()
                                && chars[pos].is_ascii_hexdigit()
                                && hex.len() < 2
                            {
                                hex.push(chars[pos]);
                                pos += 1;
                                col += 1;
                            }
                            let val = u8::from_str_radix(&hex, 16)
                                .map_err(|_| format!("Invalid hex escape at line {}", tok_line))?;
                            s.push(val as char);
                            continue;
                        }
                        c => s.push(c),
                    }
                } else {
                    if chars[pos] == '\n' {
                        line += 1;
                        col = 1;
                    }
                    s.push(chars[pos]);
                }
                pos += 1;
                col += 1;
            }
            if pos >= chars.len() {
                return Err(format!("Unterminated string at line {}", tok_line));
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
                    return Err(format!("Unterminated char literal at line {}", tok_line));
                }
                let esc = match chars[pos] {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    '\\' => '\\',
                    '\'' => '\'',
                    '0' => '\0',
                    c => c,
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
                return Err(format!("Unterminated char literal at line {}", tok_line));
            };
            if pos >= chars.len() || chars[pos] != '\'' {
                return Err(format!("Unterminated char literal at line {}", tok_line));
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

        // Operators and punctuation
        let kind = match ch {
            '(' => {
                pos += 1;
                col += 1;
                TokenKind::LeftParen
            }
            ')' => {
                pos += 1;
                col += 1;
                TokenKind::RightParen
            }
            '{' => {
                pos += 1;
                col += 1;
                TokenKind::LeftBrace
            }
            '}' => {
                pos += 1;
                col += 1;
                TokenKind::RightBrace
            }
            '[' => {
                pos += 1;
                col += 1;
                TokenKind::LeftBracket
            }
            ']' => {
                pos += 1;
                col += 1;
                TokenKind::RightBracket
            }
            ';' => {
                pos += 1;
                col += 1;
                TokenKind::Semicolon
            }
            ',' => {
                pos += 1;
                col += 1;
                TokenKind::Comma
            }
            '~' => {
                pos += 1;
                col += 1;
                TokenKind::Tilde
            }
            '?' => {
                pos += 1;
                col += 1;
                TokenKind::Question
            }
            ':' => {
                pos += 1;
                col += 1;
                TokenKind::Colon
            }
            '.' => {
                if pos + 2 < chars.len() && chars[pos + 1] == '.' && chars[pos + 2] == '.' {
                    pos += 3;
                    col += 3;
                    TokenKind::Ellipsis
                } else {
                    pos += 1;
                    col += 1;
                    TokenKind::Dot
                }
            }
            '+' => {
                pos += 1;
                col += 1;
                if pos < chars.len() && chars[pos] == '+' {
                    pos += 1;
                    col += 1;
                    TokenKind::PlusPlus
                } else if pos < chars.len() && chars[pos] == '=' {
                    pos += 1;
                    col += 1;
                    TokenKind::PlusAssign
                } else {
                    TokenKind::Plus
                }
            }
            '-' => {
                pos += 1;
                col += 1;
                if pos < chars.len() && chars[pos] == '-' {
                    pos += 1;
                    col += 1;
                    TokenKind::MinusMinus
                } else if pos < chars.len() && chars[pos] == '=' {
                    pos += 1;
                    col += 1;
                    TokenKind::MinusAssign
                } else if pos < chars.len() && chars[pos] == '>' {
                    pos += 1;
                    col += 1;
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            '*' => {
                pos += 1;
                col += 1;
                if pos < chars.len() && chars[pos] == '=' {
                    pos += 1;
                    col += 1;
                    TokenKind::StarAssign
                } else {
                    TokenKind::Star
                }
            }
            '/' => {
                pos += 1;
                col += 1;
                if pos < chars.len() && chars[pos] == '=' {
                    pos += 1;
                    col += 1;
                    TokenKind::SlashAssign
                } else {
                    TokenKind::Slash
                }
            }
            '%' => {
                pos += 1;
                col += 1;
                if pos < chars.len() && chars[pos] == '=' {
                    pos += 1;
                    col += 1;
                    TokenKind::PercentAssign
                } else {
                    TokenKind::Percent
                }
            }
            '&' => {
                pos += 1;
                col += 1;
                if pos < chars.len() && chars[pos] == '&' {
                    pos += 1;
                    col += 1;
                    TokenKind::AmpAmp
                } else if pos < chars.len() && chars[pos] == '=' {
                    pos += 1;
                    col += 1;
                    TokenKind::AmpAssign
                } else {
                    TokenKind::Ampersand
                }
            }
            '|' => {
                pos += 1;
                col += 1;
                if pos < chars.len() && chars[pos] == '|' {
                    pos += 1;
                    col += 1;
                    TokenKind::PipePipe
                } else if pos < chars.len() && chars[pos] == '=' {
                    pos += 1;
                    col += 1;
                    TokenKind::PipeAssign
                } else {
                    TokenKind::Pipe
                }
            }
            '^' => {
                pos += 1;
                col += 1;
                if pos < chars.len() && chars[pos] == '=' {
                    pos += 1;
                    col += 1;
                    TokenKind::CaretAssign
                } else {
                    TokenKind::Caret
                }
            }
            '!' => {
                pos += 1;
                col += 1;
                if pos < chars.len() && chars[pos] == '=' {
                    pos += 1;
                    col += 1;
                    TokenKind::BangEqual
                } else {
                    TokenKind::Bang
                }
            }
            '=' => {
                pos += 1;
                col += 1;
                if pos < chars.len() && chars[pos] == '=' {
                    pos += 1;
                    col += 1;
                    TokenKind::EqualEqual
                } else {
                    TokenKind::Assign
                }
            }
            '<' => {
                pos += 1;
                col += 1;
                if pos < chars.len() && chars[pos] == '<' {
                    pos += 1;
                    col += 1;
                    if pos < chars.len() && chars[pos] == '=' {
                        pos += 1;
                        col += 1;
                        TokenKind::LessLessAssign
                    } else {
                        TokenKind::LessLess
                    }
                } else if pos < chars.len() && chars[pos] == '=' {
                    pos += 1;
                    col += 1;
                    TokenKind::LessEqual
                } else {
                    TokenKind::Less
                }
            }
            '>' => {
                pos += 1;
                col += 1;
                if pos < chars.len() && chars[pos] == '>' {
                    pos += 1;
                    col += 1;
                    if pos < chars.len() && chars[pos] == '=' {
                        pos += 1;
                        col += 1;
                        TokenKind::GreaterGreaterAssign
                    } else {
                        TokenKind::GreaterGreater
                    }
                } else if pos < chars.len() && chars[pos] == '=' {
                    pos += 1;
                    col += 1;
                    TokenKind::GreaterEqual
                } else {
                    TokenKind::Greater
                }
            }
            _ => {
                return Err(format!(
                    "Unexpected character '{}' at line {}, column {}",
                    ch, line, col
                ));
            }
        };

        tokens.push(Token {
            kind,
            line: tok_line,
            col: tok_col,
        });
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
    fn test_simple_main() {
        let tokens = lex("int main() { return 42; }").unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Int));
        assert!(matches!(tokens[1].kind, TokenKind::Identifier(ref s) if s == "main"));
        assert!(matches!(tokens[2].kind, TokenKind::LeftParen));
        assert!(matches!(tokens[3].kind, TokenKind::RightParen));
        assert!(matches!(tokens[4].kind, TokenKind::LeftBrace));
        assert!(matches!(tokens[5].kind, TokenKind::Return));
        assert!(matches!(tokens[6].kind, TokenKind::IntLiteral(42)));
        assert!(matches!(tokens[7].kind, TokenKind::Semicolon));
        assert!(matches!(tokens[8].kind, TokenKind::RightBrace));
        assert!(matches!(tokens[9].kind, TokenKind::Eof));
    }

    #[test]
    fn test_string_literal() {
        let tokens = lex(r#""Hello, World!\n""#).unwrap();
        assert!(
            matches!(tokens[0].kind, TokenKind::StringLiteral(ref s) if s == "Hello, World!\n")
        );
    }

    #[test]
    fn test_operators() {
        let tokens = lex("a++ + b-- - c").unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Identifier(_)));
        assert!(matches!(tokens[1].kind, TokenKind::PlusPlus));
        assert!(matches!(tokens[2].kind, TokenKind::Plus));
        assert!(matches!(tokens[3].kind, TokenKind::Identifier(_)));
        assert!(matches!(tokens[4].kind, TokenKind::MinusMinus));
        assert!(matches!(tokens[5].kind, TokenKind::Minus));
    }

    #[test]
    fn test_comments() {
        let tokens = lex("a // comment\nb /* block */ c").unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Identifier(ref s) if s == "a"));
        assert!(matches!(tokens[1].kind, TokenKind::Identifier(ref s) if s == "b"));
        assert!(matches!(tokens[2].kind, TokenKind::Identifier(ref s) if s == "c"));
    }

    #[test]
    fn test_preprocessor_skip() {
        let tokens = lex("#include <stdio.h>\nint x;").unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Int));
        assert!(matches!(tokens[1].kind, TokenKind::Identifier(ref s) if s == "x"));
    }
}
