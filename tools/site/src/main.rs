use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [root, out] = arguments.as_slice() else {
        eprintln!("usage: post-site REPOSITORY OUTPUT");
        return ExitCode::FAILURE;
    };
    match post_site::build(Path::new(root), Path::new(out)) {
        Ok(()) => {
            println!("The website is in the directory {out}.");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("post-site: {error}");
            ExitCode::FAILURE
        }
    }
}
