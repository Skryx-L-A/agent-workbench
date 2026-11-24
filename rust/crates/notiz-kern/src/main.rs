//! The program `notiz-kern`: reads its arguments, calls the library and exits with the result code.

use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let code = notiz_kern::run(&args, &mut io::stdout().lock(), &mut io::stderr().lock());
    ExitCode::from(code)
}
