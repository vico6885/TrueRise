//! truerise binary: a three-line IO shell over the truerise_cli library
//! (all behaviour, including the repl, lives in the library and is testable).

fn main() {
    let code = truerise_cli::cli_main(std::env::args().collect());
    std::process::exit(code);
}
