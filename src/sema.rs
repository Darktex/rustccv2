//! Semantic analysis pass for the C compiler.
//!
//! Performs:
//! - Identifier resolution (variables must be declared before use)
//! - Function arity checking (correct number of arguments)
//! - Type checking (basic type compatibility)
//! - Scope management (nested scopes, variable shadowing)
//! - Struct/enum/typedef registration

use crate::parser::{
    BinOp, Block, Declaration, EnumVariant, Expr, Program, Stmt, StructField, TypeSpec, VarDecl,
};
use std::collections::HashMap;

/// Semantic errors found during analysis.
#[derive(Debug, Clone)]
pub struct SemaError {
    pub message: String,
}

impl std::fmt::Display for SemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Information about a declared variable.
#[derive(Debug, Clone)]
struct VarInfo {
    type_spec: TypeSpec,
    #[allow(dead_code)]
    is_param: bool,
}

/// Information about a declared function.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct FuncInfo {
    return_type: TypeSpec,
    param_count: usize,
    is_variadic: bool,
    is_defined: bool,
}

/// A scope level containing variable bindings.
#[derive(Debug, Clone)]
struct Scope {
    vars: HashMap<String, VarInfo>,
}

impl Scope {
    fn new() -> Self {
        Scope {
            vars: HashMap::new(),
        }
    }
}

/// The semantic analyzer.
pub struct SemanticAnalyzer {
    /// Stack of scopes (innermost last).
    scopes: Vec<Scope>,
    /// Global function declarations.
    functions: HashMap<String, FuncInfo>,
    /// Type aliases from typedef.
    typedefs: HashMap<String, TypeSpec>,
    /// Struct definitions.
    struct_defs: HashMap<String, Vec<StructField>>,
    /// Union definitions.
    union_defs: HashMap<String, Vec<StructField>>,
    /// Enum definitions (variant name -> value).
    enum_values: HashMap<String, i64>,
    /// Collected errors (we report all errors, not just the first).
    errors: Vec<SemaError>,
    /// Collected warnings.
    warnings: Vec<SemaError>,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        SemanticAnalyzer {
            scopes: vec![Scope::new()], // global scope
            functions: HashMap::new(),
            typedefs: HashMap::new(),
            struct_defs: HashMap::new(),
            union_defs: HashMap::new(),
            enum_values: HashMap::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn error(&mut self, msg: impl Into<String>) {
        self.errors.push(SemaError {
            message: msg.into(),
        });
    }

    fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(SemaError {
            message: msg.into(),
        });
    }

    fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare_var(&mut self, name: &str, type_spec: &TypeSpec, is_param: bool) {
        // Check for redeclaration in the current scope
        if let Some(scope) = self.scopes.last() {
            if scope.vars.contains_key(name) {
                self.warn(format!("Variable '{}' redeclared in the same scope", name));
            }
        }
        if let Some(scope) = self.scopes.last_mut() {
            scope.vars.insert(
                name.to_string(),
                VarInfo {
                    type_spec: type_spec.clone(),
                    is_param,
                },
            );
        }
    }

