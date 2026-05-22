pub fn run<I>(args: I) -> i32
where
    I: IntoIterator<Item = String>,
{
    let mut out = std::io::stdout();
    let mut err = std::io::stderr();
    run_with_io(args, &mut out, &mut err)
}

pub fn run_with_io<I, W, E>(args: I, out: &mut W, err: &mut E) -> i32
where
    I: IntoIterator<Item = String>,
    W: std::io::Write,
    E: std::io::Write,
{
    let collected: Vec<String> = args.into_iter().collect();

    match collected.get(1).map(String::as_str) {
        Some("help") | None => {
            let _ = writeln!(out, "wrela commands: help, version");
            0
        }
        Some("version") => {
            let _ = writeln!(out, "wrela 0.1.0");
            0
        }
        Some(other) => {
            let _ = writeln!(err, "unknown command: {other}");
            2
        }
    }
}
