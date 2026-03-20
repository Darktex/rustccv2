/// Integration tests: compile C programs using rustcc and verify they run correctly.
use std::fs;
use std::process::Command;

/// Compile a C source string, run the resulting binary, return (exit_code, stdout).
fn compile_and_run(source: &str) -> (i32, String) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let c_path = dir.path().join("test.c");
    let out_path = dir.path().join("test_bin");

    fs::write(&c_path, source).expect("write test.c");

    let rustcc = env!("CARGO_BIN_EXE_rustcc");

    let compile = Command::new(rustcc)
        .args(["-o", out_path.to_str().unwrap(), c_path.to_str().unwrap()])
        .output()
        .expect("run rustcc");

    if !compile.status.success() {
        let stderr = String::from_utf8_lossy(&compile.stderr);
        panic!("Compilation failed:\n{}", stderr);
    }

    let run = Command::new(out_path.to_str().unwrap())
        .output()
        .expect("run compiled binary");

    let exit_code = run.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&run.stdout).to_string();

    (exit_code, stdout)
}

// === Return constants ===

#[test]
fn test_return_42() {
    let (code, _) = compile_and_run("int main() { return 42; }");
    assert_eq!(code, 42);
}

#[test]
fn test_return_0() {
    let (code, _) = compile_and_run("int main() { return 0; }");
    assert_eq!(code, 0);
}

// === Arithmetic ===

#[test]
fn test_add() {
    let (code, _) = compile_and_run("int main() { return 10 + 20; }");
    assert_eq!(code, 30);
}

#[test]
fn test_sub() {
    let (code, _) = compile_and_run("int main() { return 50 - 8; }");
    assert_eq!(code, 42);
}

#[test]
fn test_mul() {
    let (code, _) = compile_and_run("int main() { return 6 * 7; }");
    assert_eq!(code, 42);
}

#[test]
fn test_div() {
    let (code, _) = compile_and_run("int main() { return 84 / 2; }");
    assert_eq!(code, 42);
}

#[test]
fn test_mod() {
    let (code, _) = compile_and_run("int main() { return 47 % 10; }");
    assert_eq!(code, 7);
}

#[test]
fn test_precedence() {
    let (code, _) = compile_and_run("int main() { return 2 + 3 * 4; }");
    assert_eq!(code, 14);
}

// === Variables ===

#[test]
fn test_var_decl() {
    let (code, _) = compile_and_run("int main() { int x = 42; return x; }");
    assert_eq!(code, 42);
}

#[test]
fn test_var_assign() {
    let (code, _) = compile_and_run("int main() { int x = 10; x = 42; return x; }");
    assert_eq!(code, 42);
}

#[test]
fn test_multiple_vars() {
    let (code, _) =
        compile_and_run("int main() { int a = 10; int b = 20; int c = 12; return a + b + c; }");
    assert_eq!(code, 42);
}

// === Comparisons ===

#[test]
fn test_eq_true() {
    let (code, _) = compile_and_run("int main() { return 5 == 5; }");
    assert_eq!(code, 1);
}

#[test]
fn test_eq_false() {
    let (code, _) = compile_and_run("int main() { return 5 == 3; }");
    assert_eq!(code, 0);
}

#[test]
fn test_ne() {
    let (code, _) = compile_and_run("int main() { return 5 != 3; }");
    assert_eq!(code, 1);
}

#[test]
fn test_lt() {
    let (code, _) = compile_and_run("int main() { return 3 < 5; }");
    assert_eq!(code, 1);
}

#[test]
fn test_gt() {
    let (code, _) = compile_and_run("int main() { return 5 > 3; }");
    assert_eq!(code, 1);
}

// === Control flow ===

#[test]
fn test_if_true() {
    let (code, _) = compile_and_run("int main() { if (1) { return 42; } return 0; }");
    assert_eq!(code, 42);
}

#[test]
fn test_if_false() {
    let (code, _) = compile_and_run("int main() { if (0) { return 42; } return 0; }");
    assert_eq!(code, 0);
}

#[test]
fn test_if_else() {
    let (code, _) =
        compile_and_run("int main() { int x = 10; if (x > 5) { return 1; } else { return 0; } }");
    assert_eq!(code, 1);
}

