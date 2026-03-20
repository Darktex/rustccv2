use crate::lexer::{Token, TokenKind};

/// AST node types for a C subset.

#[derive(Debug, Clone)]
pub struct Program {
    pub declarations: Vec<Declaration>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Declaration {
    Function(FunctionDef),
    GlobalVar(VarDecl),
    Typedef(TypeSpec, String), // aliased type, alias name
    StructDecl(TypeSpec),      // struct/union/enum definition (standalone)
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FunctionDef {
    pub return_type: TypeSpec,
    pub name: String,
    pub params: Vec<Param>,
    pub is_variadic: bool,
    pub body: Option<Block>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Param {
    pub type_spec: TypeSpec,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum TypeSpec {
    Void,
    Char,
    SignedChar,
    UnsignedChar,
    Short,
    UnsignedShort,
    Int,
    UnsignedInt,
    Long,
    UnsignedLong,
    Pointer(Box<TypeSpec>),
    Array(Box<TypeSpec>, Option<usize>), // element type, optional size
    Struct(String, Option<Vec<StructField>>), // name, optional fields (definition)
    Union(String, Option<Vec<StructField>>), // name, optional fields (definition)
    Enum(String, Option<Vec<EnumVariant>>), // name, optional variants
    TypedefName(String),                 // reference to a typedef
    FunctionPointer {
        return_type: Box<TypeSpec>,
        param_types: Vec<TypeSpec>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub type_spec: TypeSpec,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub value: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Return(Option<Expr>),
    Expr(Expr),
    VarDecl(VarDecl),
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    While {
        condition: Expr,
        body: Box<Stmt>,
    },
    For {
        init: Option<Box<Stmt>>,
        condition: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
    },
    DoWhile {
        body: Box<Stmt>,
        condition: Expr,
    },
    Block(Block),
    Break,
    Continue,
    Switch {
        expr: Expr,
        cases: Vec<SwitchCase>,
        default: Option<Vec<Stmt>>,
    },
    Empty,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SwitchCase {
    pub value: Expr,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct VarDecl {
    pub type_spec: TypeSpec,
    pub name: String,
    pub init: Option<Expr>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Expr {
    IntLiteral(i64),
    StringLiteral(String),
    CharLiteral(char),
    Identifier(String),
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
    },
    CompoundAssign {
        op: BinOp,
        target: Box<Expr>,
        value: Box<Expr>,
    },
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
    },
    PostIncrement(Box<Expr>),
    PostDecrement(Box<Expr>),
    PreIncrement(Box<Expr>),
    PreDecrement(Box<Expr>),
    Ternary {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
    },
    Deref(Box<Expr>),
    AddrOf(Box<Expr>),
    ArraySubscript {
        array: Box<Expr>,
        index: Box<Expr>,
    },
    Sizeof(Box<TypeSpec>),
    Cast {
        type_spec: TypeSpec,
        expr: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    LogicalAnd,
    LogicalOr,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    ShiftLeft,
    ShiftRight,
}

#[derive(Debug, Clone, Copy)]
pub enum UnaryOp {
    Negate,
    LogicalNot,
    BitwiseNot,
}

// --------------- Parser ---------------

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &TokenKind) -> Result<(), String> {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(expected) {
            self.advance();
            Ok(())
        } else {
            let tok = &self.tokens[self.pos];
            Err(format!(
                "Expected {:?}, got {:?} at line {}, col {}",
                expected, tok.kind, tok.line, tok.col
            ))
        }
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), TokenKind::Eof)
    }

    fn line_col(&self) -> (usize, usize) {
        let tok = &self.tokens[self.pos];
        (tok.line, tok.col)
    }

    // Parse the full program
    fn parse_program(&mut self) -> Result<Program, String> {
        let mut declarations = Vec::new();
        while !self.at_eof() {
            declarations.push(self.parse_declaration()?);
        }
        Ok(Program { declarations })
    }

    fn parse_declaration(&mut self) -> Result<Declaration, String> {
        // Check for typedef
        if matches!(self.peek(), TokenKind::Typedef) {
            self.advance();
            let type_spec = self.parse_type_spec()?;
            let alias = self.parse_identifier()?;
            self.expect(&TokenKind::Semicolon)?;
            return Ok(Declaration::Typedef(type_spec, alias));
        }

        // Check for extern
        let is_extern = matches!(self.peek(), TokenKind::Extern);
        if is_extern {
            self.advance();
        }

        // Check for static
        if matches!(self.peek(), TokenKind::Static) {
            self.advance();
        }

        let type_spec = self.parse_type_spec()?;

        // Check for standalone struct/union/enum declaration (no name follows)
        if matches!(self.peek(), TokenKind::Semicolon) {
            self.advance();
            return Ok(Declaration::StructDecl(type_spec));
        }

        let name = self.parse_identifier()?;

        if matches!(self.peek(), TokenKind::LeftParen) {
            // Function
            self.advance(); // (
            let (params, is_variadic) = self.parse_param_list()?;
            self.expect(&TokenKind::RightParen)?;

            if matches!(self.peek(), TokenKind::Semicolon) {
                // Forward declaration
                self.advance();
                Ok(Declaration::Function(FunctionDef {
                    return_type: type_spec,
                    name,
                    params,
                    is_variadic,
                    body: None,
                }))
            } else {
                let body = self.parse_block()?;
                Ok(Declaration::Function(FunctionDef {
                    return_type: type_spec,
                    name,
                    params,
                    is_variadic,
                    body: Some(body),
                }))
            }
        } else {
            // Global variable
            let init = if matches!(self.peek(), TokenKind::Assign) {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect(&TokenKind::Semicolon)?;
            Ok(Declaration::GlobalVar(VarDecl {
                type_spec,
                name,
                init,
            }))
        }
    }

    fn parse_type_spec(&mut self) -> Result<TypeSpec, String> {
        let base = match self.peek() {
            TokenKind::Int => {
                self.advance();
                TypeSpec::Int
            }
            TokenKind::Char => {
                self.advance();
                TypeSpec::Char
            }
            TokenKind::Void => {
                self.advance();
                TypeSpec::Void
            }
            TokenKind::Long => {
                self.advance();
                // Accept "long" or "long int" or "long long"
                if matches!(self.peek(), TokenKind::Long) {
                    self.advance(); // long long
                    if matches!(self.peek(), TokenKind::Int) {
                        self.advance();
                    }
                } else if matches!(self.peek(), TokenKind::Int) {
                    self.advance();
                }
                TypeSpec::Long
            }
            TokenKind::Short => {
                self.advance();
                if matches!(self.peek(), TokenKind::Int) {
                    self.advance();
                }
                TypeSpec::Short
            }
            TokenKind::Unsigned => {
                self.advance();
                match self.peek() {
                    TokenKind::Int => {
                        self.advance();
                        TypeSpec::UnsignedInt
                    }
                    TokenKind::Long => {
                        self.advance();
                        if matches!(self.peek(), TokenKind::Long) {
                            self.advance(); // unsigned long long
                        }
                        if matches!(self.peek(), TokenKind::Int) {
                            self.advance();
                        }
                        TypeSpec::UnsignedLong
                    }
                    TokenKind::Char => {
                        self.advance();
                        TypeSpec::UnsignedChar
                    }
                    TokenKind::Short => {
                        self.advance();
                        if matches!(self.peek(), TokenKind::Int) {
                            self.advance();
                        }
                        TypeSpec::UnsignedShort
                    }
                    _ => TypeSpec::UnsignedInt, // bare "unsigned" = unsigned int
                }
            }
            TokenKind::Signed => {
                self.advance();
                match self.peek() {
                    TokenKind::Int => {
                        self.advance();
                        TypeSpec::Int
                    }
                    TokenKind::Long => {
                        self.advance();
                        if matches!(self.peek(), TokenKind::Int) {
                            self.advance();
                        }
                        TypeSpec::Long
                    }
                    TokenKind::Char => {
                        self.advance();
                        TypeSpec::SignedChar
                    }
                    TokenKind::Short => {
                        self.advance();
                        if matches!(self.peek(), TokenKind::Int) {
                            self.advance();
                        }
                        TypeSpec::Short
                    }
                    _ => TypeSpec::Int, // bare "signed" = signed int
                }
            }
            TokenKind::Struct => {
                self.advance();
                self.parse_struct_or_union(true)?
            }
            TokenKind::Union => {
                self.advance();
                self.parse_struct_or_union(false)?
            }
            TokenKind::Enum => {
                self.advance();
                self.parse_enum_spec()?
            }
            TokenKind::Const => {
                self.advance();
                // Parse the underlying type, ignore const for now
                return self.parse_type_spec();
            }
            _ => {
                let (line, col) = self.line_col();
                return Err(format!(
                    "Expected type specifier, got {:?} at line {}, col {}",
                    self.peek(),
                    line,
                    col
                ));
            }
        };

        // Handle pointer types (possibly with const qualifier)
        let mut result = base;
        while matches!(self.peek(), TokenKind::Star) {
            self.advance();
            // Skip const after * (e.g., int *const p)
            if matches!(self.peek(), TokenKind::Const) {
                self.advance();
            }
            result = TypeSpec::Pointer(Box::new(result));
        }

        Ok(result)
    }

    fn parse_struct_or_union(&mut self, is_struct: bool) -> Result<TypeSpec, String> {
        // Parse optional name
        let name = if matches!(self.peek(), TokenKind::Identifier(_)) {
            self.parse_identifier()?
        } else {
            format!("__anon_{}", self.pos) // anonymous struct/union
        };

        // Parse optional body
        let fields = if matches!(self.peek(), TokenKind::LeftBrace) {
            self.advance();
            let mut fields = Vec::new();
            while !matches!(self.peek(), TokenKind::RightBrace | TokenKind::Eof) {
                let field_type = self.parse_type_spec()?;
                let field_name = self.parse_identifier()?;
                // Handle array fields
                if matches!(self.peek(), TokenKind::LeftBracket) {
                    self.advance();
                    if !matches!(self.peek(), TokenKind::RightBracket) {
                        let _ = self.parse_expr()?; // skip size for now
                    }
                    self.expect(&TokenKind::RightBracket)?;
                }
                self.expect(&TokenKind::Semicolon)?;
                fields.push(StructField {
                    type_spec: field_type,
                    name: field_name,
                });
            }
            self.expect(&TokenKind::RightBrace)?;
            Some(fields)
        } else {
            None
        };

        if is_struct {
            Ok(TypeSpec::Struct(name, fields))
        } else {
            Ok(TypeSpec::Union(name, fields))
        }
    }

    fn parse_enum_spec(&mut self) -> Result<TypeSpec, String> {
        let name = if matches!(self.peek(), TokenKind::Identifier(_)) {
            self.parse_identifier()?
        } else {
            format!("__anon_enum_{}", self.pos)
        };

        let variants = if matches!(self.peek(), TokenKind::LeftBrace) {
            self.advance();
            let mut variants = Vec::new();
            let mut next_value: i64 = 0;
            while !matches!(self.peek(), TokenKind::RightBrace | TokenKind::Eof) {
                let variant_name = self.parse_identifier()?;
                let value = if matches!(self.peek(), TokenKind::Assign) {
                    self.advance();
                    match self.peek() {
                        TokenKind::IntLiteral(v) => {
                            let v = *v;
                            self.advance();
                            next_value = v + 1;
                            Some(v)
                        }
                        _ => {
                            let (line, col) = self.line_col();
                            return Err(format!(
                                "Expected integer literal for enum value at line {}, col {}",
                                line, col
                            ));
                        }
                    }
                } else {
                    let v = next_value;
                    next_value += 1;
                    Some(v)
                };
                variants.push(EnumVariant {
                    name: variant_name,
                    value,
                });
                if matches!(self.peek(), TokenKind::Comma) {
                    self.advance();
                }
            }
            self.expect(&TokenKind::RightBrace)?;
            Some(variants)
        } else {
            None
        };

        Ok(TypeSpec::Enum(name, variants))
    }

    fn parse_identifier(&mut self) -> Result<String, String> {
        match self.peek().clone() {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            _ => {
                let (line, col) = self.line_col();
                Err(format!(
                    "Expected identifier, got {:?} at line {}, col {}",
                    self.peek(),
                    line,
                    col
                ))
            }
        }
    }

    fn parse_param_list(&mut self) -> Result<(Vec<Param>, bool), String> {
        let mut params = Vec::new();
        let mut is_variadic = false;
        if matches!(self.peek(), TokenKind::RightParen) {
            return Ok((params, false));
        }
        if matches!(self.peek(), TokenKind::Void) {
            // Check if it's just "void" with no param name
            let save = self.pos;
            self.advance();
            if matches!(self.peek(), TokenKind::RightParen) {
                return Ok((params, false));
            }
            self.pos = save;
        }

        loop {
            // Handle ellipsis for varargs
            if matches!(self.peek(), TokenKind::Ellipsis) {
                self.advance();
                is_variadic = true;
                break;
            }

            let type_spec = self.parse_type_spec()?;
            let name = if matches!(self.peek(), TokenKind::Identifier(_)) {
                self.parse_identifier()?
            } else {
                String::new() // anonymous param
            };
            // Handle array params like char *argv[]
            if matches!(self.peek(), TokenKind::LeftBracket) {
                self.advance();
                self.expect(&TokenKind::RightBracket)?;
            }
            params.push(Param { type_spec, name });
            if !matches!(self.peek(), TokenKind::Comma) {
                break;
            }
            self.advance(); // skip comma
        }
        Ok((params, is_variadic))
    }

    fn parse_block(&mut self) -> Result<Block, String> {
        self.expect(&TokenKind::LeftBrace)?;
        let mut stmts = Vec::new();
        while !matches!(self.peek(), TokenKind::RightBrace | TokenKind::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(&TokenKind::RightBrace)?;
        Ok(Block { stmts })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        match self.peek() {
            TokenKind::Return => {
                self.advance();
                if matches!(self.peek(), TokenKind::Semicolon) {
                    self.advance();
                    Ok(Stmt::Return(None))
                } else {
                    let expr = self.parse_expr()?;
                    self.expect(&TokenKind::Semicolon)?;
                    Ok(Stmt::Return(Some(expr)))
                }
            }
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::For => self.parse_for(),
            TokenKind::Do => self.parse_do_while(),
            TokenKind::Switch => self.parse_switch(),
            TokenKind::LeftBrace => {
                let block = self.parse_block()?;
                Ok(Stmt::Block(block))
            }
            TokenKind::Break => {
                self.advance();
                self.expect(&TokenKind::Semicolon)?;
                Ok(Stmt::Break)
            }
            TokenKind::Continue => {
                self.advance();
                self.expect(&TokenKind::Semicolon)?;
                Ok(Stmt::Continue)
            }
            TokenKind::Semicolon => {
                self.advance();
                Ok(Stmt::Empty)
            }
            // Type keywords indicate a variable declaration
            TokenKind::Int
            | TokenKind::Char
            | TokenKind::Void
            | TokenKind::Long
            | TokenKind::Short
            | TokenKind::Unsigned
            | TokenKind::Signed
            | TokenKind::Const
            | TokenKind::Static
            | TokenKind::Struct
            | TokenKind::Union
            | TokenKind::Enum => self.parse_var_decl_stmt(),
            _ => {
                let expr = self.parse_expr()?;
                self.expect(&TokenKind::Semicolon)?;
                Ok(Stmt::Expr(expr))
            }
        }
    }

    fn parse_var_decl_stmt(&mut self) -> Result<Stmt, String> {
        // Skip static keyword for now
        if matches!(self.peek(), TokenKind::Static) {
            self.advance();
        }
        let type_spec = self.parse_type_spec()?;
        let name = self.parse_identifier()?;

        // Check for array declaration
        if matches!(self.peek(), TokenKind::LeftBracket) {
            self.advance();
            // skip size if present
            if !matches!(self.peek(), TokenKind::RightBracket) {
                let _ = self.parse_expr()?;
            }
            self.expect(&TokenKind::RightBracket)?;
            let init = if matches!(self.peek(), TokenKind::Assign) {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect(&TokenKind::Semicolon)?;
            return Ok(Stmt::VarDecl(VarDecl {
                type_spec,
                name,
                init,
            }));
        }

        let init = if matches!(self.peek(), TokenKind::Assign) {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect(&TokenKind::Semicolon)?;
        Ok(Stmt::VarDecl(VarDecl {
            type_spec,
            name,
            init,
        }))
    }

    fn parse_if(&mut self) -> Result<Stmt, String> {
        self.advance(); // if
        self.expect(&TokenKind::LeftParen)?;
        let condition = self.parse_expr()?;
        self.expect(&TokenKind::RightParen)?;
        let then_branch = Box::new(self.parse_stmt()?);
        let else_branch = if matches!(self.peek(), TokenKind::Else) {
            self.advance();
            Some(Box::new(self.parse_stmt()?))
        } else {
            None
        };
        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
        })
    }

    fn parse_while(&mut self) -> Result<Stmt, String> {
        self.advance(); // while
        self.expect(&TokenKind::LeftParen)?;
        let condition = self.parse_expr()?;
        self.expect(&TokenKind::RightParen)?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::While { condition, body })
    }

    fn parse_for(&mut self) -> Result<Stmt, String> {
        self.advance(); // for
        self.expect(&TokenKind::LeftParen)?;

        // Init
        let init = if matches!(self.peek(), TokenKind::Semicolon) {
            self.advance();
            None
        } else if matches!(
            self.peek(),
            TokenKind::Int
                | TokenKind::Char
                | TokenKind::Long
                | TokenKind::Short
                | TokenKind::Unsigned
                | TokenKind::Signed
                | TokenKind::Struct
                | TokenKind::Union
                | TokenKind::Enum
        ) {
            Some(Box::new(self.parse_var_decl_stmt()?))
        } else {
            let expr = self.parse_expr()?;
            self.expect(&TokenKind::Semicolon)?;
            Some(Box::new(Stmt::Expr(expr)))
        };

        // Condition
        let condition = if matches!(self.peek(), TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(&TokenKind::Semicolon)?;

        // Update
        let update = if matches!(self.peek(), TokenKind::RightParen) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(&TokenKind::RightParen)?;

        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::For {
            init,
            condition,
            update,
            body,
        })
    }

    fn parse_do_while(&mut self) -> Result<Stmt, String> {
        self.advance(); // do
        let body = Box::new(self.parse_stmt()?);
        self.expect(&TokenKind::While)?;
        self.expect(&TokenKind::LeftParen)?;
        let condition = self.parse_expr()?;
        self.expect(&TokenKind::RightParen)?;
        self.expect(&TokenKind::Semicolon)?;
        Ok(Stmt::DoWhile { body, condition })
    }

    fn parse_switch(&mut self) -> Result<Stmt, String> {
        self.advance(); // switch
        self.expect(&TokenKind::LeftParen)?;
        let expr = self.parse_expr()?;
        self.expect(&TokenKind::RightParen)?;
        self.expect(&TokenKind::LeftBrace)?;

        let mut cases = Vec::new();
        let mut default = None;

        while !matches!(self.peek(), TokenKind::RightBrace | TokenKind::Eof) {
            match self.peek() {
                TokenKind::Case => {
                    self.advance(); // case
                    let value = self.parse_expr()?;
                    self.expect(&TokenKind::Colon)?;
                    let mut body = Vec::new();
                    while !matches!(
                        self.peek(),
                        TokenKind::Case
                            | TokenKind::Default
                            | TokenKind::RightBrace
                            | TokenKind::Eof
                    ) {
                        body.push(self.parse_stmt()?);
                    }
                    cases.push(SwitchCase { value, body });
                }
                TokenKind::Default => {
                    self.advance(); // default
                    self.expect(&TokenKind::Colon)?;
                    let mut body = Vec::new();
                    while !matches!(
                        self.peek(),
                        TokenKind::Case
                            | TokenKind::Default
                            | TokenKind::RightBrace
                            | TokenKind::Eof
                    ) {
                        body.push(self.parse_stmt()?);
                    }
                    default = Some(body);
                }
                _ => {
                    let (line, col) = self.line_col();
                    return Err(format!(
                        "Expected 'case' or 'default' in switch at line {}, col {}",
                        line, col
                    ));
                }
            }
        }

        self.expect(&TokenKind::RightBrace)?;
        Ok(Stmt::Switch {
            expr,
            cases,
            default,
        })
    }

    // Expression parsing with precedence climbing

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_ternary()
    }

    fn parse_ternary(&mut self) -> Result<Expr, String> {
        let expr = self.parse_assignment()?;
        if matches!(self.peek(), TokenKind::Question) {
            self.advance();
            let then_expr = self.parse_expr()?;
            self.expect(&TokenKind::Colon)?;
            let else_expr = self.parse_ternary()?;
            Ok(Expr::Ternary {
                condition: Box::new(expr),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            })
        } else {
            Ok(expr)
        }
    }

    fn parse_assignment(&mut self) -> Result<Expr, String> {
        let expr = self.parse_logical_or()?;
        match self.peek() {
            TokenKind::Assign => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(Expr::Assign {
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            TokenKind::PlusAssign => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(Expr::CompoundAssign {
                    op: BinOp::Add,
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            TokenKind::MinusAssign => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(Expr::CompoundAssign {
                    op: BinOp::Sub,
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            TokenKind::StarAssign => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(Expr::CompoundAssign {
                    op: BinOp::Mul,
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            TokenKind::SlashAssign => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(Expr::CompoundAssign {
                    op: BinOp::Div,
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            TokenKind::PercentAssign => {
                self.advance();
                let value = self.parse_assignment()?;
                Ok(Expr::CompoundAssign {
                    op: BinOp::Mod,
                    target: Box::new(expr),
                    value: Box::new(value),
                })
            }
            _ => Ok(expr),
        }
    }

    fn parse_logical_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_logical_and()?;
        while matches!(self.peek(), TokenKind::PipePipe) {
            self.advance();
            let right = self.parse_logical_and()?;
            left = Expr::Binary {
                op: BinOp::LogicalOr,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_bitwise_or()?;
        while matches!(self.peek(), TokenKind::AmpAmp) {
            self.advance();
            let right = self.parse_bitwise_or()?;
            left = Expr::Binary {
                op: BinOp::LogicalAnd,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_bitwise_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_bitwise_xor()?;
        while matches!(self.peek(), TokenKind::Pipe) {
            self.advance();
            let right = self.parse_bitwise_xor()?;
            left = Expr::Binary {
                op: BinOp::BitwiseOr,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_bitwise_xor(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_bitwise_and()?;
        while matches!(self.peek(), TokenKind::Caret) {
            self.advance();
            let right = self.parse_bitwise_and()?;
            left = Expr::Binary {
                op: BinOp::BitwiseXor,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_bitwise_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_equality()?;
        while matches!(self.peek(), TokenKind::Ampersand) {
            self.advance();
            let right = self.parse_equality()?;
            left = Expr::Binary {
                op: BinOp::BitwiseAnd,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_relational()?;
        loop {
            let op = match self.peek() {
                TokenKind::EqualEqual => BinOp::Equal,
                TokenKind::BangEqual => BinOp::NotEqual,
                _ => break,
            };
            self.advance();
            let right = self.parse_relational()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_relational(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_shift()?;
        loop {
            let op = match self.peek() {
                TokenKind::Less => BinOp::Less,
                TokenKind::LessEqual => BinOp::LessEqual,
                TokenKind::Greater => BinOp::Greater,
                TokenKind::GreaterEqual => BinOp::GreaterEqual,
                _ => break,
            };
            self.advance();
            let right = self.parse_shift()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_additive()?;
        loop {
            let op = match self.peek() {
                TokenKind::LessLess => BinOp::ShiftLeft,
                TokenKind::GreaterGreater => BinOp::ShiftRight,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            TokenKind::Minus => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnaryOp::Negate,
                    operand: Box::new(operand),
                })
            }
            TokenKind::Bang => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnaryOp::LogicalNot,
                    operand: Box::new(operand),
                })
            }
            TokenKind::Tilde => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnaryOp::BitwiseNot,
                    operand: Box::new(operand),
                })
            }
            TokenKind::Star => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::Deref(Box::new(operand)))
            }
            TokenKind::Ampersand => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::AddrOf(Box::new(operand)))
            }
            TokenKind::PlusPlus => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::PreIncrement(Box::new(operand)))
            }
            TokenKind::MinusMinus => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::PreDecrement(Box::new(operand)))
            }
            TokenKind::Sizeof => {
                self.advance();
                self.expect(&TokenKind::LeftParen)?;
                let ts = self.parse_type_spec()?;
                self.expect(&TokenKind::RightParen)?;
                Ok(Expr::Sizeof(Box::new(ts)))
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                TokenKind::LeftParen => {
                    self.advance();
                    let args = self.parse_arg_list()?;
                    self.expect(&TokenKind::RightParen)?;
                    expr = Expr::Call {
                        func: Box::new(expr),
                        args,
                    };
                }
                TokenKind::LeftBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(&TokenKind::RightBracket)?;
                    expr = Expr::ArraySubscript {
                        array: Box::new(expr),
                        index: Box::new(index),
                    };
                }
                TokenKind::PlusPlus => {
                    self.advance();
                    expr = Expr::PostIncrement(Box::new(expr));
                }
                TokenKind::MinusMinus => {
                    self.advance();
                    expr = Expr::PostDecrement(Box::new(expr));
                }
                TokenKind::Dot => {
                    self.advance();
                    let _field = self.parse_identifier()?;
                    // TODO: struct field access
                }
                TokenKind::Arrow => {
                    self.advance();
                    let _field = self.parse_identifier()?;
                    // TODO: struct pointer field access
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.peek().clone() {
            TokenKind::IntLiteral(val) => {
                self.advance();
                Ok(Expr::IntLiteral(val))
            }
            TokenKind::StringLiteral(s) => {
                let mut result = s.clone();
                self.advance();
                // Handle adjacent string literal concatenation
                while let TokenKind::StringLiteral(next) = self.peek().clone() {
                    result.push_str(&next);
                    self.advance();
                }
                Ok(Expr::StringLiteral(result))
            }
            TokenKind::CharLiteral(c) => {
                self.advance();
                Ok(Expr::CharLiteral(c))
            }
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                Ok(Expr::Identifier(name))
            }
            TokenKind::LeftParen => {
                self.advance();
                // Check for cast: (type)expr
                if self.is_type_token() {
                    let type_spec = self.parse_type_spec()?;
                    self.expect(&TokenKind::RightParen)?;
                    let expr = self.parse_unary()?;
                    Ok(Expr::Cast {
                        type_spec,
                        expr: Box::new(expr),
                    })
                } else {
                    let expr = self.parse_expr()?;
                    self.expect(&TokenKind::RightParen)?;
                    Ok(expr)
                }
            }
            _ => {
                let (line, col) = self.line_col();
                Err(format!(
                    "Expected expression, got {:?} at line {}, col {}",
                    self.peek(),
                    line,
                    col
                ))
            }
        }
    }

    fn is_type_token(&self) -> bool {
        matches!(
            self.peek(),
            TokenKind::Int
                | TokenKind::Char
                | TokenKind::Void
                | TokenKind::Long
                | TokenKind::Short
                | TokenKind::Unsigned
                | TokenKind::Signed
                | TokenKind::Const
                | TokenKind::Struct
                | TokenKind::Union
                | TokenKind::Enum
        )
    }

    fn parse_arg_list(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        if matches!(self.peek(), TokenKind::RightParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_assignment()?);
            if !matches!(self.peek(), TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        Ok(args)
    }
}

pub fn parse(tokens: Vec<Token>) -> Result<Program, String> {
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;

    #[test]
    fn test_parse_return_42() {
        let tokens = lexer::lex("int main() { return 42; }").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(program.declarations.len(), 1);
        match &program.declarations[0] {
            Declaration::Function(f) => {
                assert_eq!(f.name, "main");
                assert!(f.body.is_some());
            }
            _ => panic!("Expected function declaration"),
        }
    }

    #[test]
    fn test_parse_binary_expr() {
        let tokens = lexer::lex("int main() { return 1 + 2 * 3; }").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(program.declarations.len(), 1);
    }

    #[test]
    fn test_parse_var_decl() {
        let tokens = lexer::lex("int main() { int x = 5; return x; }").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(program.declarations.len(), 1);
    }

    #[test]
    fn test_parse_if_else() {
        let tokens = lexer::lex("int main() { if (1) return 1; else return 0; }").unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(program.declarations.len(), 1);
    }

    #[test]
    fn test_parse_for_loop() {
        let tokens =
            lexer::lex("int main() { int s = 0; for (int i = 0; i < 10; i++) s += i; return s; }")
                .unwrap();
        let program = parse(tokens).unwrap();
        assert_eq!(program.declarations.len(), 1);
    }
}
