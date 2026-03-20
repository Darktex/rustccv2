//! Abstract Syntax Tree for the C language subset.

#[derive(Debug, Clone)]
pub struct Program {
    pub declarations: Vec<Declaration>,
}

#[derive(Debug, Clone)]
pub enum Declaration {
    Function(FunctionDecl),
    #[allow(dead_code)]
    GlobalVar(VarDecl),
}

#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub return_type: Type,
    pub name: String,
    pub params: Vec<Param>,
    pub body: Option<Block>,
}

#[derive(Debug, Clone)]
pub struct Param {
    #[allow(dead_code)]
    pub ty: Type,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    #[allow(dead_code)]
    pub ty: Type,
    pub name: String,
    pub init: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Char,
    Void,
}

pub type Block = Vec<Stmt>;

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
    DoWhile {
        body: Box<Stmt>,
        condition: Expr,
    },
    For {
        init: Option<Box<Stmt>>,
        condition: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
    },
    Block(Block),
    Break,
    Continue,
    Empty,
}

#[derive(Debug, Clone)]
pub enum Expr {
    IntLiteral(i64),
    StringLiteral(String),
    Var(String),
    Assign(String, Box<Expr>),
    BinaryOp(BinOp, Box<Expr>, Box<Expr>),
    UnaryOp(UnaryOp, Box<Expr>),
    Call(String, Vec<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    PreIncrement(String),
    PreDecrement(String),
    PostIncrement(String),
    PostDecrement(String),
    CompoundAssign(CompoundOp, String, Box<Expr>),
}

#[derive(Debug, Clone, Copy)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Equal,
    NotEqual,
    Less,
    Greater,
    LessEqual,
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
    BitwiseNot,
    LogicalNot,
}

#[derive(Debug, Clone, Copy)]
#[allow(clippy::enum_variant_names)]
pub enum CompoundOp {
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
}
