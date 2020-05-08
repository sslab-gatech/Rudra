use crux::CRUX_DEFAULT_ARGS;
///! This implementation is based on `cargo-miri`
///! https://github.com/rust-lang/miri/blob/master/src/bin/cargo-miri.rs
use std::path::{Path, PathBuf};
use std::process::Command;

const CARGO_CRUX_HELP: &str = r#"Tests crates with Crux
Usage:
    cargo crux [options] [--] [<crux opts>...]

Common options:
    -h, --help               Print this message

Other [options] are the same as `cargo check`. Everything after the first "--" is
passed verbatim to Crux.
"#;

fn show_help() {
    println!("{}", CARGO_CRUX_HELP);
}

fn show_error(msg: String) -> ! {
    eprintln!("fatal error: {}", msg);
    std::process::exit(1)
}

// Determines whether a `--flag` is present.
fn has_arg_flag(name: &str) -> bool {
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

fn list_targets() -> impl Iterator<Item = cargo_metadata::Target> {
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
        show_error(format!("Could not obtain Cargo metadata"));
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
            show_error(format!(
                "This seems to be a workspace, which is not supported by cargo-crux"
            ))
        });
    let package = metadata.packages.remove(package_index);

    // Finally we got the list of targets to build
    package.targets.into_iter()
}

/// Returns the path to the `crux` binary
fn find_crux() -> PathBuf {
    let mut path = std::env::current_exe().expect("current executable path invalid");
    path.set_file_name("crux");
    path
}

/// Make sure that the `crux` and `rustc` binary are from the same sysroot.
/// This can be violated e.g. when crux is locally built and installed with a different
/// toolchain than what is used when `cargo crux` is run.
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
    let crux_sysroot = get_sysroot(Command::new(find_crux()));

    if rustc_sysroot != crux_sysroot {
        show_error(format!(
            "crux was built for a different sysroot than the rustc in your current toolchain.\n\
             Make sure you use the same toolchain to run crux that you used to build it!\n\
             rustc sysroot: `{}`\n\
             crux sysroot: `{}`",
            rustc_sysroot.display(),
            crux_sysroot.display()
        ));
    }
}

fn main() {
    // Check for version and help flags even when invoked as `cargo-crux`.
    if std::env::args().any(|a| a == "--help" || a == "-h") {
        show_help();
        return;
    }

    if let Some("crux") = std::env::args().nth(1).as_ref().map(AsRef::as_ref) {
        // This arm is for when `cargo crux` is called. We call `cargo rustc` for each applicable target,
        // but with the `RUSTC` env var set to the `cargo-crux` binary so that we come back in the other branch,
        // and dispatch the invocations to `rustc` and `crux`, respectively.
        in_cargo_crux();
    } else if let Some("rustc") = std::env::args().nth(1).as_ref().map(AsRef::as_ref) {
        // This arm is executed when `cargo-crux` runs `cargo rustc` with the `RUSTC_WRAPPER` env var set to itself:
        // dependencies get dispatched to `rustc`, the final test/binary to `crux`.
        inside_cargo_rustc();
    } else {
        show_error(format!(
            "must be called with either `crux` or `rustc` as first argument."
        ))
    }
}

