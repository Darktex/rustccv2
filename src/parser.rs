use crate::lexer::{Token, TokenKind};

/// AST node types for a subset of C.

#[derive(Debug, Clone)]
pub struct Program {
    pub declarations: Vec<Declaration>,
}

#[derive(Debug, Clone)]
pub enum Declaration {
    Function(Function),
    GlobalVar(VarDecl),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Function {
    pub return_type: Type,
    pub name: String,
    pub params: Vec<Param>,
    pub body: Option<Block>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Param {
    pub ty: Type,
    pub name: String,
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
        cond: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
    },
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
    },
    DoWhile {
        body: Box<Stmt>,
        cond: Expr,
    },
    Block(Block),
    Break,
    Continue,
    Empty,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct VarDecl {
    pub ty: Type,
    pub name: String,
    pub init: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Char,
    Void,
    Long,
    Short,
    Unsigned,
    Pointer(Box<Type>),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Expr {
    IntLiteral(i64),
    StringLiteral(String),
    CharLiteral(char),
    Ident(String),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Assign(Box<Expr>, Box<Expr>),
    CompoundAssign(BinaryOp, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    PreIncrement(Box<Expr>),
    PreDecrement(Box<Expr>),
    PostIncrement(Box<Expr>),
    PostDecrement(Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    Sizeof(Box<Type>),
    Cast(Type, Box<Expr>),
    ArrayAccess(Box<Expr>, Box<Expr>),
    Deref(Box<Expr>),
    AddrOf(Box<Expr>),
}

#[derive(Debug, Clone, Copy)]
pub enum UnaryOp {
    Neg,
    BitNot,
    LogNot,
}

#[derive(Debug, Clone, Copy)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
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
        if self.peek() == expected {
            self.advance();
            Ok(())
        } else {
            let tok = &self.tokens[self.pos];
            Err(format!(
                "Expected {expected:?}, got {:?} at line {}:{}",
                tok.kind, tok.line, tok.col
            ))
        }
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), TokenKind::Eof)
    }

    fn parse_program(&mut self) -> Result<Program, String> {
        let mut declarations = Vec::new();
        while !self.at_eof() {
            declarations.push(self.parse_declaration()?);
        }
        Ok(Program { declarations })
    }