    fn lookup_var(&self, name: &str) -> Option<&VarInfo> {
        // Search from innermost scope to outermost
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.vars.get(name) {
                return Some(info);
            }
        }
        None
    }

    fn register_typedef(&mut self, name: &str, type_spec: &TypeSpec) {
        self.typedefs.insert(name.to_string(), type_spec.clone());
    }

    fn register_struct(&mut self, name: &str, fields: &[StructField]) {
        // Check for duplicate field names
        let mut seen = std::collections::HashSet::new();
        for field in fields {
            if !seen.insert(&field.name) {
                self.error(format!(
                    "Duplicate field '{}' in struct '{}'",
                    field.name, name
                ));
            }
        }
        self.struct_defs.insert(name.to_string(), fields.to_vec());
    }

    fn register_union(&mut self, name: &str, fields: &[StructField]) {
        let mut seen = std::collections::HashSet::new();
        for field in fields {
            if !seen.insert(&field.name) {
                self.error(format!(
                    "Duplicate field '{}' in union '{}'",
                    field.name, name
                ));
            }
        }
        self.union_defs.insert(name.to_string(), fields.to_vec());
    }

    fn register_enum_variants(&mut self, variants: &[EnumVariant]) {
        for variant in variants {
            if self.enum_values.contains_key(&variant.name) {
                self.warn(format!("Enum value '{}' redefined", variant.name));
            }
            if let Some(val) = variant.value {
                self.enum_values.insert(variant.name.clone(), val);
            }
        }
    }

    /// Resolve a TypeSpec, expanding typedefs.
    #[allow(dead_code)]
    fn resolve_type(&self, ty: &TypeSpec) -> TypeSpec {
        match ty {
            TypeSpec::TypedefName(name) => {
                if let Some(resolved) = self.typedefs.get(name) {
                    self.resolve_type(resolved)
                } else {
                    ty.clone()
                }
            }
            TypeSpec::Pointer(inner) => TypeSpec::Pointer(Box::new(self.resolve_type(inner))),
            TypeSpec::Array(inner, size) => {
                TypeSpec::Array(Box::new(self.resolve_type(inner)), *size)
            }
            _ => ty.clone(),
        }
    }

    /// Check if a type is an integer type (for arithmetic operations).
    fn is_integer_type(ty: &TypeSpec) -> bool {
        matches!(
            ty,
            TypeSpec::Char
                | TypeSpec::SignedChar
                | TypeSpec::UnsignedChar
                | TypeSpec::Short
                | TypeSpec::UnsignedShort
                | TypeSpec::Int
                | TypeSpec::UnsignedInt
                | TypeSpec::Long
                | TypeSpec::UnsignedLong
        )
    }

    /// Check if a type is a scalar type (integer, pointer, or enum treated as int).
    fn is_scalar_type(ty: &TypeSpec) -> bool {
        Self::is_integer_type(ty) || matches!(ty, TypeSpec::Pointer(_) | TypeSpec::Enum(_, _))
    }

    /// Get the size of a type in bytes.
    #[allow(dead_code)]
    pub fn size_of(ty: &TypeSpec) -> usize {
        match ty {
            TypeSpec::Void => 0,
            TypeSpec::Char | TypeSpec::SignedChar | TypeSpec::UnsignedChar => 1,
            TypeSpec::Short | TypeSpec::UnsignedShort => 2,
            TypeSpec::Int | TypeSpec::UnsignedInt | TypeSpec::Enum(_, _) => 4,
            TypeSpec::Long | TypeSpec::UnsignedLong => 8,
            TypeSpec::Pointer(_) | TypeSpec::FunctionPointer { .. } => 8,
            TypeSpec::Array(elem, Some(count)) => Self::size_of(elem) * count,
            TypeSpec::Array(_, None) => 8, // flexible array, treat as pointer size
            TypeSpec::Struct(_, Some(fields)) => {
                fields.iter().map(|f| Self::size_of(&f.type_spec)).sum()
            }
            TypeSpec::Union(_, Some(fields)) => fields
                .iter()
                .map(|f| Self::size_of(&f.type_spec))
                .max()
                .unwrap_or(0),
            TypeSpec::Struct(_, None) | TypeSpec::Union(_, None) => 0, // incomplete type
            TypeSpec::TypedefName(_) => 4,                             // unresolved
        }
    }

    // ---- Analysis entry point ----

    pub fn analyze(&mut self, program: &Program) -> Result<(), Vec<SemaError>> {
        // First pass: register all top-level declarations
        for decl in &program.declarations {
            match decl {
                Declaration::Function(func) => {
                    let info = FuncInfo {
                        return_type: func.return_type.clone(),
                        param_count: func.params.len(),
                        is_variadic: func.is_variadic,
                        is_defined: func.body.is_some(),
                    };
                    if let Some(existing) = self.functions.get(&func.name) {
                        // Check for conflicting return types
                        if existing.return_type != func.return_type {
                            self.error(format!(
                                "Conflicting return types for function '{}'",
                                func.name
                            ));
                        }
                    }
                    self.functions.insert(func.name.clone(), info);
                }
                Declaration::GlobalVar(var) => {
                    self.declare_var(&var.name, &var.type_spec, false);
                }
                Declaration::Typedef(type_spec, name) => {
                    self.register_typedef(name, type_spec);
                }
                Declaration::StructDecl(type_spec) => {
                    self.register_type_decl(type_spec);
                }
            }
        }

        // Second pass: analyze function bodies
        for decl in &program.declarations {
            if let Declaration::Function(func) = decl {
                if let Some(body) = &func.body {
                    self.analyze_function(func, body);
                }
            }
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.clone())
        }
    }

    fn register_type_decl(&mut self, type_spec: &TypeSpec) {
        match type_spec {
            TypeSpec::Struct(name, Some(fields)) => {
                self.register_struct(name, fields);
            }
            TypeSpec::Union(name, Some(fields)) => {
                self.register_union(name, fields);
            }
            TypeSpec::Enum(_, Some(variants)) => {
                self.register_enum_variants(variants);
            }
            _ => {}
        }
    }

    fn analyze_function(&mut self, func: &crate::parser::FunctionDef, body: &Block) {
        self.push_scope();

        // Add parameters to scope
        for param in &func.params {
            if !param.name.is_empty() {
                self.declare_var(&param.name, &param.type_spec, true);
            }
        }

        // Analyze body
        for stmt in &body.stmts {
            self.analyze_stmt(stmt, &func.return_type);
        }

        self.pop_scope();
    }

    fn analyze_stmt(&mut self, stmt: &Stmt, return_type: &TypeSpec) {
        match stmt {
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    let expr_ty = self.analyze_expr(e);
                    // Check return type compatibility
                    if *return_type == TypeSpec::Void && expr_ty != TypeSpec::Void {
                        self.warn("Returning a value from void function".to_string());
                    }
                } else if *return_type != TypeSpec::Void {
                    // returning nothing from non-void is common in C (implicit return 0 for main)
                }
            }
            Stmt::Expr(expr) => {
                self.analyze_expr(expr);
            }
            Stmt::VarDecl(decl) => {
                self.analyze_var_decl(decl);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_ty = self.analyze_expr(condition);
                if !Self::is_scalar_type(&cond_ty) && cond_ty != TypeSpec::Void {
                    self.error("Condition in 'if' must be a scalar type".to_string());
                }
                self.analyze_stmt(then_branch, return_type);
                if let Some(else_br) = else_branch {
                    self.analyze_stmt(else_br, return_type);
                }
            }
            Stmt::While { condition, body } => {
                let cond_ty = self.analyze_expr(condition);
                if !Self::is_scalar_type(&cond_ty) && cond_ty != TypeSpec::Void {
                    self.error("Condition in 'while' must be a scalar type".to_string());
                }
                self.analyze_stmt(body, return_type);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
            } => {
                self.push_scope();
                if let Some(init) = init {
                    self.analyze_stmt(init, return_type);
                }
                if let Some(cond) = condition {
                    let cond_ty = self.analyze_expr(cond);
                    if !Self::is_scalar_type(&cond_ty) && cond_ty != TypeSpec::Void {
                        self.error("Condition in 'for' must be a scalar type".to_string());
                    }
                }
                if let Some(update) = update {
                    self.analyze_expr(update);
                }
                self.analyze_stmt(body, return_type);
                self.pop_scope();
            }
            Stmt::DoWhile { body, condition } => {
                self.analyze_stmt(body, return_type);
                let cond_ty = self.analyze_expr(condition);
                if !Self::is_scalar_type(&cond_ty) && cond_ty != TypeSpec::Void {
                    self.error("Condition in 'do-while' must be a scalar type".to_string());
                }
            }
            Stmt::Block(block) => {
                self.push_scope();
                for s in &block.stmts {
                    self.analyze_stmt(s, return_type);
                }
                self.pop_scope();
            }
            Stmt::Break | Stmt::Continue | Stmt::Empty => {}
            Stmt::Switch {
                expr,
                cases,
                default,
            } => {
                self.analyze_expr(expr);
                for case in cases {
                    self.analyze_expr(&case.value);
                    for stmt in &case.body {
                        self.analyze_stmt(stmt, return_type);
                    }
                }
                if let Some(default_stmts) = default {
                    for stmt in default_stmts {
                        self.analyze_stmt(stmt, return_type);
                    }
                }
            }
        }
    }

    fn analyze_var_decl(&mut self, decl: &VarDecl) {
        // Register any struct/enum types in the declaration
        self.register_type_decl(&decl.type_spec);

        // Analyze initializer
        if let Some(init) = &decl.init {
            self.analyze_expr(init);
        }

        self.declare_var(&decl.name, &decl.type_spec, false);
    }

    /// Analyze an expression and return its inferred type.
    fn analyze_expr(&mut self, expr: &Expr) -> TypeSpec {
        match expr {
            Expr::IntLiteral(_) => TypeSpec::Int,
            Expr::CharLiteral(_) => TypeSpec::Char,
            Expr::StringLiteral(_) => TypeSpec::Pointer(Box::new(TypeSpec::Char)),
            Expr::Identifier(name) => {
                // Check if it's an enum constant
                if self.enum_values.contains_key(name) {
                    return TypeSpec::Int;
                }
                // Check if it's a known function name
                if self.functions.contains_key(name) {
                    return TypeSpec::Int; // function used as value
                }
                // Look up in scopes
                if let Some(info) = self.lookup_var(name) {
                    return info.type_spec.clone();
                }
                self.error(format!("Use of undeclared identifier '{}'", name));
                TypeSpec::Int // assume int for error recovery
            }
            Expr::Binary { op, left, right } => {
                let left_ty = self.analyze_expr(left);
                let right_ty = self.analyze_expr(right);

                match op {
                    BinOp::Equal
                    | BinOp::NotEqual
                    | BinOp::Less
                    | BinOp::LessEqual
                    | BinOp::Greater
                    | BinOp::GreaterEqual
                    | BinOp::LogicalAnd
                    | BinOp::LogicalOr => TypeSpec::Int, // comparison yields int

                    BinOp::Add | BinOp::Sub => {
                        // Pointer arithmetic
                        if matches!(left_ty, TypeSpec::Pointer(_)) {
                            left_ty
                        } else if matches!(right_ty, TypeSpec::Pointer(_)) {
                            right_ty
                        } else {
                            self.promote_types(&left_ty, &right_ty)
                        }
                    }

                    _ => self.promote_types(&left_ty, &right_ty),
                }
            }
            Expr::Unary { operand, .. } => self.analyze_expr(operand),
            Expr::Assign { target, value } => {
                let target_ty = self.analyze_expr(target);
                self.analyze_expr(value);
                target_ty
            }
            Expr::CompoundAssign { target, value, .. } => {
                let target_ty = self.analyze_expr(target);
                self.analyze_expr(value);
                target_ty
            }
            Expr::Call { func, args } => {
                if let Expr::Identifier(name) = func.as_ref() {
                    if let Some(func_info) = self.functions.get(name).cloned() {
                        // Check argument count
                        if !func_info.is_variadic && args.len() != func_info.param_count {
                            self.error(format!(
                                "Function '{}' expects {} arguments, got {}",
                                name,
                                func_info.param_count,
                                args.len()
                            ));
                        } else if func_info.is_variadic && args.len() < func_info.param_count {
                            self.error(format!(
                                "Function '{}' expects at least {} arguments, got {}",
                                name,
                                func_info.param_count,
                                args.len()
                            ));
                        }
                        // Analyze arguments
                        for arg in args {
                            self.analyze_expr(arg);
                        }
                        return func_info.return_type;
                    }
                    // Unknown function - still analyze args
                    for arg in args {
                        self.analyze_expr(arg);
                    }
                    self.warn(format!("Implicit declaration of function '{}'", name));
                    TypeSpec::Int
                } else {
                    // Indirect call (function pointer)
                    self.analyze_expr(func);
                    for arg in args {
                        self.analyze_expr(arg);
                    }
                    TypeSpec::Int
                }
            }
            Expr::PostIncrement(operand)
            | Expr::PostDecrement(operand)
            | Expr::PreIncrement(operand)
            | Expr::PreDecrement(operand) => self.analyze_expr(operand),
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                let cond_ty = self.analyze_expr(condition);
                if !Self::is_scalar_type(&cond_ty) {
                    self.error("Condition in ternary must be a scalar type".to_string());
                }
                let then_ty = self.analyze_expr(then_expr);
                self.analyze_expr(else_expr);
                then_ty
            }
            Expr::Deref(operand) => {
                let ty = self.analyze_expr(operand);
                match ty {
                    TypeSpec::Pointer(inner) => *inner,
                    _ => {
                        self.error("Dereferencing a non-pointer type".to_string());
                        TypeSpec::Int
                    }
                }
            }
            Expr::AddrOf(operand) => {
                let ty = self.analyze_expr(operand);
                TypeSpec::Pointer(Box::new(ty))
            }
            Expr::ArraySubscript { array, index } => {
                let arr_ty = self.analyze_expr(array);
                self.analyze_expr(index);
                match arr_ty {
                    TypeSpec::Pointer(inner) | TypeSpec::Array(inner, _) => *inner,
                    _ => {
                        self.error("Subscript requires array or pointer type".to_string());
                        TypeSpec::Int
                    }
                }
            }
            Expr::Sizeof(_) => TypeSpec::UnsignedLong,
            Expr::Cast { type_spec, expr } => {
                self.analyze_expr(expr);
                type_spec.clone()
            }
        }
    }

    /// Perform "usual arithmetic conversions" - promote to the wider type.
    fn promote_types(&self, a: &TypeSpec, b: &TypeSpec) -> TypeSpec {
        let rank_a = Self::type_rank(a);
        let rank_b = Self::type_rank(b);
        if rank_a >= rank_b {
            a.clone()
        } else {
            b.clone()
        }
    }

    /// Integer conversion rank (higher = wider).
    fn type_rank(ty: &TypeSpec) -> u32 {
        match ty {
            TypeSpec::Char | TypeSpec::SignedChar | TypeSpec::UnsignedChar => 1,
            TypeSpec::Short | TypeSpec::UnsignedShort => 2,
            TypeSpec::Int | TypeSpec::UnsignedInt | TypeSpec::Enum(_, _) => 3,
            TypeSpec::Long | TypeSpec::UnsignedLong => 4,
            _ => 3, // default to int rank
        }
    }

    /// Get any warnings collected during analysis.
    #[allow(dead_code)]
    pub fn get_warnings(&self) -> &[SemaError] {
        &self.warnings
    }
}