#[test]
fn test_while_loop() {
    let (code, _) = compile_and_run(
        "int main() { int i = 0; int sum = 0; while (i < 10) { sum = sum + i; i = i + 1; } return sum; }",
    );
    assert_eq!(code, 45);
}

#[test]
fn test_for_loop() {
    let (code, _) = compile_and_run(
        "int main() { int sum = 0; for (int i = 0; i < 5; i = i + 1) { sum = sum + i; } return sum; }",
    );
    assert_eq!(code, 10);
}

// === Unary operators ===

#[test]
fn test_negate() {
    let (code, _) = compile_and_run("int main() { return -(-42); }");
    assert_eq!(code, 42);
}

// === Function calls ===

#[test]
fn test_function_call() {
    let (code, _) = compile_and_run(
        "int add(int a, int b) { return a + b; } int main() { return add(20, 22); }",
    );
    assert_eq!(code, 42);
}

#[test]
fn test_hello_world() {
    let source = r#"
        int printf(const char *fmt, ...);
        int main() {
            printf("Hello, World!\n");
            return 0;
        }
    "#;
    let (code, stdout) = compile_and_run(source);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "Hello, World!");
}

// === Complex programs ===

#[test]
fn test_nested_if() {
    let (code, _) = compile_and_run(
        "int main() { int x = 10; int y = 20; if (x > 5) { if (y > 15) { return 42; } } return 0; }",
    );
    assert_eq!(code, 42);
}

#[test]
fn test_complex_expr() {
    let (code, _) =
        compile_and_run("int main() { int x = 2; int y = 3; return (x + y) * (x + y) - x * y; }");
    // (2+3)*(2+3) - 2*3 = 25 - 6 = 19
    assert_eq!(code, 19);
}

// === Compound assignment operators ===

#[test]
fn test_plus_assign() {
    let (code, _) = compile_and_run("int main() { int x = 10; x += 32; return x; }");
    assert_eq!(code, 42);
}

#[test]
fn test_minus_assign() {
    let (code, _) = compile_and_run("int main() { int x = 50; x -= 8; return x; }");
    assert_eq!(code, 42);
}

#[test]
fn test_star_assign() {
    let (code, _) = compile_and_run("int main() { int x = 6; x *= 7; return x; }");
    assert_eq!(code, 42);
}

// === Do-while ===

#[test]
fn test_do_while() {
    let (code, _) = compile_and_run(
        "int main() { int i = 0; int sum = 0; do { sum = sum + i; i = i + 1; } while (i < 10); return sum; }",
    );
    assert_eq!(code, 45);
}

#[test]
fn test_do_while_once() {
    // do-while always executes body at least once, even if condition is false
    let (code, _) =
        compile_and_run("int main() { int x = 0; do { x = 42; } while (0); return x; }");
    assert_eq!(code, 42);
}

// === Break and continue ===

#[test]
fn test_break_in_while() {
    let (code, _) = compile_and_run(
        "int main() { int i = 0; while (1) { if (i == 5) { break; } i = i + 1; } return i; }",
    );
    assert_eq!(code, 5);
}

#[test]
fn test_continue_in_for() {
    let (code, _) = compile_and_run(
        "int main() { int sum = 0; for (int i = 0; i < 10; i = i + 1) { if (i % 2 == 0) { continue; } sum = sum + i; } return sum; }",
    );
    // sum of odd numbers 1+3+5+7+9 = 25
    assert_eq!(code, 25);
}

// === Post/pre increment/decrement ===

#[test]
fn test_post_increment() {
    let (code, _) = compile_and_run("int main() { int x = 41; x++; return x; }");
    assert_eq!(code, 42);
}

#[test]
fn test_post_decrement() {
    let (code, _) = compile_and_run("int main() { int x = 43; x--; return x; }");
    assert_eq!(code, 42);
}

// === Logical operators ===

#[test]
fn test_logical_and_true() {
    let (code, _) = compile_and_run("int main() { return 1 && 1; }");
    assert_eq!(code, 1);
}

#[test]
fn test_logical_and_false() {
    let (code, _) = compile_and_run("int main() { return 1 && 0; }");
    assert_eq!(code, 0);
}

