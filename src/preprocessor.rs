//! C Preprocessor
//!
//! Processes C source code before lexing, handling:
//! - `#include "file"` and `#include <file>`
//! - `#define` (object-like and function-like macros)
//! - `#undef`
//! - `#ifdef` / `#ifndef` / `#if` / `#elif` / `#else` / `#endif`
//! - `#pragma` (ignored)
//! - `#error`
//! - `#line`
//! - Predefined macros: `__FILE__`, `__LINE__`, `__DATE__`, `__TIME__`
//! - Macro operators: `#` (stringification), `##` (token pasting)

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A macro definition.
#[derive(Debug, Clone)]
enum MacroDef {
    /// Object-like macro: `#define FOO value`
    ObjectLike { body: String },
    /// Function-like macro: `#define FOO(a, b) a + b`
    FunctionLike { params: Vec<String>, body: String },
}

/// Preprocessor state.
pub struct Preprocessor {
    /// Defined macros.
    defines: HashMap<String, MacroDef>,
    /// Include search paths.
    include_paths: Vec<PathBuf>,
    /// Current file name (for `__FILE__`).
    current_file: String,
    /// Current line number (for `__LINE__`).
    current_line: usize,
    /// Conditional compilation stack.
    /// Each entry is (active, has_been_true, is_else_seen).
    cond_stack: Vec<(bool, bool, bool)>,
    /// Max include depth to prevent infinite recursion.
    max_include_depth: usize,
    /// Current include depth.
    include_depth: usize,
}

impl Preprocessor {
    pub fn new(filename: &str, include_paths: Vec<PathBuf>) -> Self {
        let mut defines = HashMap::new();
        // Predefined macros
        defines.insert(
            "__STDC__".to_string(),
            MacroDef::ObjectLike {
                body: "1".to_string(),
            },
        );
        defines.insert(
            "__STDC_VERSION__".to_string(),
            MacroDef::ObjectLike {
                body: "199901L".to_string(),
            },
        );

        Preprocessor {
            defines,
            include_paths,
            current_file: filename.to_string(),
            current_line: 0,
            cond_stack: Vec::new(),
            max_include_depth: 64,
            include_depth: 0,
        }
    }

    /// Check if we're in an active (not skipped) section.
    fn is_active(&self) -> bool {
        self.cond_stack.iter().all(|(active, _, _)| *active)
    }

    /// Process source code and return preprocessed output.
    pub fn preprocess(&mut self, source: &str) -> Result<String, String> {
        let mut output = String::new();
        let lines: Vec<&str> = source.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            self.current_line = i + 1;
            let line = lines[i];
            // Handle line continuations
            let mut full_line = line.to_string();
            while full_line.ends_with('\\') && i + 1 < lines.len() {
                full_line.pop(); // remove backslash
                i += 1;
                full_line.push_str(lines[i]);
            }
            let trimmed = full_line.trim();

            if trimmed.starts_with('#') {
                self.process_directive(trimmed, &mut output)?;
            } else if self.is_active() {
                let expanded = self.expand_macros(&full_line)?;
                output.push_str(&expanded);
                output.push('\n');
            } else {
                // In a skipped section, emit empty line to preserve line numbers
                output.push('\n');
            }

            i += 1;
        }

        if !self.cond_stack.is_empty() {
            return Err(format!(
                "{}:{}: unterminated conditional directive ({} level(s) deep)",
                self.current_file,
                self.current_line,
                self.cond_stack.len()
            ));
        }

