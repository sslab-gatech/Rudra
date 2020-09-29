use rustc_middle::ty::{Instance, InstanceDef, TyCtxt};
use rustc_mir::util::write_mir_pretty;
use rustc_span::Span;

use crate::compile_time_sysroot;

pub fn print_span<'tcx>(tcx: TyCtxt<'tcx>, span: &Span) {
    let source_map = tcx.sess.source_map();
    eprintln!(
        "{}\n{}\n",
        source_map.span_to_string(span.clone()),
        source_map.span_to_snippet(span.clone()).unwrap()
    );
}

pub fn print_span_to_file<'tcx>(tcx: TyCtxt<'tcx>, span: &Span, output_name: &str) {
    let source_map = tcx.sess.source_map();
    let sysroot = compile_time_sysroot().expect("Failed to fetch sysroot");
    let filename = format!("{}/logs/{}", sysroot, output_name);
    let content = format!(
        "{}\n{}\n",
        source_map.span_to_string(span.clone()),
        source_map.span_to_snippet(span.clone()).unwrap()
    );
    std::fs::write(filename, content).expect("Unable to write file");
}

pub fn print_mir<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) {
    info!("Printing MIR for {:?}", instance);

    match instance.def {
        InstanceDef::Item(_) => {
            if tcx.is_mir_available(instance.def.def_id()) {
                let stderr = std::io::stderr();
                let mut handle = stderr.lock();
                if let Err(_) = write_mir_pretty(tcx, Some(instance.def.def_id()), &mut handle) {
                    error!(
                        "Cannot print MIR: error while printing `{:?}`",
                        instance.def.def_id()
                    );
                }
            } else {
                info!("Cannot print MIR: no MIR for `{:?}`", &instance);
            }
        }
        _ => info!("Cannot print MIR: `{:?}` is a shim", instance),
    }
}

pub fn print_mir_to_file<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, output_name: &str) {
    let sysroot = compile_time_sysroot().expect("Failed to fetch sysroot");
    let filename = format!("{}/logs/{}", sysroot, output_name);
    info!("Printing MIR for {:?} to {}", instance, filename);

    match instance.def {
        InstanceDef::Item(_) => {
            if tcx.is_mir_available(instance.def.def_id()) {
                let mut handle =
                    std::fs::File::create(filename).expect("Error while creating file");
                if let Err(_) = write_mir_pretty(tcx, Some(instance.def.def_id()), &mut handle) {
                    error!(
                        "Cannot print MIR: error while printing `{:?}`",
                        instance.def.def_id()
                    );
                }
            } else {
                info!("Cannot print MIR: no MIR for `{:?}`", &instance);
            }
        }
        _ => info!("Cannot print MIR: `{:?}` is a shim", instance),
    }
}
