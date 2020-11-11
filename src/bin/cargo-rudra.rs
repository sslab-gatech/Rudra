#![feature(backtrace)]

///! This implementation is based on `cargo-miri`
///! https://github.com/rust-lang/miri/blob/master/src/bin/cargo-miri.rs
#[macro_use]
extern crate log as log_crate;

use std::env;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use rustc_version::VersionMeta;

use wait_timeout::ChildExt;

use rudra::log::{self, Verbosity};
use rudra::{progress_error, progress_info};

const CARGO_RUDRA_HELP: &str = r#"Tests crates with Rudra
Usage:
    cargo rudra [<cargo options>] [--] [<rustc/rudra options>...]

Common options:
    -h, --help               Print this message

Other [options] are the same as `cargo check`. Everything after the first "--" is
passed verbatim to Rudra.
"#;

fn show_help() {
    println!("{}", CARGO_RUDRA_HELP);
}

fn show_error(msg: impl AsRef<str>) -> ! {
    progress_error!("{}", msg.as_ref());
    std::process::exit(1)
}

// Determines whether a `--flag` is present.
fn has_arg_flag(name: &str) -> bool {
    // Stop searching at `--`.
    let mut args = std::env::args().take_while(|val| val != "--");
    args.any(|val| val == name)
}

/// Gets the value of a `--flag`.
fn get_arg_flag_value(name: &str) -> Option<String> {
    // Stop searching at `--`.
    let mut args = std::env::args().take_while(|val| val != "--");
    loop {
        let arg = match args.next() {
            Some(arg) => arg,
            None => return None,
        };
        if !arg.starts_with(name) {
            continue;
        }
        // Strip leading `name`.
        let suffix = &arg[name.len()..];
        if suffix.is_empty() {
            // This argument is exactly `name`; the next one is the value.
            return args.next();
        } else if suffix.starts_with('=') {
            // This argument is `name=value`; get the value.
            // Strip leading `=`.
            return Some(suffix[1..].to_owned());
        }
    }
}

fn any_arg_flag<F>(name: &str, mut check: F) -> bool
where
    F: FnMut(&str) -> bool,
{
    // Stop searching at `--`.
    let mut args = std::env::args().take_while(|val| val != "--");
    loop {
        let arg = match args.next() {
            Some(arg) => arg,
            None => return false,
        };
        if !arg.starts_with(name) {
            continue;
        }

        // Strip leading `name`.
        let suffix = &arg[name.len()..];
        let value = if suffix.is_empty() {
            // This argument is exactly `name`; the next one is the value.
            match args.next() {
                Some(arg) => arg,
                None => return false,
            }
        } else if suffix.starts_with('=') {
            // This argument is `name=value`; get the value.
            // Strip leading `=`.
            suffix[1..].to_owned()
        } else {
            return false;
        };

        if check(&value) {
            return true;
        }
    }
}

/// Finds the first argument ends with `.rs`.
fn get_first_arg_with_rs_suffix() -> Option<String> {
    // Stop searching at `--`.
    let mut args = std::env::args().take_while(|val| val != "--");
    args.find(|arg| arg.ends_with(".rs"))
}

fn version_info() -> VersionMeta {
    VersionMeta::for_command(Command::new(find_rudra()))
        .expect("failed to determine underlying rustc version of Rudra")
}

fn cargo_package() -> cargo_metadata::Package {
    // We need to get the manifest, and then the metadata, to enumerate targets.
    let manifest_path =
        get_arg_flag_value("--manifest-path").map(|m| Path::new(&m).canonicalize().unwrap());

    let mut cmd = cargo_metadata::MetadataCommand::new();
    if let Some(manifest_path) = &manifest_path {
        cmd.manifest_path(manifest_path);
    }
    let mut metadata = if let Ok(metadata) = cmd.exec() {
        metadata
    } else {
        show_error("Could not obtain Cargo metadata");
    };

    let current_dir = std::env::current_dir();

    let package_index = metadata
        .packages
        .iter()
        .position(|package| {
            let package_manifest_path = Path::new(&package.manifest_path);
            if let Some(manifest_path) = &manifest_path {
                package_manifest_path == manifest_path
            } else {
                let current_dir = current_dir
                    .as_ref()
                    .expect("could not read current directory");
                let package_manifest_directory = package_manifest_path
                    .parent()
                    .expect("could not find parent directory of package manifest");
                package_manifest_directory == current_dir
            }
        })
        .unwrap_or_else(|| {
            show_error("This seems to be a workspace, which is not supported by cargo-rudra");
        });

    metadata.packages.remove(package_index)
}