/// Run semantic analysis on a parsed program.
/// Returns Ok(()) if no errors found, Err with list of errors otherwise.
pub fn analyze(program: &Program) -> Result<Vec<SemaError>, Vec<SemaError>> {
    let mut analyzer = SemanticAnalyzer::new();
    match analyzer.analyze(program) {
        Ok(()) => Ok(analyzer.warnings.clone()),
        Err(errors) => Err(errors),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;
    use crate::parser;

    fn analyze_source(source: &str) -> Result<Vec<SemaError>, Vec<SemaError>> {
        let tokens = lexer::lex(source).unwrap();
        let program = parser::parse(tokens).unwrap();
        analyze(&program)
    }

    #[test]
    fn test_valid_program() {
        let result = analyze_source("int main() { int x = 42; return x; }");
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    }

    #[test]
    fn test_undeclared_variable() {
        let result = analyze_source("int main() { return y; }");
        assert!(result.is_err(), "Expected error for undeclared variable");
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("undeclared")));
    }

    #[test]
    fn test_function_arity_mismatch() {
        let result = analyze_source(
            "int add(int a, int b) { return a + b; } int main() { return add(1, 2, 3); }",
        );
        assert!(result.is_err(), "Expected error for wrong arg count");
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.message.contains("expects 2 arguments")));
    }

    #[test]
    fn test_variadic_function() {
        let result = analyze_source(
            r#"int printf(const char *fmt, ...); int main() { printf("hello %d", 42); return 0; }"#,
        );
        assert!(
            result.is_ok(),
            "Variadic function should accept extra args: {:?}",
            result
        );
    }

    #[test]
    fn test_variable_shadowing() {
        let result = analyze_source("int main() { int x = 1; { int x = 2; return x; } }");
        // Shadowing is allowed in C (different scopes)
        assert!(
            result.is_ok(),
            "Variable shadowing should be allowed: {:?}",
            result
        );
    }

    #[test]
    fn test_struct_declaration() {
        let result = analyze_source("struct Point { int x; int y; }; int main() { return 0; }");
        assert!(
            result.is_ok(),
            "Struct declaration should be valid: {:?}",
            result
        );
    }

    #[test]
    fn test_enum_declaration() {
        let result = analyze_source(
            "enum Color { RED, GREEN, BLUE }; int main() { int c = RED; return c; }",
        );
        assert!(result.is_ok(), "Enum usage should be valid: {:?}", result);
    }

    #[test]
    fn test_typedef() {
        let result = analyze_source("typedef int i32; int main() { return 0; }");
        assert!(result.is_ok(), "Typedef should be valid: {:?}", result);
    }

    #[test]
    fn test_multiple_types() {
        let result = analyze_source(
            "int main() { char c = 65; short s = 100; long l = 1000; unsigned int u = 42; return 0; }",
        );
        assert!(
            result.is_ok(),
            "Multiple integer types should work: {:?}",
            result
        );
    }

    #[test]
    fn test_sizeof() {
        let result = analyze_source("int main() { return sizeof(int); }");
        assert!(result.is_ok(), "sizeof should work: {:?}", result);
    }

    #[test]
    fn test_struct_duplicate_field() {
        let result = analyze_source("struct Bad { int x; int x; }; int main() { return 0; }");
        assert!(
            result.is_err(),
            "Duplicate struct fields should be an error"
        );
    }

    #[test]
    fn test_function_type_conflict() {
        let result = analyze_source("int foo(); long foo(); int main() { return 0; }");
        assert!(
            result.is_err(),
            "Conflicting return types should be an error"
        );
    }

    #[test]
    fn test_for_loop_scoping() {
        let result = analyze_source(
            "int main() { for (int i = 0; i < 10; i = i + 1) { int j = i; } return 0; }",
        );
        assert!(result.is_ok(), "For loop scoping should work: {:?}", result);
    }

    #[test]
    fn test_nested_scopes() {
        let result =
            analyze_source("int main() { int a = 1; { int b = 2; { int c = a + b; } } return a; }");
        assert!(result.is_ok(), "Nested scopes should work: {:?}", result);
    }

    #[test]
    fn test_size_of_types() {
        assert_eq!(SemanticAnalyzer::size_of(&TypeSpec::Char), 1);
        assert_eq!(SemanticAnalyzer::size_of(&TypeSpec::Short), 2);
        assert_eq!(SemanticAnalyzer::size_of(&TypeSpec::Int), 4);
        assert_eq!(SemanticAnalyzer::size_of(&TypeSpec::Long), 8);
        assert_eq!(
            SemanticAnalyzer::size_of(&TypeSpec::Pointer(Box::new(TypeSpec::Int))),
            8
        );
        assert_eq!(
            SemanticAnalyzer::size_of(&TypeSpec::Array(Box::new(TypeSpec::Int), Some(10))),
            40
        );
    }

    #[test]
    fn test_deref_non_pointer() {
        let result = analyze_source("int main() { int x = 5; int y = *x; return y; }");
        assert!(
            result.is_err(),
            "Dereferencing non-pointer should be an error"
        );
    }

    #[test]
    fn test_implicit_function_declaration_warning() {
        let result = analyze_source("int main() { puts(\"hello\"); return 0; }");
        // This should succeed (implicit decl is a warning, not error) but with warnings
        assert!(
            result.is_ok(),
            "Implicit function decl should be a warning: {:?}",
            result
        );
        let warnings = result.unwrap();
        assert!(warnings
            .iter()
            .any(|w| w.message.contains("Implicit declaration")));
    }

    #[test]
    fn test_enum_with_explicit_values() {
        let result = analyze_source(
            "enum Flags { A = 1, B = 2, C = 4 }; int main() { int f = A; return f; }",
        );
        assert!(
            result.is_ok(),
            "Enum with explicit values should work: {:?}",
            result
        );
    }
}
