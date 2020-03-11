use std::io;
use std::path::Path;
use std::process::{Command, Output};

pub fn run_command(cmd: &str, work_dir: impl AsRef<Path>) -> io::Result<Output> {
    let arg_iter: Vec<_> = cmd.split(' ').collect();
    Command::new(arg_iter[0])
        .args(&arg_iter[1..])
        .current_dir(work_dir)
        .output()
}

pub fn is_cmd_success(cmd_result: &io::Result<Output>) -> bool {
    match &cmd_result {
        Ok(output) if output.status.success() => true,
        _ => false,
    }
}
