use std::path::Path;

use std::process::Command;

fn main() {
    let path_str = option_env!("CARGO_MANIFEST_DIR").expect("failed to detect the build path");
    let crux_path = Path::new(&path_str).join("..");

    let mut child = Command::new("./install-debug")
        .current_dir(crux_path)
        .spawn()
        .expect("failed to execute install command");

    child.wait().expect("wait failed");
}
