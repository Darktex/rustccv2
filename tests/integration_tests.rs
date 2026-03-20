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