    fn is_type_keyword(&self) -> bool {
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
                | TokenKind::Static
                | TokenKind::Extern
        )
    }

    fn parse_type(&mut self) -> Result<Type, String> {
        // Skip storage class and qualifiers
        while matches!(
            self.peek(),
            TokenKind::Const | TokenKind::Static | TokenKind::Extern | TokenKind::Signed
        ) {
            self.advance();
        }

        let base = match self.peek() {
            TokenKind::Int => {
                self.advance();
                Type::Int
            }
            TokenKind::Char => {
                self.advance();
                Type::Char
            }
            TokenKind::Void => {
                self.advance();
                Type::Void
            }
            TokenKind::Long => {
                self.advance();
                // "long int" or just "long"
                if matches!(self.peek(), TokenKind::Int) {
                    self.advance();
                }
                Type::Long
            }
            TokenKind::Short => {
                self.advance();
                if matches!(self.peek(), TokenKind::Int) {
                    self.advance();
                }
                Type::Short
            }
            TokenKind::Unsigned => {
                self.advance();
                match self.peek() {
                    TokenKind::Int => {
                        self.advance();
                        Type::Unsigned
                    }
                    TokenKind::Long => {
                        self.advance();
                        if matches!(self.peek(), TokenKind::Int) {
                            self.advance();
                        }
                        Type::Unsigned
                    }
                    TokenKind::Char => {
                        self.advance();
                        Type::Char
                    }
                    TokenKind::Short => {
                        self.advance();
                        if matches!(self.peek(), TokenKind::Int) {
                            self.advance();
                        }
                        Type::Unsigned
                    }
                    _ => Type::Unsigned,
                }
            }
            _ => {
                let tok = &self.tokens[self.pos];
                return Err(format!(
                    "Expected type, got {:?} at line {}:{}",
                    tok.kind, tok.line, tok.col
                ));
            }
        };

        // Handle pointer types
        let mut ty = base;
        while matches!(self.peek(), TokenKind::Star) {
            self.advance();
            ty = Type::Pointer(Box::new(ty));
        }

        Ok(ty)
    }

    fn parse_declaration(&mut self) -> Result<Declaration, String> {
        let ty = self.parse_type()?;

        let name = match self.peek() {
            TokenKind::Ident(n) => {
                let n = n.clone();
                self.advance();
                n
            }
            _ => {
                let tok = &self.tokens[self.pos];
                return Err(format!(
                    "Expected identifier, got {:?} at line {}:{}",
                    tok.kind, tok.line, tok.col
                ));
            }
        };

        if matches!(self.peek(), TokenKind::LParen) {
            // Function definition or declaration
            self.advance(); // (
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen)?;

            if matches!(self.peek(), TokenKind::LBrace) {
                let body = self.parse_block()?;
                Ok(Declaration::Function(Function {
                    return_type: ty,
                    name,
                    params,
                    body: Some(body),
                }))
            } else {
                // Forward declaration
                self.expect(&TokenKind::Semicolon)?;
                Ok(Declaration::Function(Function {
                    return_type: ty,
                    name,
                    params,
                    body: None,
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
            Ok(Declaration::GlobalVar(VarDecl { ty, name, init }))
        }
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, String> {
        let mut params = Vec::new();
        if matches!(self.peek(), TokenKind::RParen) {
            return Ok(params);
        }
        // Handle (void)
        if matches!(self.peek(), TokenKind::Void) {
            let saved = self.pos;
            self.advance();
            if matches!(self.peek(), TokenKind::RParen) {
                return Ok(params);
            }
            self.pos = saved;
        }
        loop {
            let ty = self.parse_type()?;
            let name = match self.peek() {
                TokenKind::Ident(n) => {
                    let n = n.clone();
                    self.advance();
                    n
                }
                _ => String::new(), // unnamed parameter
            };
            // Handle array parameters like int arr[]
            if matches!(self.peek(), TokenKind::LBracket) {
                self.advance();
                self.expect(&TokenKind::RBracket)?;
            }
            params.push(Param { ty, name });
            if matches!(self.peek(), TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        Ok(params)
    }

    fn parse_block(&mut self) -> Result<Block, String> {
        self.expect(&TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(&TokenKind::RBrace)?;
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
            TokenKind::If => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let cond = self.parse_expr()?;
                self.expect(&TokenKind::RParen)?;
                let then_branch = Box::new(self.parse_stmt()?);
                let else_branch = if matches!(self.peek(), TokenKind::Else) {
                    self.advance();
                    Some(Box::new(self.parse_stmt()?))
                } else {
                    None
                };
                Ok(Stmt::If {
                    cond,
                    then_branch,
                    else_branch,
                })
            }
            TokenKind::While => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let cond = self.parse_expr()?;
                self.expect(&TokenKind::RParen)?;
                let body = Box::new(self.parse_stmt()?);
                Ok(Stmt::While { cond, body })
            }
            TokenKind::For => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let init = if matches!(self.peek(), TokenKind::Semicolon) {
                    self.advance();
                    None
                } else if self.is_type_keyword() {
                    let s = self.parse_var_decl()?;
                    Some(Box::new(s))
                } else {
                    let e = self.parse_expr()?;
                    self.expect(&TokenKind::Semicolon)?;
                    Some(Box::new(Stmt::Expr(e)))
                };
                let cond = if matches!(self.peek(), TokenKind::Semicolon) {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                self.expect(&TokenKind::Semicolon)?;
                let update = if matches!(self.peek(), TokenKind::RParen) {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                self.expect(&TokenKind::RParen)?;
                let body = Box::new(self.parse_stmt()?);
                Ok(Stmt::For {
                    init,
                    cond,
                    update,
                    body,
                })
            }
            TokenKind::Do => {
                self.advance();
                let body = Box::new(self.parse_stmt()?);
                self.expect(&TokenKind::While)?;
                self.expect(&TokenKind::LParen)?;
                let cond = self.parse_expr()?;
                self.expect(&TokenKind::RParen)?;
                self.expect(&TokenKind::Semicolon)?;
                Ok(Stmt::DoWhile { body, cond })
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
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                Ok(Stmt::Block(block))
            }
            TokenKind::Semicolon => {
                self.advance();
                Ok(Stmt::Empty)
            }
            _ if self.is_type_keyword() => self.parse_var_decl(),
            _ => {
                let expr = self.parse_expr()?;
                self.expect(&TokenKind::Semicolon)?;
                Ok(Stmt::Expr(expr))
            }
        }
    }

    fn parse_var_decl(&mut self) -> Result<Stmt, String> {
        let ty = self.parse_type()?;
        let name = match self.peek() {
            TokenKind::Ident(n) => {
                let n = n.clone();
                self.advance();
                n
            }
            _ => {
                let tok = &self.tokens[self.pos];
                return Err(format!(
                    "Expected variable name, got {:?} at line {}:{}",
                    tok.kind, tok.line, tok.col
                ));
            }
        };
        let init = if matches!(self.peek(), TokenKind::Assign) {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect(&TokenKind::Semicolon)?;
        Ok(Stmt::VarDecl(VarDecl { ty, name, init }))
    }

    // Expression parsing with precedence climbing
    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_ternary()
    }

    fn parse_ternary(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_assignment()?;
        if matches!(self.peek(), TokenKind::Question) {
            self.advance();
            let then_expr = self.parse_expr()?;
            self.expect(&TokenKind::Colon)?;
            let else_expr = self.parse_ternary()?;
            expr = Expr::Ternary(Box::new(expr), Box::new(then_expr), Box::new(else_expr));
        }
        Ok(expr)
    }

    fn parse_assignment(&mut self) -> Result<Expr, String> {
        let expr = self.parse_or()?;
        match self.peek() {
            TokenKind::Assign => {
                self.advance();
                let rhs = self.parse_assignment()?;
                Ok(Expr::Assign(Box::new(expr), Box::new(rhs)))
            }
            TokenKind::PlusAssign => {
                self.advance();
                let rhs = self.parse_assignment()?;
                Ok(Expr::CompoundAssign(
                    BinaryOp::Add,
                    Box::new(expr),
                    Box::new(rhs),
                ))
            }
            TokenKind::MinusAssign => {
                self.advance();
                let rhs = self.parse_assignment()?;
                Ok(Expr::CompoundAssign(
                    BinaryOp::Sub,
                    Box::new(expr),
                    Box::new(rhs),
                ))
            }
            TokenKind::StarAssign => {
                self.advance();
                let rhs = self.parse_assignment()?;
                Ok(Expr::CompoundAssign(
                    BinaryOp::Mul,
                    Box::new(expr),
                    Box::new(rhs),
                ))
            }
            TokenKind::SlashAssign => {
                self.advance();
                let rhs = self.parse_assignment()?;
                Ok(Expr::CompoundAssign(
                    BinaryOp::Div,
                    Box::new(expr),
                    Box::new(rhs),
                ))
            }
            TokenKind::PercentAssign => {
                self.advance();
                let rhs = self.parse_assignment()?;
                Ok(Expr::CompoundAssign(
                    BinaryOp::Mod,
                    Box::new(expr),
                    Box::new(rhs),
                ))
            }
            _ => Ok(expr),
        }
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), TokenKind::Or) {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Binary(BinaryOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_bitor()?;
        while matches!(self.peek(), TokenKind::And) {
            self.advance();
            let right = self.parse_bitor()?;
            left = Expr::Binary(BinaryOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_bitor(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_bitxor()?;
        while matches!(self.peek(), TokenKind::Pipe) {
            self.advance();
            let right = self.parse_bitxor()?;
            left = Expr::Binary(BinaryOp::BitOr, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_bitxor(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_bitand()?;
        while matches!(self.peek(), TokenKind::Caret) {
            self.advance();
            let right = self.parse_bitand()?;
            left = Expr::Binary(BinaryOp::BitXor, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_bitand(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_equality()?;
        while matches!(self.peek(), TokenKind::Ampersand) {
            self.advance();
            let right = self.parse_equality()?;
            left = Expr::Binary(BinaryOp::BitAnd, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_relational()?;
        loop {
            match self.peek() {
                TokenKind::Eq => {
                    self.advance();
                    let right = self.parse_relational()?;
                    left = Expr::Binary(BinaryOp::Eq, Box::new(left), Box::new(right));
                }
                TokenKind::Neq => {
                    self.advance();
                    let right = self.parse_relational()?;
                    left = Expr::Binary(BinaryOp::Neq, Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_relational(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_shift()?;
        loop {
            match self.peek() {
                TokenKind::Lt => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = Expr::Binary(BinaryOp::Lt, Box::new(left), Box::new(right));
                }
                TokenKind::Gt => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = Expr::Binary(BinaryOp::Gt, Box::new(left), Box::new(right));
                }
                TokenKind::Le => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = Expr::Binary(BinaryOp::Le, Box::new(left), Box::new(right));
                }
                TokenKind::Ge => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = Expr::Binary(BinaryOp::Ge, Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_additive()?;
        loop {
            match self.peek() {
                TokenKind::Shl => {
                    self.advance();
                    let right = self.parse_additive()?;
                    left = Expr::Binary(BinaryOp::Shl, Box::new(left), Box::new(right));
                }
                TokenKind::Shr => {
                    self.advance();
                    let right = self.parse_additive()?;
                    left = Expr::Binary(BinaryOp::Shr, Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_multiplicative()?;
        loop {
            match self.peek() {
                TokenKind::Plus => {
                    self.advance();
                    let right = self.parse_multiplicative()?;
                    left = Expr::Binary(BinaryOp::Add, Box::new(left), Box::new(right));
                }
                TokenKind::Minus => {
                    self.advance();
                    let right = self.parse_multiplicative()?;
                    left = Expr::Binary(BinaryOp::Sub, Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            match self.peek() {
                TokenKind::Star => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::Binary(BinaryOp::Mul, Box::new(left), Box::new(right));
                }
                TokenKind::Slash => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::Binary(BinaryOp::Div, Box::new(left), Box::new(right));
                }
                TokenKind::Percent => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::Binary(BinaryOp::Mod, Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            TokenKind::Minus => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::Unary(UnaryOp::Neg, Box::new(expr)))
            }
            TokenKind::Tilde => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::Unary(UnaryOp::BitNot, Box::new(expr)))
            }
            TokenKind::Bang => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::Unary(UnaryOp::LogNot, Box::new(expr)))
            }
            TokenKind::Increment => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::PreIncrement(Box::new(expr)))
            }
            TokenKind::Decrement => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::PreDecrement(Box::new(expr)))
            }
            TokenKind::Star => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::Deref(Box::new(expr)))
            }
            TokenKind::Ampersand => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::AddrOf(Box::new(expr)))
            }
            TokenKind::Sizeof => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                if self.is_type_keyword() {
                    let ty = self.parse_type()?;
                    self.expect(&TokenKind::RParen)?;
                    Ok(Expr::Sizeof(Box::new(ty)))
                } else {
                    // sizeof(expr) - treat as sizeof(int) for now
                    let _expr = self.parse_expr()?;
                    self.expect(&TokenKind::RParen)?;
                    Ok(Expr::Sizeof(Box::new(Type::Int)))
                }
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                TokenKind::LParen => {
                    // Function call
                    if let Expr::Ident(name) = expr {
                        self.advance();
                        let args = self.parse_args()?;
                        self.expect(&TokenKind::RParen)?;
                        expr = Expr::Call(name, args);
                    } else {
                        break;
                    }
                }
                TokenKind::LBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(&TokenKind::RBracket)?;
                    expr = Expr::ArrayAccess(Box::new(expr), Box::new(index));
                }
                TokenKind::Increment => {
                    self.advance();
                    expr = Expr::PostIncrement(Box::new(expr));
                }
                TokenKind::Decrement => {
                    self.advance();
                    expr = Expr::PostDecrement(Box::new(expr));
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        if matches!(self.peek(), TokenKind::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_assignment()?);
            if matches!(self.peek(), TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.peek().clone() {
            TokenKind::IntLiteral(n) => {
                self.advance();
                Ok(Expr::IntLiteral(n))
            }
            TokenKind::StringLiteral(s) => {
                let mut result = s.clone();
                self.advance();
                // Handle string concatenation (e.g., "hello " "world")
                while let TokenKind::StringLiteral(next) = self.peek() {
                    result.push_str(next);
                    self.advance();
                }
                Ok(Expr::StringLiteral(result))
            }
            TokenKind::CharLiteral(c) => {
                self.advance();
                Ok(Expr::CharLiteral(c))
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();
                Ok(Expr::Ident(name))
            }
            TokenKind::LParen => {
                self.advance();
                // Check for cast expression
                if self.is_type_keyword() {
                    let ty = self.parse_type()?;
                    self.expect(&TokenKind::RParen)?;
                    let expr = self.parse_unary()?;
                    Ok(Expr::Cast(ty, Box::new(expr)))
                } else {
                    let expr = self.parse_expr()?;
                    self.expect(&TokenKind::RParen)?;
                    Ok(expr)
                }
            }
            _ => {
                let tok = &self.tokens[self.pos];
                Err(format!(
                    "Unexpected token {:?} at line {}:{}",
                    tok.kind, tok.line, tok.col
                ))
            }
        }
    }
}

pub fn parse(tokens: &[Token]) -> Result<Program, String> {
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;

    fn parse_source(src: &str) -> Result<Program, String> {
        let tokens = lexer::lex(src)?;
        parse(&tokens)
    }

    #[test]
    fn test_return_42() {
        let prog = parse_source("int main() { return 42; }").unwrap();
        assert_eq!(prog.declarations.len(), 1);
        match &prog.declarations[0] {
            Declaration::Function(f) => {
                assert_eq!(f.name, "main");
                assert_eq!(f.return_type, Type::Int);
            }
            _ => panic!("Expected function"),
        }
    }

    #[test]
    fn test_variable_decl() {
        let prog = parse_source("int main() { int x = 5; return x; }").unwrap();
        match &prog.declarations[0] {
            Declaration::Function(f) => {
                let body = f.body.as_ref().unwrap();
                assert_eq!(body.stmts.len(), 2);
            }
            _ => panic!("Expected function"),
        }
    }

    #[test]
    fn test_if_else() {
        let prog =
            parse_source("int main() { int x = 10; if (x > 5) { return 1; } else { return 0; } }")
                .unwrap();
        match &prog.declarations[0] {
            Declaration::Function(f) => {
                let body = f.body.as_ref().unwrap();
                assert!(body.stmts.len() >= 2);
            }
            _ => panic!("Expected function"),
        }
    }

    #[test]
    fn test_for_loop() {
        let prog = parse_source(
            "int main() { int sum = 0; for (int i = 0; i < 10; i++) { sum += i; } return sum; }",
        )
        .unwrap();
        assert_eq!(prog.declarations.len(), 1);
    }

    #[test]
    fn test_function_call() {
        let prog = parse_source(
            r#"
            int add(int a, int b) { return a + b; }
            int main() { return add(1, 2); }
        "#,
        )
        .unwrap();
        assert_eq!(prog.declarations.len(), 2);
    }

    #[test]
    fn test_arithmetic() {
        let prog = parse_source("int main() { return 2 + 3 * 4; }").unwrap();
        match &prog.declarations[0] {
            Declaration::Function(f) => {
                let body = f.body.as_ref().unwrap();
                assert_eq!(body.stmts.len(), 1);
            }
            _ => panic!("Expected function"),
        }
    }
}