fn in_cargo_crux() {
    let verbose = has_arg_flag("-v");

    // Some basic sanity checks
    test_sysroot_consistency();

    // Now run the command.
    for target in list_targets() {
        // Skip `cargo crux`
        let mut args = std::env::args().skip(2);
        let kind = target
            .kind
            .get(0)
            .expect("badly formatted cargo metadata: target::kind is an empty array");

        // Now we run `cargo check $FLAGS $ARGS`, giving the user the
        // change to add additional arguments. `FLAGS` is set to identify
        // this target. The user gets to control what gets actually passed to Crux.
        let mut cmd = Command::new("cargo");
        cmd.arg("check");
        match kind.as_str() {
            // Only libraries are supported at this point
            "bin" => {
                // FIXME: we just analyze all the binaries here.
                // We should instead support `cargo crux --bin foo`.
                cmd.arg("--bin").arg(target.name);
            }
            "lib" => {
                // There can be only one lib in a crate.
                cmd.arg("--lib");
            }
            s => {
                println!("Target {}:{} is not supported", s, target.name);
                continue;
            }
        }

        // Forward user-defined `cargo` args until first `--`.
        while let Some(arg) = args.next() {
            if arg == "--" {
                break;
            }
            cmd.arg(arg);
        }

        // Serialize the remaining args into a special environemt variable.
        // This will be read by `inside_cargo_rustc` when we go to invoke
        // our actual target crate (the binary or the test we are running).
        // Since we're using "cargo check", we have no other way of passing
        // these arguments.
        let args_vec: Vec<String> = args.collect();
        cmd.env(
            "CRUX_ARGS",
            serde_json::to_string(&args_vec).expect("failed to serialize args"),
        );

        // Set `RUSTC_WRAPPER` to ourselves.  Cargo will prepend that binary to its usual invocation,
        // i.e., the first argument is `rustc` -- which is what we use in `main` to distinguish
        // the two codepaths.
        let path = std::env::current_exe().expect("current executable path invalid");
        cmd.env("RUSTC_WRAPPER", path);
        if verbose {
            cmd.env("CRUX_VERBOSE", ""); // this makes `inside_cargo_rustc` verbose.
            eprintln!("+ {:?}", cmd);
        }

        let exit_status = cmd
            .spawn()
            .expect("could not run cargo")
            .wait()
            .expect("failed to wait for cargo?");

        if !exit_status.success() {
            std::process::exit(exit_status.code().unwrap_or(-1))
        }
    }
}

fn inside_cargo_rustc() {
    /// Determines if we are being invoked (as rustc) to build a runnable
    /// executable. We run "cargo check", so this should only happen when
    /// we are trying to compile a build script or build script dependency,
    /// which actually needs to be executed on the host platform.
    ///
    /// Currently, we detect this by checking for "--emit=link",
    /// which indicates that Cargo instruced rustc to output
    /// a native object.
    fn is_target_crate() -> bool {
        // `--emit` is sometimes missing, e.g. cargo calls rustc for "--print".
        // That is definitely not a target crate.
        // If `--emit` is present, then host crates are built ("--emit=link,...),
        // while the rest is only checked.
        get_arg_flag_value("--emit").map_or(false, |emit| !emit.contains("link"))
    }

    /// Returns whether or not Cargo invoked the wrapper (this binary) to compile
    /// the final, target crate (either a test for 'cargo test', or a binary for 'cargo run')
    /// Cargo does not give us this information directly, so we need to check
    /// various command-line flags.
    fn is_runnable_crate() -> bool {
        let is_bin = get_arg_flag_value("--crate-type").as_deref() == Some("bin");
        let is_test = has_arg_flag("--test");

        // The final runnable (under Crux) crate will either be a binary crate
        // or a test crate. We make sure to exclude build scripts here, since
        // they are also build with "--crate-type bin"
        is_bin || is_test
    }

    let verbose = std::env::var("CRUX_VERBOSE").is_ok();
    let target_crate = is_target_crate();

    // Figure out which arguments we need to pass.
    let mut args: Vec<String> = std::env::args().skip(2).collect(); // skip `cargo-crux rustc`

    if target_crate {
        // TODO: Miri sets custom sysroot here, check if it is needed (CRUX-30)
        args.splice(0..0, CRUX_DEFAULT_ARGS.iter().map(ToString::to_string));
    }

    // Figure out the binary we need to call. If this is a runnable target crate, we want to call
    // Crux to start interpretation; otherwise we want to call rustc to build the crate as usual.
    let mut command = if target_crate && is_runnable_crate() {
        // This is the 'target crate' - the binary or test crate that
        // we want to interpret under Crux. We deserialize the user-provided arguments
        // from the special environment variable "CRUX_ARGS", and feed them
        // to the 'crux' binary.
        let magic = std::env::var("CRUX_ARGS").expect("missing CRUX_ARGS");
        let mut user_args: Vec<String> =
            serde_json::from_str(&magic).expect("failed to deserialize CRUX_ARGS");
        args.append(&mut user_args);
        // Run this in Crux.
        Command::new(find_crux())
    } else {
        Command::new("rustc")
    };

    // Run it.
    command.args(&args);
    if verbose {
        eprintln!("+ {:?}", command);
    }

    match command.status() {
        Ok(exit) => {
            if !exit.success() {
                std::process::exit(exit.code().unwrap_or(42));
            }
        }
        Err(e) => panic!("error running {:?}:\n{:?}", command, e),
    }
}
