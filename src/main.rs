use std::{env, path::Path};
use vorgrs::{Error, ErrorKind, Repo, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let wrong_arg_error = Error {
        msg: String::from(
            "Usage:
    vorgrs import [vorg repo path] [file or folder to import]
    vorgrs check [vorg repo path]",
        ),
        kind: ErrorKind::WrongArguments,
    };

    // TODO: rework arg parsing logic
    if args.len() < 2 {
        return Err(wrong_arg_error);
    }

    if args[1] == "import" {
        if args.len() < 4 {
            return Err(wrong_arg_error);
        }

        let mut repo = Repo::new(Path::new(&args[2])).await.unwrap();

        let path = Path::new(&args[3]);
        repo.import(path).await.unwrap();
    } else if args[1] == "check" {
        if args.len() < 3 {
            return Err(wrong_arg_error);
        }

        let mut repo = Repo::new(Path::new(&args[2])).await.unwrap();

        let result = repo
            .check_data_integrity()
            .await
            .expect("Error checking vorg repo.");
        eprint!("{result}");
    } else {
        return Err(wrong_arg_error);
    }

    Ok(())
}