/// Returns the path to the `rudra` binary
fn find_rudra() -> PathBuf {
    let mut path = std::env::current_exe().expect("current executable path invalid");
    path.set_file_name("rudra");
    path
}

/// Make sure that the `rudra` and `rustc` binary are from the same sysroot.
/// This can be violated e.g. when rudra is locally built and installed with a different
/// toolchain than what is used when `cargo rudra` is run.
fn test_sysroot_consistency() {
    fn get_sysroot(mut cmd: Command) -> PathBuf {
        let out = cmd
            .arg("--print")
            .arg("sysroot")
            .output()
            .expect("Failed to run rustc to get sysroot info");
        let stdout = String::from_utf8(out.stdout).expect("stdout is not valid UTF-8");
        let stderr = String::from_utf8(out.stderr).expect("stderr is not valid UTF-8");
        let stdout = stdout.trim();
        assert!(
            out.status.success(),
            "Bad status code when getting sysroot info.\nstdout:\n{}\nstderr:\n{}",
            stdout,
            stderr
        );
        PathBuf::from(stdout)
            .canonicalize()
            .unwrap_or_else(|_| panic!("Failed to canonicalize sysroot: {}", stdout))
    }

    let rustc_sysroot = get_sysroot(Command::new("rustc"));
    let rudra_sysroot = get_sysroot(Command::new(find_rudra()));

    if rustc_sysroot != rudra_sysroot {
        show_error(format!(
            "rudra was built for a different sysroot than the rustc in your current toolchain.\n\
             Make sure you use the same toolchain to run rudra that you used to build it!\n\
             rustc sysroot: `{}`\n\
             rudra sysroot: `{}`",
            rustc_sysroot.display(),
            rudra_sysroot.display()
        ));
    }
}

fn clean_package(package_name: &str) {
    let mut cmd = Command::new("cargo");
    cmd.arg("clean");

    cmd.arg("-p");
    cmd.arg(package_name);

    cmd.arg("--target");
    cmd.arg(version_info().host);

    let exit_status = cmd
        .spawn()
        .expect("could not run cargo clean")
        .wait()
        .expect("failed to wait for cargo?");

    if !exit_status.success() {
        show_error(format!("cargo clean failed"));
    }
}

fn main() {
    // Check for version and help flags even when invoked as `cargo-rudra`.
    if std::env::args().any(|a| a == "--help" || a == "-h") {
        show_help();
        return;
    }

    log::setup_logging(Verbosity::Normal).expect("Rudra failed to initialize");

    if let Some("rudra") = std::env::args().nth(1).as_ref().map(AsRef::as_ref) {
        progress_info!("Running cargo rudra");
        // This arm is for when `cargo rudra` is called. We call `cargo rustc` for each applicable target,
        // but with the `RUSTC` env var set to the `cargo-rudra` binary so that we come back in the other branch,
        // and dispatch the invocations to `rustc` and `rudra`, respectively.
        in_cargo_rudra();
        progress_info!("cargo rudra finished");
    } else if let Some("rustc") = std::env::args().nth(1).as_ref().map(AsRef::as_ref) {
        // This arm is executed when `cargo-rudra` runs `cargo rustc` with the `RUSTC_WRAPPER` env var set to itself:
        // dependencies get dispatched to `rustc`, the final test/binary to `rudra`.
        inside_cargo_rustc();
    } else {
        show_error(
            "`cargo-rudra` must be called with either `rudra` or `rustc` as first argument.",
        );
    }
}

