use std::ffi::OsStr;
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

pub fn run_command_with_env<K>(
    cmd: &str,
    work_dir: impl AsRef<Path>,
    env: &[(K, &OsStr)],
) -> io::Result<Output>
where
    K: AsRef<OsStr>,
{
    let arg_iter: Vec<_> = cmd.split(' ').collect();
    let mut cmd = Command::new(arg_iter[0]);
    cmd.args(&arg_iter[1..]).current_dir(work_dir);
    for (k, v) in env {
        cmd.env(k.as_ref(), v);
    }

    cmd.output()
}

pub fn is_cmd_success(cmd_result: &io::Result<Output>) -> bool {
    match &cmd_result {
        Ok(output) if output.status.success() => true,
        _ => false,
    }
}

pub fn is_cmd_empty(cmd_result: &io::Result<Output>) -> bool {
    match &cmd_result {
        Ok(output) if output.stdout.len() > 0 => false,
        _ => true,
    }
}
