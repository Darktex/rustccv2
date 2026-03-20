//! Recursive-descent parser for C subset -> AST.

use crate::ast::*;
use crate::lexer::Token;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse_program(&mut self) -> Result<Program, String> {
        let mut declarations = Vec::new();
        while self.pos < self.tokens.len() {
            declarations.push(self.parse_declaration()?);
        }
        Ok(Program { declarations })
    }

    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Result<Token, String> {
        if self.pos >= self.tokens.len() {
            return Err("Unexpected end of input".to_string());
        }
        let tok = self.tokens[self.pos].clone();
        self.pos += 1;
        Ok(tok)
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        let tok = self.advance()?;
        if &tok == expected {
            Ok(())
        } else {
            Err(format!("Expected {expected}, got {tok}"))
        }
    }

    fn parse_type(&mut self) -> Result<Type, String> {
        match self.advance()? {
            Token::Int => Ok(Type::Int),
            Token::Char => Ok(Type::Char),
            Token::Void => Ok(Type::Void),
            tok => Err(format!("Expected type, got {tok}")),
        }
    }

    fn parse_declaration(&mut self) -> Result<Declaration, String> {
        let ty = self.parse_type()?;
        let name = match self.advance()? {
            Token::Identifier(n) => n,
            tok => return Err(format!("Expected identifier, got {tok}")),
        };

        if self.current() == Some(&Token::OpenParen) {
            // Function declaration
            self.advance()?; // consume (
            let params = self.parse_params()?;
            self.expect(&Token::CloseParen)?;

            if self.current() == Some(&Token::Semicolon) {
                self.advance()?;
                return Ok(Declaration::Function(FunctionDecl {
                    return_type: ty,
                    name,
                    params,
                    body: None,
                }));
            }

            let body = self.parse_block()?;
            Ok(Declaration::Function(FunctionDecl {
                return_type: ty,
                name,
                params,
                body: Some(body),
            }))
        } else {
            // Global variable
            let init = if self.current() == Some(&Token::Equal) {
                self.advance()?;
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect(&Token::Semicolon)?;
            Ok(Declaration::GlobalVar(VarDecl { ty, name, init }))
        }
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, String> {
        let mut params = Vec::new();
        if self.current() == Some(&Token::CloseParen) {
            return Ok(params);
        }
        // Handle (void)
        if self.current() == Some(&Token::Void)
            && self.tokens.get(self.pos + 1) == Some(&Token::CloseParen)
        {
            self.advance()?;
            return Ok(params);
        }

        loop {
            let ty = self.parse_type()?;
            let name = match self.advance()? {
                Token::Identifier(n) => n,
                tok => return Err(format!("Expected parameter name, got {tok}")),
            };
            params.push(Param { ty, name });
            if self.current() != Some(&Token::Comma) {
                break;
            }
            self.advance()?; // consume comma
        }
        Ok(params)
    }

    fn parse_block(&mut self) -> Result<Block, String> {
        self.expect(&Token::OpenBrace)?;
        let mut stmts = Vec::new();
        while self.current() != Some(&Token::CloseBrace) {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(&Token::CloseBrace)?;
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        match self.current() {
            Some(Token::Return) => {
                self.advance()?;
                if self.current() == Some(&Token::Semicolon) {
                    self.advance()?;
                    Ok(Stmt::Return(None))
                } else {
                    let expr = self.parse_expr()?;
                    self.expect(&Token::Semicolon)?;
                    Ok(Stmt::Return(Some(expr)))
                }
            }
            Some(Token::If) => self.parse_if(),
            Some(Token::While) => self.parse_while(),
            Some(Token::Do) => self.parse_do_while(),
            Some(Token::For) => self.parse_for(),
            Some(Token::Break) => {
                self.advance()?;
                self.expect(&Token::Semicolon)?;
                Ok(Stmt::Break)
            }
            Some(Token::Continue) => {
                self.advance()?;
                self.expect(&Token::Semicolon)?;
                Ok(Stmt::Continue)
            }
            Some(Token::OpenBrace) => {
                let block = self.parse_block()?;
                Ok(Stmt::Block(block))
            }
            Some(Token::Semicolon) => {
                self.advance()?;
                Ok(Stmt::Empty)
            }
            Some(Token::Int | Token::Char | Token::Void) => self.parse_var_decl(),
            _ => {
                let expr = self.parse_expr()?;
                self.expect(&Token::Semicolon)?;
                Ok(Stmt::Expr(expr))
            }
        }
    }

    fn parse_var_decl(&mut self) -> Result<Stmt, String> {
        let ty = self.parse_type()?;
        let name = match self.advance()? {
            Token::Identifier(n) => n,
            tok => return Err(format!("Expected variable name, got {tok}")),
        };
        let init = if self.current() == Some(&Token::Equal) {
            self.advance()?;
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect(&Token::Semicolon)?;
        Ok(Stmt::VarDecl(VarDecl { ty, name, init }))
    }

    fn parse_if(&mut self) -> Result<Stmt, String> {
        self.advance()?; // consume 'if'
        self.expect(&Token::OpenParen)?;
        let condition = self.parse_expr()?;
        self.expect(&Token::CloseParen)?;
        let then_branch = Box::new(self.parse_stmt()?);
        let else_branch = if self.current() == Some(&Token::Else) {
            self.advance()?;
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
        self.advance()?; // consume 'while'
        self.expect(&Token::OpenParen)?;
        let condition = self.parse_expr()?;
        self.expect(&Token::CloseParen)?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::While { condition, body })
    }

    fn parse_do_while(&mut self) -> Result<Stmt, String> {
        self.advance()?; // consume 'do'
        let body = Box::new(self.parse_stmt()?);
        self.expect(&Token::While)?;
        self.expect(&Token::OpenParen)?;
        let condition = self.parse_expr()?;
        self.expect(&Token::CloseParen)?;
        self.expect(&Token::Semicolon)?;
        Ok(Stmt::DoWhile { body, condition })
    }

    fn parse_for(&mut self) -> Result<Stmt, String> {
        self.advance()?; // consume 'for'
        self.expect(&Token::OpenParen)?;

        let init = if self.current() == Some(&Token::Semicolon) {
            self.advance()?;
            None
        } else if matches!(self.current(), Some(Token::Int | Token::Char | Token::Void)) {
            Some(Box::new(self.parse_var_decl()?))
        } else {
            let expr = self.parse_expr()?;
            self.expect(&Token::Semicolon)?;
            Some(Box::new(Stmt::Expr(expr)))
        };

        let condition = if self.current() == Some(&Token::Semicolon) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(&Token::Semicolon)?;

        let update = if self.current() == Some(&Token::CloseParen) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(&Token::CloseParen)?;

        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::For {
            init,
            condition,
            update,
            body,
        })
    }

    // Expression parsing with precedence climbing
    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expr, String> {
        let expr = self.parse_ternary()?;

        // Check for assignment operators
        if let Expr::Var(ref name) = expr {
            let name = name.clone();
            match self.current() {
                Some(Token::Equal) => {
                    self.advance()?;
                    let rhs = self.parse_assignment()?;
                    return Ok(Expr::Assign(name, Box::new(rhs)));
                }
                Some(Token::PlusEqual) => {
                    self.advance()?;
                    let rhs = self.parse_assignment()?;
                    return Ok(Expr::CompoundAssign(
                        CompoundOp::AddAssign,
                        name,
                        Box::new(rhs),
                    ));
                }
                Some(Token::MinusEqual) => {
                    self.advance()?;
                    let rhs = self.parse_assignment()?;
                    return Ok(Expr::CompoundAssign(
                        CompoundOp::SubAssign,
                        name,
                        Box::new(rhs),
                    ));
                }
                Some(Token::StarEqual) => {
                    self.advance()?;
                    let rhs = self.parse_assignment()?;
                    return Ok(Expr::CompoundAssign(
                        CompoundOp::MulAssign,
                        name,
                        Box::new(rhs),
                    ));
                }
                Some(Token::SlashEqual) => {
                    self.advance()?;
                    let rhs = self.parse_assignment()?;
                    return Ok(Expr::CompoundAssign(
                        CompoundOp::DivAssign,
                        name,
                        Box::new(rhs),
                    ));
                }
                Some(Token::PercentEqual) => {
                    self.advance()?;
                    let rhs = self.parse_assignment()?;
                    return Ok(Expr::CompoundAssign(
                        CompoundOp::ModAssign,
                        name,
                        Box::new(rhs),
                    ));
                }
                _ => {}
            }
        }
        Ok(expr)
    }

    fn parse_ternary(&mut self) -> Result<Expr, String> {
        let expr = self.parse_logical_or()?;
        if self.current() == Some(&Token::Question) {
            self.advance()?;
            let then_expr = self.parse_expr()?;
            self.expect(&Token::Colon)?;
            let else_expr = self.parse_ternary()?;
            Ok(Expr::Ternary(
                Box::new(expr),
                Box::new(then_expr),
                Box::new(else_expr),
            ))
        } else {
            Ok(expr)
        }
    }

    fn parse_logical_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_logical_and()?;
        while self.current() == Some(&Token::PipePipe) {
            self.advance()?;
            let right = self.parse_logical_and()?;
            left = Expr::BinaryOp(BinOp::LogicalOr, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_bitwise_or()?;
        while self.current() == Some(&Token::AmpersandAmpersand) {
            self.advance()?;
            let right = self.parse_bitwise_or()?;
            left = Expr::BinaryOp(BinOp::LogicalAnd, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_bitwise_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_bitwise_xor()?;
        while self.current() == Some(&Token::Pipe) {
            self.advance()?;
            let right = self.parse_bitwise_xor()?;
            left = Expr::BinaryOp(BinOp::BitwiseOr, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_bitwise_xor(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_bitwise_and()?;
        while self.current() == Some(&Token::Caret) {
            self.advance()?;
            let right = self.parse_bitwise_and()?;
            left = Expr::BinaryOp(BinOp::BitwiseXor, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_bitwise_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_equality()?;
        while self.current() == Some(&Token::Ampersand) {
            self.advance()?;
            let right = self.parse_equality()?;
            left = Expr::BinaryOp(BinOp::BitwiseAnd, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_relational()?;
        loop {
            let op = match self.current() {
                Some(Token::EqualEqual) => BinOp::Equal,
                Some(Token::BangEqual) => BinOp::NotEqual,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_relational()?;
            left = Expr::BinaryOp(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_relational(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_shift()?;
        loop {
            let op = match self.current() {
                Some(Token::Less) => BinOp::Less,
                Some(Token::Greater) => BinOp::Greater,
                Some(Token::LessEqual) => BinOp::LessEqual,
                Some(Token::GreaterEqual) => BinOp::GreaterEqual,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_shift()?;
            left = Expr::BinaryOp(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_additive()?;
        loop {
            let op = match self.current() {
                Some(Token::LessLess) => BinOp::ShiftLeft,
                Some(Token::GreaterGreater) => BinOp::ShiftRight,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_additive()?;
            left = Expr::BinaryOp(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.current() {
                Some(Token::Plus) => BinOp::Add,
                Some(Token::Minus) => BinOp::Sub,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_multiplicative()?;
            left = Expr::BinaryOp(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.current() {
                Some(Token::Star) => BinOp::Mul,
                Some(Token::Slash) => BinOp::Div,
                Some(Token::Percent) => BinOp::Mod,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_unary()?;
            left = Expr::BinaryOp(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.current() {
            Some(Token::Minus) => {
                self.advance()?;
                let expr = self.parse_unary()?;
                Ok(Expr::UnaryOp(UnaryOp::Negate, Box::new(expr)))
            }
            Some(Token::Tilde) => {
                self.advance()?;
                let expr = self.parse_unary()?;
                Ok(Expr::UnaryOp(UnaryOp::BitwiseNot, Box::new(expr)))
            }
            Some(Token::Bang) => {
                self.advance()?;
                let expr = self.parse_unary()?;
                Ok(Expr::UnaryOp(UnaryOp::LogicalNot, Box::new(expr)))
            }
            Some(Token::PlusPlus) => {
                self.advance()?;
                let name = match self.advance()? {
                    Token::Identifier(n) => n,
                    tok => return Err(format!("Expected identifier after ++, got {tok}")),
                };
                Ok(Expr::PreIncrement(name))
            }
            Some(Token::MinusMinus) => {
                self.advance()?;
                let name = match self.advance()? {
                    Token::Identifier(n) => n,
                    tok => return Err(format!("Expected identifier after --, got {tok}")),
                };
                Ok(Expr::PreDecrement(name))
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let expr = self.parse_primary()?;

        if let Expr::Var(ref name) = expr {
            let name = name.clone();
            match self.current() {
                Some(Token::PlusPlus) => {
                    self.advance()?;
                    return Ok(Expr::PostIncrement(name));
                }
                Some(Token::MinusMinus) => {
                    self.advance()?;
                    return Ok(Expr::PostDecrement(name));
                }
                _ => {}
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.advance()? {
            Token::IntLiteral(n) => Ok(Expr::IntLiteral(n)),
            Token::StringLiteral(s) => Ok(Expr::StringLiteral(s)),
            Token::Identifier(name) => {
                if self.current() == Some(&Token::OpenParen) {
                    self.advance()?;
                    let mut args = Vec::new();
                    if self.current() != Some(&Token::CloseParen) {
                        args.push(self.parse_expr()?);
                        while self.current() == Some(&Token::Comma) {
                            self.advance()?;
                            args.push(self.parse_expr()?);
                        }
                    }
                    self.expect(&Token::CloseParen)?;
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Var(name))
                }
            }
            Token::OpenParen => {
                let expr = self.parse_expr()?;
                self.expect(&Token::CloseParen)?;
                Ok(expr)
            }
            tok => Err(format!("Unexpected token in expression: {tok}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(input: &str) -> Program {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse_program().unwrap()
    }

    #[test]
    fn test_return_constant() {
        let prog = parse("int main() { return 42; }");
        assert_eq!(prog.declarations.len(), 1);
        if let Declaration::Function(f) = &prog.declarations[0] {
            assert_eq!(f.name, "main");
            assert_eq!(f.return_type, Type::Int);
        } else {
            panic!("Expected function declaration");
        }
    }

    #[test]
    fn test_variable_decl() {
        let prog = parse("int main() { int x = 5; return x; }");
        assert_eq!(prog.declarations.len(), 1);
    }

    #[test]
    fn test_if_else() {
        let prog = parse("int main() { if (1) return 1; else return 0; }");
        assert_eq!(prog.declarations.len(), 1);
    }

    #[test]
    fn test_for_loop() {
        let prog = parse(
            "int main() { int s = 0; for (int i = 0; i < 10; i = i + 1) s = s + i; return s; }",
        );
        assert_eq!(prog.declarations.len(), 1);
    }
}