#[repr(u8)]
enum TargetKind {
    Library = 0,
    Bin,
    Unknown,
}

impl TargetKind {
    fn is_lib_str(s: &str) -> bool {
        s == "lib" || s == "rlib" || s == "staticlib"
    }
}

impl From<&cargo_metadata::Target> for TargetKind {
    fn from(target: &cargo_metadata::Target) -> Self {
        if target.kind.iter().any(|s| TargetKind::is_lib_str(s)) {
            TargetKind::Library
        } else if let Some("bin") = target.kind.get(0).map(|s| s.as_ref()) {
            TargetKind::Bin
        } else {
            TargetKind::Unknown
        }
    }
}

impl Display for TargetKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TargetKind::Library => "lib",
                TargetKind::Bin => "bin",
                TargetKind::Unknown => "unknown",
            }
        )
    }
}

fn in_cargo_rudra() {
    let verbose = has_arg_flag("-v");

    // Some basic sanity checks
    test_sysroot_consistency();

    // Now run the command.
    let package = cargo_package();
    let mut targets: Vec<_> = package.targets.into_iter().collect();

    // Ensure `lib` is compiled before `bin`
    targets.sort_by_key(|target| TargetKind::from(target) as u8);

    for target in targets {
        // Skip `cargo rudra`
        let mut args = std::env::args().skip(2);
        let kind = TargetKind::from(&target);

        // Now we run `cargo check $FLAGS $ARGS`, giving the user the
        // change to add additional arguments. `FLAGS` is set to identify
        // this target. The user gets to control what gets actually passed to Rudra.
        let mut cmd = Command::new("cargo");
        cmd.arg("check");
        match kind {
            TargetKind::Bin => {
                // Analyze all the binaries.
                cmd.arg("--bin").arg(&target.name);
            }
            TargetKind::Library => {
                // There can be only one lib in a crate.
                cmd.arg("--lib");
                // Clean the result to disable Cargo's freshness check
                clean_package(&package.name);
            }
            TargetKind::Unknown => {
                warn!(
                    "Target {}:{} is not supported",
                    target.kind.as_slice().join("/"),
                    &target.name
                );
                continue;
            }
        }

        if !cfg!(debug_assertions) && !verbose {
            cmd.arg("-q");
        }

        // Forward user-defined `cargo` args until first `--`.
        while let Some(arg) = args.next() {
            if arg == "--" {
                break;
            }
            cmd.arg(arg);
        }

        // We want to always run `cargo` with `--target`. This later helps us detect
        // which crates are proc-macro/build-script (host crates) and which crates are
        // needed for the program itself.
        if get_arg_flag_value("--target").is_none() {
            // When no `--target` is given, default to the host.
            cmd.arg("--target");
            cmd.arg(version_info().host);
        }

        // Add suffix to RUDRA_REPORT_PATH
        if let Ok(report) = env::var("RUDRA_REPORT_PATH") {
            cmd.env(
                "RUDRA_REPORT_PATH",
                format!("{}-{}-{}", report, kind, &target.name),
            );
        }

        // Serialize the remaining args into a special environment variable.
        // This will be read by `inside_cargo_rustc` when we go to invoke
        // our actual target crate (the binary or the test we are running).
        // Since we're using "cargo check", we have no other way of passing
        // these arguments.
        let args_vec: Vec<String> = args.collect();
        cmd.env(
            "RUDRA_ARGS",
            serde_json::to_string(&args_vec).expect("failed to serialize args"),
        );

        // Set `RUSTC_WRAPPER` to ourselves.  Cargo will prepend that binary to its usual invocation,
        // i.e., the first argument is `rustc` -- which is what we use in `main` to distinguish
        // the two codepaths.
        if env::var_os("RUSTC_WRAPPER").is_some() {
            println!("WARNING: Ignoring existing `RUSTC_WRAPPER` environment variable, Rudra does not support wrapping.");
        }

        let path = std::env::current_exe().expect("current executable path invalid");
        cmd.env("RUSTC_WRAPPER", path);
        if verbose {
            cmd.env("RUDRA_VERBOSE", ""); // this makes `inside_cargo_rustc` verbose.
            eprintln!("+ {:?}", cmd);
        }

        progress_info!("Running rudra for target {}:{}", kind, &target.name);
        let mut child = cmd.spawn().expect("could not run cargo check");
        // 1 hour timeout
        match child
            .wait_timeout(Duration::from_secs(60 * 60))
            .expect("failed to wait for subprocess")
        {
            Some(exit_status) => {
                if !exit_status.success() {
                    show_error("Finished with non-zero exit code");
                }
            }
            None => {
                child.kill().expect("failed to kill subprocess");
                child.wait().expect("failed to wait for subprocess");
                show_error("Killed due to timeout");
            }
        };
    }
}

