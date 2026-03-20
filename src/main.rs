mod codegen;
mod ir;
mod lexer;
mod parser;
mod preprocessor;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{self, Command};

fn print_usage() {
    eprintln!("Usage: rustcc [-o output] [-I path] <input.c>");
    process::exit(1);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
    }

    let mut input_file = None;
    let mut output_file = String::from("a.out");
    let mut include_paths: Vec<PathBuf> = Vec::new();
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
            "-I" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: -I requires an argument");
                    process::exit(1);
                }
                include_paths.push(PathBuf::from(&args[i]));
            }
            arg if arg.starts_with("-I") => {
                // Support -Ipath (no space)
                include_paths.push(PathBuf::from(&arg[2..]));
            }
            arg if arg.starts_with('-') => {
                eprintln!("Error: unknown option '{}'", arg);
                process::exit(1);
            }
            _ => {
                input_file = Some(args[i].clone());
            }
        }
        i += 1;
    }

    let input_file = match input_file {
        Some(f) => f,
        None => {
            eprintln!("Error: no input file");
            process::exit(1);
        }
    };

    let source = match fs::read_to_string(&input_file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: cannot read '{}': {}", input_file, e);
            process::exit(1);
        }
    };

    // Preprocess
    let source = match preprocessor::preprocess(&source, &input_file, include_paths) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Preprocessor error: {}", e);
            process::exit(1);
        }
    };

    // Lex
    let tokens = match lexer::lex(&source) {
        Ok(tokens) => tokens,
        Err(e) => {
            eprintln!("Lexer error: {}", e);
            process::exit(1);
        }
    };

    // Parse
    let program = match parser::parse(tokens) {
        Ok(program) => program,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            process::exit(1);
        }
    };

    // Lower to IR
    let ir_module = ir::lower(&program);

    // Generate x86-64 assembly
    let asm = codegen::generate(&ir_module);

    // Write assembly to temp file
    let asm_file = format!("{}.s", output_file);
    if let Err(e) = fs::write(&asm_file, &asm) {
        eprintln!("Error: cannot write '{}': {}", asm_file, e);
        process::exit(1);
    }

    // Assemble and link
    let cc = env::var("CC").unwrap_or_else(|_| "cc".to_string());
    let status = Command::new(&cc)
        .args([&asm_file, "-o", &output_file, "-no-pie"])
        .status();

    // Clean up assembly file
    let _ = fs::remove_file(&asm_file);

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            eprintln!(
                "Error: assembler/linker failed with exit code {}",
                s.code().unwrap_or(-1)
            );
            process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: cannot run '{}': {}", cc, e);
            process::exit(1);
        }
    }
}
