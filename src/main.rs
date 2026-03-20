//! rustcc — a C compiler written in Rust.
//!
//! Pipeline: lex → parse → IR → codegen → assemble → link

mod ast;
mod codegen;
mod ir;
mod ir_builder;
mod lexer;
mod parser;

use std::env;
use std::fs;
use std::path::Path;
use std::process::{self, Command};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: rustcc [-o output] <input.c>");
        process::exit(1);
    }

    let mut input_file = None;
    let mut output_file = String::from("a.out");
    let mut emit_asm = false;
    let mut emit_ir = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: -o requires an argument");
                    process::exit(1);
                }
                output_file = args[i].clone();
            }
            "-S" => {
                emit_asm = true;
            }
            "--emit-ir" => {
                emit_ir = true;
            }
            arg if !arg.starts_with('-') => {
                input_file = Some(arg.to_string());
            }
            arg => {
                eprintln!("Unknown option: {arg}");
                process::exit(1);
            }
        }
        i += 1;
    }

    let input_file = match input_file {
        Some(f) => f,
        None => {
            eprintln!("Error: no input file specified");
            process::exit(1);
        }
    };

    // Read source
    let source = match fs::read_to_string(&input_file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {input_file}: {e}");
            process::exit(1);
        }
    };

    // Lex
    let mut lxr = lexer::Lexer::new(&source);
    let tokens = match lxr.tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Lexer error: {e}");
            process::exit(1);
        }
    };

    // Parse
    let mut psr = parser::Parser::new(tokens);
    let program = match psr.parse_program() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parser error: {e}");
            process::exit(1);
        }
    };

    // Lower to IR
    let builder = ir_builder::IrBuilder::new();
    let ir_program = match builder.lower(&program) {
        Ok(ir) => ir,
        Err(e) => {
            eprintln!("IR lowering error: {e}");
            process::exit(1);
        }
    };

    if emit_ir {
        print!("{ir_program}");
        return;
    }

    // Code generation
    let gen = codegen::CodeGen::new();
    let asm = gen.generate(&ir_program);

    if emit_asm {
        if output_file == "a.out" {
            // Default asm output name: input.s
            let asm_file = Path::new(&input_file).with_extension("s");
            fs::write(&asm_file, &asm).unwrap_or_else(|e| {
                eprintln!("Error writing assembly: {e}");
                process::exit(1);
            });
        } else {
            fs::write(&output_file, &asm).unwrap_or_else(|e| {
                eprintln!("Error writing assembly: {e}");
                process::exit(1);
            });
        }
        return;
    }

    // Write assembly to temp file, assemble and link
    let asm_path = format!("/tmp/rustcc_{}.s", process::id());
    let obj_path = format!("/tmp/rustcc_{}.o", process::id());

    fs::write(&asm_path, &asm).unwrap_or_else(|e| {
        eprintln!("Error writing temp assembly: {e}");
        process::exit(1);
    });

    // Assemble
    let as_status = Command::new("as")
        .args([&asm_path, "-o", &obj_path])
        .status();

    match as_status {
        Ok(status) if status.success() => {}
        Ok(status) => {
            eprintln!("Assembler failed with exit code: {}", status);
            cleanup(&[&asm_path, &obj_path]);
            process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to run assembler: {e}");
            cleanup(&[&asm_path]);
            process::exit(1);
        }
    }

    // Link using cc (handles libc linking automatically)
    let cc_status = Command::new("cc")
        .args([&obj_path, "-o", &output_file])
        .status();

    match cc_status {
        Ok(status) if status.success() => {}
        Ok(status) => {
            eprintln!("Linker failed with exit code: {}", status);
            cleanup(&[&asm_path, &obj_path]);
            process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to run linker: {e}");
            cleanup(&[&asm_path, &obj_path]);
            process::exit(1);
        }
    }

    // Cleanup temp files
    cleanup(&[&asm_path, &obj_path]);
}

fn cleanup(files: &[&str]) {
    for f in files {
        let _ = fs::remove_file(f);
    }
}
