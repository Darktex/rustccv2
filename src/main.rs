fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: rustcc [-o <output>] <input.c>");
        std::process::exit(1);
    }

    let mut input_file = None;
    let mut output_file = String::from("a.out");
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: -o requires an argument");
                    std::process::exit(1);
                }
                output_file = args[i].clone();
            }
            arg => {
                input_file = Some(arg.to_string());
            }
        }
        i += 1;
    }

    let _input = match input_file {
        Some(f) => f,
        None => {
            eprintln!("Error: no input file specified");
            std::process::exit(1);
        }
    };

    // TODO: lex -> parse -> IR -> codegen
    // For now just acknowledge the pipeline
    let _ = output_file;
    eprintln!("rustcc: compilation pipeline not yet complete");
    std::process::exit(1);
}