        Ok(output)
    }

    /// Process a preprocessor directive line.
    fn process_directive(&mut self, line: &str, output: &mut String) -> Result<(), String> {
        // Strip the '#' and any leading whitespace after it
        let after_hash = line[1..].trim_start();

        // Get the directive name
        let (directive, rest) = match after_hash.find(|c: char| c.is_whitespace()) {
            Some(pos) => (&after_hash[..pos], after_hash[pos..].trim()),
            None => (after_hash, ""),
        };

        match directive {
            "define" => {
                if self.is_active() {
                    self.process_define(rest)?;
                }
            }
            "undef" => {
                if self.is_active() {
                    let name = rest.trim();
                    self.defines.remove(name);
                }
            }
            "include" => {
                if self.is_active() {
                    let included = self.process_include(rest)?;
                    output.push_str(&included);
                }
            }
            "ifdef" => {
                let name = rest.trim();
                let defined = self.defines.contains_key(name);
                if self.is_active() {
                    self.cond_stack.push((defined, defined, false));
                } else {
                    self.cond_stack.push((false, false, false));
                }
            }
            "ifndef" => {
                let name = rest.trim();
                let not_defined = !self.defines.contains_key(name);
                if self.is_active() {
                    self.cond_stack.push((not_defined, not_defined, false));
                } else {
                    self.cond_stack.push((false, false, false));
                }
            }
            "if" => {
                if self.is_active() {
                    let val = self.evaluate_condition(rest)?;
                    self.cond_stack.push((val, val, false));
                } else {
                    self.cond_stack.push((false, false, false));
                }
            }
            "elif" => {
                if self.cond_stack.is_empty() {
                    return Err(format!(
                        "{}:{}: #elif without #if",
                        self.current_file, self.current_line
                    ));
                }
                let len = self.cond_stack.len();
                let (_, _, else_seen) = self.cond_stack[len - 1];
                if else_seen {
                    return Err(format!(
                        "{}:{}: #elif after #else",
                        self.current_file, self.current_line
                    ));
                }
                let (_, has_been_true, _) = self.cond_stack[len - 1];
                let parent_active =
                    len <= 1 || self.cond_stack[..len - 1].iter().all(|(a, _, _)| *a);
                if parent_active && !has_been_true {
                    let val = self.evaluate_condition(rest)?;
                    self.cond_stack[len - 1].0 = val;
                    if val {
                        self.cond_stack[len - 1].1 = true;
                    }
                } else {
                    self.cond_stack[len - 1].0 = false;
                }
            }
            "else" => {
                if self.cond_stack.is_empty() {
                    return Err(format!(
                        "{}:{}: #else without #if",
                        self.current_file, self.current_line
                    ));
                }
                let len = self.cond_stack.len();
                let (_, _, else_seen) = self.cond_stack[len - 1];
                if else_seen {
                    return Err(format!(
                        "{}:{}: duplicate #else",
                        self.current_file, self.current_line
                    ));
                }
                self.cond_stack[len - 1].2 = true; // mark else seen
                let has_been_true = self.cond_stack[len - 1].1;
                let parent_active =
                    len <= 1 || self.cond_stack[..len - 1].iter().all(|(a, _, _)| *a);
                self.cond_stack[len - 1].0 = parent_active && !has_been_true;
            }
            "endif" => {
                if self.cond_stack.pop().is_none() {
                    return Err(format!(
                        "{}:{}: #endif without #if",
                        self.current_file, self.current_line
                    ));
                }
            }
            "error" => {
                if self.is_active() {
                    return Err(format!(
                        "{}:{}: #error {}",
                        self.current_file, self.current_line, rest
                    ));
                }
            }
            "pragma" | "line" => {
                // Silently ignore
            }
            "" => {
                // Bare `#` is allowed in C (null directive)
            }
            _ => {
                if self.is_active() {
                    return Err(format!(
                        "{}:{}: unknown preprocessor directive '#{}' ",
                        self.current_file, self.current_line, directive
                    ));
                }
            }
        }

        // Emit empty line to preserve line numbers
        output.push('\n');
        Ok(())
    }

    /// Process a `#define` directive.
    fn process_define(&mut self, rest: &str) -> Result<(), String> {
        if rest.is_empty() {
            return Err(format!(
                "{}:{}: #define requires a name",
                self.current_file, self.current_line
            ));
        }

        // Parse macro name
        let mut chars = rest.chars().peekable();
        let mut name = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_alphanumeric() || c == '_' {
                name.push(c);
                chars.next();
            } else {
                break;
            }
        }

        if name.is_empty() {
            return Err(format!(
                "{}:{}: invalid macro name",
                self.current_file, self.current_line
            ));
        }

        // Check for function-like macro (lparen immediately after name, no space)
        if chars.peek() == Some(&'(') {
            chars.next(); // consume '('
            let mut params = Vec::new();

            loop {
                // Skip whitespace
                while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                    chars.next();
                }

                if chars.peek() == Some(&')') {
                    chars.next();
                    break;
                }

                if !params.is_empty() && chars.peek() == Some(&',') {
                    chars.next();
                    // Skip whitespace after comma
                    while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                        chars.next();
                    }
                }

                // Check for variadic
                if chars.peek() == Some(&'.') {
                    chars.next();
                    chars.next();
                    chars.next(); // consume "..."
                    while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                        chars.next();
                    }
                    if chars.peek() == Some(&')') {
                        chars.next();
                        params.push("__VA_ARGS__".to_string());
                        break;
                    }
                    return Err(format!(
                        "{}:{}: expected ')' after '...'",
                        self.current_file, self.current_line
                    ));
                }

                // Read param name
                let mut param = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        param.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if param.is_empty() {
                    return Err(format!(
                        "{}:{}: expected parameter name",
                        self.current_file, self.current_line
                    ));
                }
                params.push(param);
            }

            // Rest is the body
            let body: String = chars.collect();
            let body = body.trim().to_string();

            self.defines
                .insert(name, MacroDef::FunctionLike { params, body });
        } else {
            // Object-like macro - skip whitespace then take the rest
            let body: String = chars.collect();
            let body = body.trim().to_string();

            self.defines.insert(name, MacroDef::ObjectLike { body });
        }

        Ok(())
    }

    /// Process an `#include` directive.
    fn process_include(&mut self, rest: &str) -> Result<String, String> {
        if self.include_depth >= self.max_include_depth {
            return Err(format!(
                "{}:{}: #include nested too deeply",
                self.current_file, self.current_line
            ));
        }

        let rest = rest.trim();

        let (filename, is_system) = if let Some(after_quote) = rest.strip_prefix('"') {
            // #include "file"
            let end = after_quote.find('"').ok_or_else(|| {
                format!(
                    "{}:{}: unterminated #include filename",
                    self.current_file, self.current_line
                )
            })?;
            (&after_quote[..end], false)
        } else if let Some(after_angle) = rest.strip_prefix('<') {
            // #include <file>
            let end = after_angle.find('>').ok_or_else(|| {
                format!(
                    "{}:{}: unterminated #include filename",
                    self.current_file, self.current_line
                )
            })?;
            (&after_angle[..end], true)
        } else {
            return Err(format!(
                "{}:{}: expected '\"' or '<' after #include",
                self.current_file, self.current_line
            ));
        };

        // Resolve the file path
        let resolved = self.resolve_include(filename, is_system)?;

        // Read the file
        let content = std::fs::read_to_string(&resolved).map_err(|e| {
            format!(
                "{}:{}: cannot read '{}': {}",
                self.current_file,
                self.current_line,
                resolved.display(),
                e
            )
        })?;

        // Recursively preprocess
        let saved_file = self.current_file.clone();
        let saved_line = self.current_line;

        self.current_file = resolved.to_string_lossy().to_string();
        self.include_depth += 1;

        let result = self.preprocess(&content);

        self.current_file = saved_file;
        self.current_line = saved_line;
        self.include_depth -= 1;

        result
    }

    /// Resolve an include path.
    fn resolve_include(&self, filename: &str, is_system: bool) -> Result<PathBuf, String> {
        if !is_system {
            // For quoted includes, first search relative to the current file
            let current_dir = Path::new(&self.current_file)
                .parent()
                .unwrap_or(Path::new("."));
            let candidate = current_dir.join(filename);
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        // Search include paths
        for path in &self.include_paths {
            let candidate = path.join(filename);
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        Err(format!(
            "{}:{}: '{}': file not found",
            self.current_file, self.current_line, filename
        ))
    }

    /// Expand macros in a line of text.
    fn expand_macros(&self, text: &str) -> Result<String, String> {
        self.expand_macros_recursive(text, &mut Vec::new(), 0)
    }

    /// Recursively expand macros, tracking already-expanded names to prevent infinite recursion.
    fn expand_macros_recursive(
        &self,
        text: &str,
        expanding: &mut Vec<String>,
        depth: usize,
    ) -> Result<String, String> {
        if depth > 256 {
            return Err(format!(
                "{}:{}: macro expansion too deep",
                self.current_file, self.current_line
            ));
        }

        let mut result = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut pos = 0;

        while pos < chars.len() {
            // Skip string literals
            if chars[pos] == '"' {
                result.push(chars[pos]);
                pos += 1;
                while pos < chars.len() && chars[pos] != '"' {
                    if chars[pos] == '\\' && pos + 1 < chars.len() {
                        result.push(chars[pos]);
                        pos += 1;
                    }
                    result.push(chars[pos]);
                    pos += 1;
                }
                if pos < chars.len() {
                    result.push(chars[pos]);
                    pos += 1;
                }
                continue;
            }

            // Skip char literals
            if chars[pos] == '\'' {
                result.push(chars[pos]);
                pos += 1;
                while pos < chars.len() && chars[pos] != '\'' {
                    if chars[pos] == '\\' && pos + 1 < chars.len() {
                        result.push(chars[pos]);
                        pos += 1;
                    }
                    result.push(chars[pos]);
                    pos += 1;
                }
                if pos < chars.len() {
                    result.push(chars[pos]);
                    pos += 1;
                }
                continue;
            }

            // Skip line comments
            if chars[pos] == '/' && pos + 1 < chars.len() && chars[pos + 1] == '/' {
                // Copy rest of line as-is
                while pos < chars.len() {
                    result.push(chars[pos]);
                    pos += 1;
                }
                continue;
            }

            // Try to read an identifier
            if chars[pos].is_ascii_alphabetic() || chars[pos] == '_' {
                let start = pos;
                while pos < chars.len() && (chars[pos].is_ascii_alphanumeric() || chars[pos] == '_')
                {
                    pos += 1;
                }
                let ident: String = chars[start..pos].iter().collect();

                // Check for predefined macros
                if ident == "__FILE__" && !expanding.contains(&ident) {
                    result.push('"');
                    result.push_str(&self.current_file);
                    result.push('"');
                    continue;
                }
                if ident == "__LINE__" && !expanding.contains(&ident) {
                    result.push_str(&self.current_line.to_string());
                    continue;
                }
                if ident == "__DATE__" && !expanding.contains(&ident) {
                    result.push_str("\"Mar 20 2026\"");
                    continue;
                }
                if ident == "__TIME__" && !expanding.contains(&ident) {
                    result.push_str("\"00:00:00\"");
                    continue;
                }
                if ident == "defined" {
                    // Handle `defined(MACRO)` or `defined MACRO` in #if expressions
                    result.push_str(&ident);
                    continue;
                }

                // Check for macro expansion
                if !expanding.contains(&ident) {
                    if let Some(mac) = self.defines.get(&ident) {
                        match mac {
                            MacroDef::ObjectLike { body } => {
                                expanding.push(ident.clone());
                                let expanded =
                                    self.expand_macros_recursive(body, expanding, depth + 1)?;
                                result.push_str(&expanded);
                                expanding.pop();
                                continue;
                            }
                            MacroDef::FunctionLike { params, body } => {
                                // Check if followed by '('
                                let mut look = pos;
                                while look < chars.len() && chars[look].is_whitespace() {
                                    look += 1;
                                }
                                if look < chars.len() && chars[look] == '(' {
                                    // Parse arguments
                                    let args =
                                        self.parse_macro_args(&chars, &mut look, params.len())?;
                                    pos = look;

                                    // Substitute parameters
                                    let substituted =
                                        self.substitute_params(body, params, &args)?;

                                    expanding.push(ident.clone());
                                    let expanded = self.expand_macros_recursive(
                                        &substituted,
                                        expanding,
                                        depth + 1,
                                    )?;
                                    result.push_str(&expanded);
                                    expanding.pop();
                                    continue;
                                }
                                // Not followed by '(' - don't expand
                            }
                        }
                    }
                }

                result.push_str(&ident);
                continue;
            }

            result.push(chars[pos]);
            pos += 1;
        }

        Ok(result)
    }

    /// Parse macro call arguments from source text.
    fn parse_macro_args(
        &self,
        chars: &[char],
        pos: &mut usize,
        _expected: usize,
    ) -> Result<Vec<String>, String> {
        // pos should be at '('
        *pos += 1; // skip '('
        let mut args = Vec::new();
        let mut current = String::new();
        let mut depth = 1;

        while *pos < chars.len() && depth > 0 {
            let c = chars[*pos];
            match c {
                '(' => {
                    depth += 1;
                    current.push(c);
                }
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        let trimmed = current.trim().to_string();
                        if !trimmed.is_empty() || !args.is_empty() {
                            args.push(trimmed);
                        }
                    } else {
                        current.push(c);
                    }
                }
                ',' if depth == 1 => {
                    args.push(current.trim().to_string());
                    current = String::new();
                }
                '"' => {
                    // String literal in argument
                    current.push(c);
                    *pos += 1;
                    while *pos < chars.len() && chars[*pos] != '"' {
                        if chars[*pos] == '\\' && *pos + 1 < chars.len() {
                            current.push(chars[*pos]);
                            *pos += 1;
                        }
                        current.push(chars[*pos]);
                        *pos += 1;
                    }
                    if *pos < chars.len() {
                        current.push(chars[*pos]);
                    }
                }
                '\'' => {
                    // Char literal in argument
                    current.push(c);
                    *pos += 1;
                    while *pos < chars.len() && chars[*pos] != '\'' {
                        if chars[*pos] == '\\' && *pos + 1 < chars.len() {
                            current.push(chars[*pos]);
                            *pos += 1;
                        }
                        current.push(chars[*pos]);
                        *pos += 1;
                    }
                    if *pos < chars.len() {
                        current.push(chars[*pos]);
                    }
                }
                _ => {
                    current.push(c);
                }
            }
            *pos += 1;
        }

        if depth != 0 {
            return Err(format!(
                "{}:{}: unterminated macro argument list",
                self.current_file, self.current_line
            ));
        }

        Ok(args)
    }

    /// Substitute macro parameters in the body, handling # and ##.
    fn substitute_params(
        &self,
        body: &str,
        params: &[String],
        args: &[String],
    ) -> Result<String, String> {
        let chars: Vec<char> = body.chars().collect();
        let mut result = String::new();
        let mut pos = 0;

        while pos < chars.len() {
            // Token pasting operator ##
            if chars[pos] == '#' && pos + 1 < chars.len() && chars[pos + 1] == '#' {
                // Remove trailing whitespace from result
                while result.ends_with(' ') || result.ends_with('\t') {
                    result.pop();
                }
                pos += 2;
                // Skip whitespace after ##
                while pos < chars.len() && (chars[pos] == ' ' || chars[pos] == '\t') {
                    pos += 1;
                }
                continue;
            }

            // Stringification operator #
            if chars[pos] == '#' {
                pos += 1;
                // Skip whitespace
                while pos < chars.len() && (chars[pos] == ' ' || chars[pos] == '\t') {
                    pos += 1;
                }
                // Read identifier
                let start = pos;
                while pos < chars.len() && (chars[pos].is_ascii_alphanumeric() || chars[pos] == '_')
                {
                    pos += 1;
                }
                let param_name: String = chars[start..pos].iter().collect();
                if let Some(idx) = params.iter().position(|p| p == &param_name) {
                    let arg = args.get(idx).map(|s| s.as_str()).unwrap_or("");
                    result.push('"');
                    // Escape special chars in stringification
                    for c in arg.chars() {
                        if c == '"' || c == '\\' {
                            result.push('\\');
                        }
                        result.push(c);
                    }
                    result.push('"');
                } else {
                    result.push('#');
                    result.push_str(&param_name);
                }
                continue;
            }

            // Identifier - check if it's a parameter
            if chars[pos].is_ascii_alphabetic() || chars[pos] == '_' {
                let start = pos;
                while pos < chars.len() && (chars[pos].is_ascii_alphanumeric() || chars[pos] == '_')
                {
                    pos += 1;
                }
                let ident: String = chars[start..pos].iter().collect();
                if let Some(idx) = params.iter().position(|p| p == &ident) {
                    let arg = args.get(idx).map(|s| s.as_str()).unwrap_or("");
                    result.push_str(arg);
                } else {
                    result.push_str(&ident);
                }
                continue;
            }

            result.push(chars[pos]);
            pos += 1;
        }

        Ok(result)
    }

    /// Evaluate a `#if` / `#elif` condition expression.
    fn evaluate_condition(&self, expr: &str) -> Result<bool, String> {
        // First expand macros in the expression
        let expanded = self.expand_condition_expr(expr)?;
        // Parse and evaluate the constant expression
        let val = self.eval_const_expr(&expanded)?;
        Ok(val != 0)
    }

    /// Expand macros in a condition expression, handling `defined(X)` and `defined X`.
    fn expand_condition_expr(&self, expr: &str) -> Result<String, String> {
        let chars: Vec<char> = expr.chars().collect();
        let mut result = String::new();
        let mut pos = 0;

        while pos < chars.len() {
            if chars[pos].is_ascii_alphabetic() || chars[pos] == '_' {
                let start = pos;
                while pos < chars.len() && (chars[pos].is_ascii_alphanumeric() || chars[pos] == '_')
                {
                    pos += 1;
                }
                let ident: String = chars[start..pos].iter().collect();

                if ident == "defined" {
                    // Handle defined(X) or defined X
                    while pos < chars.len() && chars[pos].is_whitespace() {
                        pos += 1;
                    }
                    let has_paren = pos < chars.len() && chars[pos] == '(';
                    if has_paren {
                        pos += 1;
                    }
                    while pos < chars.len() && chars[pos].is_whitespace() {
                        pos += 1;
                    }
                    let name_start = pos;
                    while pos < chars.len()
                        && (chars[pos].is_ascii_alphanumeric() || chars[pos] == '_')
                    {
                        pos += 1;
                    }
                    let name: String = chars[name_start..pos].iter().collect();
                    if has_paren {
                        while pos < chars.len() && chars[pos].is_whitespace() {
                            pos += 1;
                        }
                        if pos < chars.len() && chars[pos] == ')' {
                            pos += 1;
                        }
                    }
                    let defined = self.defines.contains_key(&name);
                    result.push_str(if defined { "1" } else { "0" });
                } else if let Some(MacroDef::ObjectLike { body }) = self.defines.get(&ident) {
                    result.push_str(body);
                } else {
                    // Undefined identifiers become 0 in #if expressions
                    result.push('0');
                }
            } else {
                result.push(chars[pos]);
                pos += 1;
            }
        }

        Ok(result)
    }

    /// Evaluate a constant expression (simplified: handles integers, +, -, *, /, %, comparisons, &&, ||, !).
    fn eval_const_expr(&self, expr: &str) -> Result<i64, String> {
        let tokens = self.tokenize_const_expr(expr)?;
        let mut pos = 0;
        let result = self.parse_ternary(&tokens, &mut pos)?;
        Ok(result)
    }

    /// Tokenize a constant expression.
    fn tokenize_const_expr(&self, expr: &str) -> Result<Vec<CondToken>, String> {
        let chars: Vec<char> = expr.chars().collect();
        let mut tokens = Vec::new();
        let mut pos = 0;

        while pos < chars.len() {
            if chars[pos].is_whitespace() {
                pos += 1;
                continue;
            }

            if chars[pos].is_ascii_digit() {
                let start = pos;
                if chars[pos] == '0'
                    && pos + 1 < chars.len()
                    && (chars[pos + 1] == 'x' || chars[pos + 1] == 'X')
                {
                    pos += 2;
                    while pos < chars.len() && chars[pos].is_ascii_hexdigit() {
                        pos += 1;
                    }
                    let s: String = chars[start..pos].iter().collect();
                    let val = i64::from_str_radix(&s[2..], 16).unwrap_or(0);
                    // Skip suffixes
                    while pos < chars.len() && matches!(chars[pos], 'u' | 'U' | 'l' | 'L') {
                        pos += 1;
                    }
                    tokens.push(CondToken::Num(val));
                } else {
                    while pos < chars.len() && chars[pos].is_ascii_digit() {
                        pos += 1;
                    }
                    let s: String = chars[start..pos].iter().collect();
                    let val: i64 = s.parse().unwrap_or(0);
                    // Skip suffixes
                    while pos < chars.len() && matches!(chars[pos], 'u' | 'U' | 'l' | 'L') {
                        pos += 1;
                    }
                    tokens.push(CondToken::Num(val));
                }
                continue;
            }

            // Character literal
            if chars[pos] == '\'' {
                pos += 1;
                let c = if pos < chars.len() && chars[pos] == '\\' {
                    pos += 1;
                    match chars.get(pos) {
                        Some('n') => '\n',
                        Some('t') => '\t',
                        Some('0') => '\0',
                        Some('\\') => '\\',
                        Some('\'') => '\'',
                        Some(&c) => c,
                        None => return Err("unterminated char literal in expression".to_string()),
                    }
                } else if pos < chars.len() {
                    chars[pos]
                } else {
                    return Err("unterminated char literal in expression".to_string());
                };
                pos += 1;
                if pos < chars.len() && chars[pos] == '\'' {
                    pos += 1;
                }
                tokens.push(CondToken::Num(c as i64));
                continue;
            }

            match chars[pos] {
                '+' => {
                    tokens.push(CondToken::Plus);
                    pos += 1;
                }
                '-' => {
                    tokens.push(CondToken::Minus);
                    pos += 1;
                }
                '*' => {
                    tokens.push(CondToken::Star);
                    pos += 1;
                }
                '/' => {
                    tokens.push(CondToken::Slash);
                    pos += 1;
                }
                '%' => {
                    tokens.push(CondToken::Percent);
                    pos += 1;
                }
                '(' => {
                    tokens.push(CondToken::LParen);
                    pos += 1;
                }
                ')' => {
                    tokens.push(CondToken::RParen);
                    pos += 1;
                }
                '~' => {
                    tokens.push(CondToken::Tilde);
                    pos += 1;
                }
                '?' => {
                    tokens.push(CondToken::Question);
                    pos += 1;
                }
                ':' => {
                    tokens.push(CondToken::Colon);
                    pos += 1;
                }
                '!' => {
                    pos += 1;
                    if pos < chars.len() && chars[pos] == '=' {
                        tokens.push(CondToken::NotEq);
                        pos += 1;
                    } else {
                        tokens.push(CondToken::Bang);
                    }
                }
                '=' => {
                    pos += 1;
                    if pos < chars.len() && chars[pos] == '=' {
                        tokens.push(CondToken::EqEq);
                        pos += 1;
                    }
                }
                '<' => {
                    pos += 1;
                    if pos < chars.len() && chars[pos] == '=' {
                        tokens.push(CondToken::LessEq);
                        pos += 1;
                    } else if pos < chars.len() && chars[pos] == '<' {
                        tokens.push(CondToken::LShift);
                        pos += 1;
                    } else {
                        tokens.push(CondToken::Less);
                    }
                }
                '>' => {
                    pos += 1;
                    if pos < chars.len() && chars[pos] == '=' {
                        tokens.push(CondToken::GreaterEq);
                        pos += 1;
                    } else if pos < chars.len() && chars[pos] == '>' {
                        tokens.push(CondToken::RShift);
                        pos += 1;
                    } else {
                        tokens.push(CondToken::Greater);
                    }
                }
                '&' => {
                    pos += 1;
                    if pos < chars.len() && chars[pos] == '&' {
                        tokens.push(CondToken::AmpAmp);
                        pos += 1;
                    } else {
                        tokens.push(CondToken::Amp);
                    }
                }
                '|' => {
                    pos += 1;
                    if pos < chars.len() && chars[pos] == '|' {
                        tokens.push(CondToken::PipePipe);
                        pos += 1;
                    } else {
                        tokens.push(CondToken::Pipe);
                    }
                }
                '^' => {
                    tokens.push(CondToken::Caret);
                    pos += 1;
                }
                _ => {
                    // Skip unknown characters (e.g. from macro expansion residue)
                    pos += 1;
                }
            }
        }

        Ok(tokens)
    }

    // Expression parsing using precedence climbing

    fn parse_ternary(&self, tokens: &[CondToken], pos: &mut usize) -> Result<i64, String> {
        let cond = self.parse_or(tokens, pos)?;
        if *pos < tokens.len() && tokens[*pos] == CondToken::Question {
            *pos += 1;
            let then_val = self.parse_ternary(tokens, pos)?;
            if *pos < tokens.len() && tokens[*pos] == CondToken::Colon {
                *pos += 1;
            }
            let else_val = self.parse_ternary(tokens, pos)?;
            Ok(if cond != 0 { then_val } else { else_val })
        } else {
            Ok(cond)
        }
    }

    fn parse_or(&self, tokens: &[CondToken], pos: &mut usize) -> Result<i64, String> {
        let mut left = self.parse_and(tokens, pos)?;
        while *pos < tokens.len() && tokens[*pos] == CondToken::PipePipe {
            *pos += 1;
            let right = self.parse_and(tokens, pos)?;
            left = if left != 0 || right != 0 { 1 } else { 0 };
        }
        Ok(left)
    }

    fn parse_and(&self, tokens: &[CondToken], pos: &mut usize) -> Result<i64, String> {
        let mut left = self.parse_bitor(tokens, pos)?;
        while *pos < tokens.len() && tokens[*pos] == CondToken::AmpAmp {
            *pos += 1;
            let right = self.parse_bitor(tokens, pos)?;
            left = if left != 0 && right != 0 { 1 } else { 0 };
        }
        Ok(left)
    }

    fn parse_bitor(&self, tokens: &[CondToken], pos: &mut usize) -> Result<i64, String> {
        let mut left = self.parse_bitxor(tokens, pos)?;
        while *pos < tokens.len() && tokens[*pos] == CondToken::Pipe {
            *pos += 1;
            let right = self.parse_bitxor(tokens, pos)?;
            left |= right;
        }
        Ok(left)
    }

    fn parse_bitxor(&self, tokens: &[CondToken], pos: &mut usize) -> Result<i64, String> {
        let mut left = self.parse_bitand(tokens, pos)?;
        while *pos < tokens.len() && tokens[*pos] == CondToken::Caret {
            *pos += 1;
            let right = self.parse_bitand(tokens, pos)?;
            left ^= right;
        }
        Ok(left)
    }

    fn parse_bitand(&self, tokens: &[CondToken], pos: &mut usize) -> Result<i64, String> {
        let mut left = self.parse_equality(tokens, pos)?;
        while *pos < tokens.len() && tokens[*pos] == CondToken::Amp {
            *pos += 1;
            let right = self.parse_equality(tokens, pos)?;
            left &= right;
        }
        Ok(left)
    }

    fn parse_equality(&self, tokens: &[CondToken], pos: &mut usize) -> Result<i64, String> {
        let mut left = self.parse_relational(tokens, pos)?;
        while *pos < tokens.len() {
            match tokens[*pos] {
                CondToken::EqEq => {
                    *pos += 1;
                    let right = self.parse_relational(tokens, pos)?;
                    left = if left == right { 1 } else { 0 };
                }
                CondToken::NotEq => {
                    *pos += 1;
                    let right = self.parse_relational(tokens, pos)?;
                    left = if left != right { 1 } else { 0 };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_relational(&self, tokens: &[CondToken], pos: &mut usize) -> Result<i64, String> {
        let mut left = self.parse_shift(tokens, pos)?;
        while *pos < tokens.len() {
            match tokens[*pos] {
                CondToken::Less => {
                    *pos += 1;
                    let right = self.parse_shift(tokens, pos)?;
                    left = if left < right { 1 } else { 0 };
                }
                CondToken::LessEq => {
                    *pos += 1;
                    let right = self.parse_shift(tokens, pos)?;
                    left = if left <= right { 1 } else { 0 };
                }
                CondToken::Greater => {
                    *pos += 1;
                    let right = self.parse_shift(tokens, pos)?;
                    left = if left > right { 1 } else { 0 };
                }
                CondToken::GreaterEq => {
                    *pos += 1;
                    let right = self.parse_shift(tokens, pos)?;
                    left = if left >= right { 1 } else { 0 };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_shift(&self, tokens: &[CondToken], pos: &mut usize) -> Result<i64, String> {
        let mut left = self.parse_additive(tokens, pos)?;
        while *pos < tokens.len() {
            match tokens[*pos] {
                CondToken::LShift => {
                    *pos += 1;
                    let right = self.parse_additive(tokens, pos)?;
                    left <<= right;
                }
                CondToken::RShift => {
                    *pos += 1;
                    let right = self.parse_additive(tokens, pos)?;
                    left >>= right;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_additive(&self, tokens: &[CondToken], pos: &mut usize) -> Result<i64, String> {
        let mut left = self.parse_multiplicative(tokens, pos)?;
        while *pos < tokens.len() {
            match tokens[*pos] {
                CondToken::Plus => {
                    *pos += 1;
                    let right = self.parse_multiplicative(tokens, pos)?;
                    left += right;
                }
                CondToken::Minus => {
                    *pos += 1;
                    let right = self.parse_multiplicative(tokens, pos)?;
                    left -= right;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_multiplicative(&self, tokens: &[CondToken], pos: &mut usize) -> Result<i64, String> {
        let mut left = self.parse_unary(tokens, pos)?;
        while *pos < tokens.len() {
            match tokens[*pos] {
                CondToken::Star => {
                    *pos += 1;
                    let right = self.parse_unary(tokens, pos)?;
                    left *= right;
                }
                CondToken::Slash => {
                    *pos += 1;
                    let right = self.parse_unary(tokens, pos)?;
                    if right == 0 {
                        return Err("division by zero in preprocessor expression".to_string());
                    }
                    left /= right;
                }
                CondToken::Percent => {
                    *pos += 1;
                    let right = self.parse_unary(tokens, pos)?;
                    if right == 0 {
                        return Err("modulo by zero in preprocessor expression".to_string());
                    }
                    left %= right;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_unary(&self, tokens: &[CondToken], pos: &mut usize) -> Result<i64, String> {
        if *pos >= tokens.len() {
            return Ok(0);
        }
        match tokens[*pos] {
            CondToken::Minus => {
                *pos += 1;
                let val = self.parse_unary(tokens, pos)?;
                Ok(-val)
            }
            CondToken::Plus => {
                *pos += 1;
                self.parse_unary(tokens, pos)
            }
            CondToken::Bang => {
                *pos += 1;
                let val = self.parse_unary(tokens, pos)?;
                Ok(if val == 0 { 1 } else { 0 })
            }
            CondToken::Tilde => {
                *pos += 1;
                let val = self.parse_unary(tokens, pos)?;
                Ok(!val)
            }
            _ => self.parse_primary(tokens, pos),
        }
    }

    fn parse_primary(&self, tokens: &[CondToken], pos: &mut usize) -> Result<i64, String> {
        if *pos >= tokens.len() {
            return Ok(0);
        }
        match &tokens[*pos] {
            CondToken::Num(n) => {
                let val = *n;
                *pos += 1;
                Ok(val)
            }
            CondToken::LParen => {
                *pos += 1;
                let val = self.parse_ternary(tokens, pos)?;
                if *pos < tokens.len() && tokens[*pos] == CondToken::RParen {
                    *pos += 1;
                }
                Ok(val)
            }
            _ => {
                // Unknown token, treat as 0
                *pos += 1;
                Ok(0)
            }
        }
    }
}

/// Token type for constant expression evaluation.
#[derive(Debug, Clone, PartialEq)]
enum CondToken {
    Num(i64),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LParen,
    RParen,
    Bang,
    Tilde,
    AmpAmp,
    PipePipe,
    Amp,
    Pipe,
    Caret,
    EqEq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    LShift,
    RShift,
    Question,
    Colon,
}

/// Convenience function: preprocess a source file.
pub fn preprocess(
    source: &str,
    filename: &str,
    include_paths: Vec<PathBuf>,
) -> Result<String, String> {
    let mut pp = Preprocessor::new(filename, include_paths);
    pp.preprocess(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_directives() {
        let source = "int main() { return 0; }";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert_eq!(result.trim(), "int main() { return 0; }");
    }

    #[test]
    fn test_object_like_define() {
        let source = "#define X 42\nint a = X;\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("int a = 42;"));
    }

    #[test]
    fn test_function_like_define() {
        let source = "#define ADD(a, b) ((a) + (b))\nint x = ADD(1, 2);\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("int x = ((1) + (2));"));
    }

    #[test]
    fn test_ifdef_defined() {
        let source = "#define FOO\n#ifdef FOO\nint x = 1;\n#endif\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("int x = 1;"));
    }

    #[test]
    fn test_ifdef_not_defined() {
        let source = "#ifdef FOO\nint x = 1;\n#endif\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(!result.contains("int x = 1;"));
    }

    #[test]
    fn test_ifndef() {
        let source = "#ifndef FOO\nint x = 1;\n#endif\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("int x = 1;"));
    }

    #[test]
    fn test_ifdef_else() {
        let source = "#ifdef FOO\nint x = 1;\n#else\nint x = 2;\n#endif\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(!result.contains("int x = 1;"));
        assert!(result.contains("int x = 2;"));
    }

    #[test]
    fn test_if_expression() {
        let source = "#define VER 2\n#if VER > 1\nint x = 1;\n#endif\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("int x = 1;"));
    }

    #[test]
    fn test_elif() {
        let source =
            "#define X 2\n#if X == 1\nint a;\n#elif X == 2\nint b;\n#else\nint c;\n#endif\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(!result.contains("int a;"));
        assert!(result.contains("int b;"));
        assert!(!result.contains("int c;"));
    }

    #[test]
    fn test_undef() {
        let source = "#define X 1\n#undef X\n#ifdef X\nint a;\n#endif\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(!result.contains("int a;"));
    }

    #[test]
    fn test_nested_ifdef() {
        let source = "#define A\n#define B\n#ifdef A\n#ifdef B\nint x;\n#endif\n#endif\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("int x;"));
    }

    #[test]
    fn test_error_directive() {
        let source = "#error this is bad";
        let result = preprocess(source, "test.c", vec![]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("this is bad"));
    }

    #[test]
    fn test_unterminated_conditional() {
        let source = "#ifdef FOO\n";
        let result = preprocess(source, "test.c", vec![]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unterminated"));
    }

    #[test]
    fn test_stringification() {
        let source = "#define STR(x) #x\nSTR(hello)\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("\"hello\""));
    }

    #[test]
    fn test_token_pasting() {
        let source = "#define PASTE(a, b) a##b\nPASTE(foo, bar)\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("foobar"));
    }

    #[test]
    fn test_predefined_file() {
        let source = "__FILE__\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("\"test.c\""));
    }

    #[test]
    fn test_predefined_line() {
        let source = "__LINE__\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("1"));
    }

    #[test]
    fn test_macro_no_expand_in_string() {
        let source = "#define X 42\n\"X is X\"\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("\"X is X\""));
    }

    #[test]
    fn test_if_defined_operator() {
        let source = "#define FOO\n#if defined(FOO)\nint x;\n#endif\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("int x;"));
    }

    #[test]
    fn test_if_not_defined_operator() {
        let source = "#if defined(BAR)\nint x;\n#endif\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(!result.contains("int x;"));
    }

    #[test]
    fn test_line_continuation() {
        let source = "#define LONG_MACRO \\\n    42\nint x = LONG_MACRO;\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("int x = 42;"));
    }

    #[test]
    fn test_include_quoted() {
        // Create a temp dir with a header
        let dir = std::env::temp_dir().join("rustcc_test_include");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("test.h"), "int included_var;").unwrap();

        let source = format!("#include \"{}/test.h\"\n", dir.display());
        let result = preprocess(&source, "test.c", vec![]).unwrap();
        assert!(result.contains("int included_var;"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_recursive_macro_prevention() {
        let source = "#define X X\nX\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("X"));
    }

    #[test]
    fn test_null_directive() {
        let source = "#\nint x;\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("int x;"));
    }

    #[test]
    fn test_pragma_ignored() {
        let source = "#pragma once\nint x;\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("int x;"));
    }

    #[test]
    fn test_nested_function_macro() {
        let source = "#define A(x) x * 2\n#define B(x) A(x) + 1\nB(3)\n";
        let result = preprocess(source, "test.c", vec![]).unwrap();
        assert!(result.contains("3 * 2 + 1"));
    }
}
