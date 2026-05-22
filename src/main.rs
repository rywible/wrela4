fn main() {
    let code = wrela::command::run(std::env::args());
    std::process::exit(code);
}
