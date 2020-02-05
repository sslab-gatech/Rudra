use std::path::Path;
use std::process::Command;

fn main() {
    if let Some(path_str) = option_env!("CARGO_MANIFEST_DIR") {
        let mut child = Command::new(Path::new(&path_str).join("install-debug"))
            .spawn()
            .expect(&format!(
                "failed to invoke install command at: {}",
                &path_str
            ));
        child.wait().expect("failed to wait for child process");
    }
}
