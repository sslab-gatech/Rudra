use std::path::{Path, PathBuf};
use std::process::Command;

const MARKER_START: &str = "cargo-crux-marker-start";
const MARKER_END: &str = "cargo-crux-marker-end";

const CARGO_CRUX_HELP: &str = r#"Tests crates with Crux
Usage:
    cargo crux [options] [--] [<crux opts>...]

Common options:
    -h, --help               Print this message

Other [options] are the same as `cargo rustc`.  Everything after the first "--" is
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
        .expect("could not find matching package");
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
        // This arm is executed when `cargo-miri` runs `cargo rustc` with the `RUSTC_WRAPPER` env var set to itself:
        // dependencies get dispatched to `rustc`, the final test/binary to `miri`.
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
        // Now we run `cargo rustc $FLAGS $ARGS`, giving the user the
        // change to add additional arguments. `FLAGS` is set to identify
        // this target. The user gets to control what gets actually passed to Crux.
        let mut cmd = Command::new("cargo");
        cmd.arg("rustc");
        match kind.as_str() {
            // Only libraries are supported at this point
            "lib" => {
                // There can be only one lib in a crate.
                cmd.arg("--lib");
            }
            s => {
                println!("Target {}:{} is not supported", s, target.name);
            }
        }

        // Add user-defined args until first `--`.
        while let Some(arg) = args.next() {
            if arg == "--" {
                break;
            }
            cmd.arg(arg);
        }

        // Add `--` (to end the `cargo` flags), and then the user flags. We add markers around the
        // user flags to be able to identify them later.  "cargo rustc" adds more stuff after this,
        // so we have to mark both the beginning and the end.
        cmd.arg("--").arg(MARKER_START).args(args).arg(MARKER_END);
        let path = std::env::current_exe().expect("current executable path invalid");
        cmd.env("RUSTC_WRAPPER", path);
        if verbose {
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
    let rustc_args = std::env::args().skip(2); // skip `cargo rustc`
    let mut args: Vec<String> = rustc_args.collect();
    args.splice(
        0..0,
        crux::CRUX_DEFAULT_ARGS.iter().map(ToString::to_string),
    );

    // See if we can find the `cargo-crux` markers. Those only get added to the binary we want to
    // run. They also serve to mark the user-defined arguments, which we have to move all the way
    // to the end (they get added somewhere in the middle).
    let needs_crux = if let Some(begin) = args.iter().position(|arg| arg == MARKER_START) {
        let end = args
            .iter()
            .position(|arg| arg == MARKER_END)
            .expect("cannot find end marker");

        // These mark the user arguments. We remove the first and last as they are the markers.
        let mut user_args = args.drain(begin..=end);
        assert_eq!(user_args.next().unwrap(), MARKER_START);
        assert_eq!(user_args.next_back().unwrap(), MARKER_END);

        // Collect the rest and add it back at the end.
        let mut user_args = user_args.collect::<Vec<String>>();
        args.append(&mut user_args);

        // Run this in Crux
        true
    } else {
        false
    };

    let mut command = if needs_crux {
        Command::new(find_crux())
    } else {
        Command::new("rustc")
    };
    command.args(&args);
    if has_arg_flag("-v") {
        eprintln!("+ {:?}", command);
    }

    match command.status() {
        Ok(exit) => {
            if !exit.success() {
                std::process::exit(exit.code().unwrap_or(-1));
            }
        }
        Err(e) if needs_crux => panic!("error during crux run: {:?}", e),
        Err(e) => panic!("error during rustc call: {:?}", e),
    }
}
