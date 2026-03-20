mod codegen;
mod lexer;
mod parser;

use std::env;
use std::fs;
use std::process::{self, Command};

fn print_usage() {
    eprintln!("Usage: rustcc [-o <output>] <input.c>");
    eprintln!("  -o <output>  Output executable path (default: a.out)");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut input_file = None;
    let mut output_file = String::from("a.out");

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: -o requires an argument");
                    print_usage();
                    process::exit(1);
                }
                output_file = args[i].clone();
            }
            arg if arg.starts_with('-') => {
                eprintln!("Error: unknown option '{arg}'");
                print_usage();
                process::exit(1);
            }
            _ => {
                if input_file.is_some() {
                    eprintln!("Error: multiple input files not supported");
                    print_usage();
                    process::exit(1);
                }
                input_file = Some(args[i].clone());
            }
        }
        i += 1;
    }

    let input_file = match input_file {
        Some(f) => f,
        None => {
            eprintln!("Error: no input file");
            print_usage();
            process::exit(1);
        }
    };

    let source = match fs::read_to_string(&input_file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: cannot read '{input_file}': {e}");
            process::exit(1);
        }
    };

    // Lex
    let tokens = match lexer::lex(&source) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Lexer error: {e}");
            process::exit(1);
        }
    };

    // Parse
    let program = match parser::parse(&tokens) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {e}");
            process::exit(1);
        }
    };

    // Codegen
    let assembly = codegen::generate(&program);

    // Write assembly to temp file
    let asm_path = format!("{output_file}.s");
    let obj_path = format!("{output_file}.o");

    if let Err(e) = fs::write(&asm_path, &assembly) {
        eprintln!("Error: cannot write assembly: {e}");
        process::exit(1);
    }

    // Assemble
    let status = Command::new("as")
        .args(["-o", &obj_path, &asm_path])
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            eprintln!("Error: assembler failed with {s}");
            cleanup(&[&asm_path, &obj_path]);
            process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: cannot run assembler: {e}");
            cleanup(&[&asm_path]);
            process::exit(1);
        }
    }

    // Link
    let status = Command::new("cc")
        .args(["-o", &output_file, &obj_path])
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            eprintln!("Error: linker failed with {s}");
            cleanup(&[&asm_path, &obj_path]);
            process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: cannot run linker: {e}");
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
