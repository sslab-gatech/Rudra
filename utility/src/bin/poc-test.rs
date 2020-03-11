use std::error::Error as StdError;
use std::path::PathBuf;
use std::result::Result as StdResult;

use ansi_term::Color::{Green, Red};

use utility::utils::{is_cmd_success, run_command};

fn main() -> StdResult<(), Box<dyn StdError>> {
    let manifest_path: PathBuf = option_env!("CARGO_MANIFEST_DIR")
        .expect("Failed to get workspace information")
        .into();
    let poc_dir = manifest_path.join("../samples/poc");

    let mut test_names: Vec<_> = poc_dir.read_dir()?.map(|entry| entry.unwrap()).collect();
    test_names.sort_by_key(|entry| entry.file_name());

    test_names.iter().for_each(|dir_entry| {
        print!("Testing `{}`... ", dir_entry.file_name().to_string_lossy());

        let test_dir = &dir_entry.path();
        run_command("cargo clean", test_dir).expect("clean shouldn't fail");
        let result = run_command("cargo check", test_dir);
        if is_cmd_success(&result) {
            print!("cargo check {} ", Green.bold().paint("Passed"));

            run_command("cargo clean", test_dir).expect("clean shouldn't fail");
            let result = run_command("cargo miri", test_dir);
            if is_cmd_success(&result) {
                println!("Miri {}", Red.bold().paint("Passed"));
            } else {
                println!("Miri {}", Green.bold().paint("Failed"));
            }

            // TODO: add `cargo crux` test here once it becomes mature enough to handle libraries
            ()
        } else {
            println!("cargo check {}", Red.bold().paint("Failed"));
        }
    });

    Ok(())
}