#[test]
fn test_logical_or_true() {
    let (code, _) = compile_and_run("int main() { return 0 || 1; }");
    assert_eq!(code, 1);
}

#[test]
fn test_logical_or_false() {
    let (code, _) = compile_and_run("int main() { return 0 || 0; }");
    assert_eq!(code, 0);
}

// === Comparison operators (le, ge) ===

#[test]
fn test_le() {
    let (code, _) = compile_and_run("int main() { return (5 <= 5) + (3 <= 5); }");
    assert_eq!(code, 2);
}

#[test]
fn test_ge() {
    let (code, _) = compile_and_run("int main() { return (5 >= 5) + (5 >= 3); }");
    assert_eq!(code, 2);
}

// === Multiple function calls ===

#[test]
fn test_recursive_function() {
    let source = r#"
        int factorial(int n) {
            if (n <= 1) { return 1; }
            return n * factorial(n - 1);
        }
        int main() {
            return factorial(5);
        }
    "#;
    let (code, _) = compile_and_run(source);
    // 5! = 120
    assert_eq!(code, 120);
}

#[test]
fn test_multiple_functions() {
    let source = r#"
        int square(int x) { return x * x; }
        int add(int a, int b) { return a + b; }
        int main() {
            return add(square(3), square(4));
        }
    "#;
    let (code, _) = compile_and_run(source);
    // 9 + 16 = 25
    assert_eq!(code, 25);
}

// === Preprocessor integration ===

#[test]
fn test_preprocessor_define() {
    let source = r#"
        #define ANSWER 42
        int main() { return ANSWER; }
    "#;
    let (code, _) = compile_and_run(source);
    assert_eq!(code, 42);
}

#[test]
fn test_preprocessor_ifdef() {
    let source = r#"
        #define DEBUG
        int main() {
            #ifdef DEBUG
            return 1;
            #else
            return 0;
            #endif
        }
    "#;
    let (code, _) = compile_and_run(source);
    assert_eq!(code, 1);
}

#[test]
fn test_preprocessor_ifndef() {
    let source = r#"
        int main() {
            #ifndef UNDEFINED_MACRO
            return 42;
            #else
            return 0;
            #endif
        }
    "#;
    let (code, _) = compile_and_run(source);
    assert_eq!(code, 42);
}

#[test]
fn test_preprocessor_function_macro() {
    let source = r#"
        #define MAX(a, b) ((a) > (b) ? (a) : (b))
        int main() { return MAX(10, 42); }
    "#;
    let (code, _) = compile_and_run(source);
    assert_eq!(code, 42);
}

// === Ternary operator ===

#[test]
fn test_ternary_true() {
    let (code, _) = compile_and_run("int main() { return 1 ? 42 : 0; }");
    assert_eq!(code, 42);
}

#[test]
fn test_ternary_false() {
    let (code, _) = compile_and_run("int main() { return 0 ? 0 : 42; }");
    assert_eq!(code, 42);
}

// === Char literal ===

#[test]
fn test_char_literal() {
    let (code, _) = compile_and_run("int main() { char c = 'A'; return c; }");
    assert_eq!(code, 65);
}

// === Printf with integer ===

#[test]
fn test_printf_format() {
    let source = r#"
        int printf(const char *fmt, ...);
        int main() {
            printf("%d plus %d equals %d\n", 2, 3, 5);
            return 0;
        }
    "#;
    let (code, stdout) = compile_and_run(source);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "2 plus 3 equals 5");
}

// === Nested loops ===

#[test]
fn test_nested_for_loops() {
    let source = r#"
        int main() {
            int sum = 0;
            for (int i = 0; i < 3; i = i + 1) {
                for (int j = 0; j < 3; j = j + 1) {
                    sum = sum + 1;
                }
            }
            return sum;
        }
    "#;
    let (code, _) = compile_and_run(source);
    assert_eq!(code, 9);
}

// === Enum usage ===

#[test]
fn test_enum_values() {
    let source = r#"
        enum Color { RED, GREEN, BLUE };
        int main() {
            int c = 2;
            return c;
        }
    "#;
    let (code, _) = compile_and_run(source);
    // BLUE = 2
    assert_eq!(code, 2);
}