fn inside_cargo_rustc() {
    /// Determines if we are being invoked (as rustc) to build a crate for
    /// the "target" architecture, in contrast to the "host" architecture.
    /// Host crates are for build scripts and proc macros and still need to
    /// be built like normal; target crates need to be built for or interpreted
    /// by Rudra.
    ///
    /// Currently, we detect this by checking for "--target=", which is
    /// never set for host crates. This matches what rustc bootstrap does,
    /// which hopefully makes it "reliable enough". This relies on us always
    /// invoking cargo itself with `--target`, which `in_cargo_rudra` ensures.
    fn contains_target_flag() -> bool {
        get_arg_flag_value("--target").is_some()
    }

    /// Returns whether we are building the target crate.
    /// Cargo passes the file name as a relative address when building the local crate,
    /// such as `crawl/src/bin/unsafe-counter.rs` when building the target crate.
    /// This might not be a stable behavior, but let's rely on this for now.
    fn is_target_crate() -> bool {
        let entry_path_arg = match get_first_arg_with_rs_suffix() {
            Some(arg) => arg,
            None => return false,
        };
        let entry_path: &Path = entry_path_arg.as_ref();

        entry_path.is_relative()
    }

    fn is_crate_type_lib() -> bool {
        any_arg_flag("--crate-type", TargetKind::is_lib_str)
    }

    fn run_command(mut cmd: Command) {
        // Run it.
        let verbose = std::env::var_os("RUDRA_VERBOSE").is_some();
        if verbose {
            eprintln!("+ {:?}", cmd);
        }

        match cmd.status() {
            Ok(exit) => {
                if !exit.success() {
                    std::process::exit(exit.code().unwrap_or(42));
                }
            }
            Err(e) => panic!("error running {:?}:\n{:?}", cmd, e),
        }
    }

    // TODO: Miri sets custom sysroot here, check if it is needed for us (RUDRA-30)

    let needs_rudra_analysis = contains_target_flag() && is_target_crate();
    if needs_rudra_analysis {
        let mut cmd = Command::new(find_rudra());
        cmd.args(std::env::args().skip(2)); // skip `cargo-rudra rustc`

        // This is the local crate that we want to analyze with Rudra.
        // (Testing `target_crate` is needed to exclude build scripts.)
        // We deserialize the arguments that are meant for Rudra from the special
        // environment variable "RUDRA_ARGS", and feed them to the 'rudra' binary.
        //
        // `env::var` is okay here, well-formed JSON is always UTF-8.
        let magic = std::env::var("RUDRA_ARGS").expect("missing RUDRA_ARGS");
        let rudra_args: Vec<String> =
            serde_json::from_str(&magic).expect("failed to deserialize RUDRA_ARGS");
        cmd.args(rudra_args);

        run_command(cmd);
    }

    // Libraries might be used for dependency, so we need to analyze and build it.
    if !needs_rudra_analysis || is_crate_type_lib() {
        let mut cmd = Command::new(find_rudra());
        cmd.args(std::env::args().skip(2)); // skip `cargo-rudra rustc`

        // We want to compile, not interpret.
        cmd.env("RUDRA_BE_RUSTC", "1");

        run_command(cmd);
    }
}
