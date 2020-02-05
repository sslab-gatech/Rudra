use rustc::ty::{Instance, InstanceDef, TyCtxt};
use rustc_mir::util::write_mir_pretty;
use rustc_span::Span;

// TODO: support dump to another file

pub fn print_span<'tcx>(tcx: TyCtxt<'tcx>, span: &Span) {
    let source_map = tcx.sess.source_map();
    println!(
        "{}\n{}\n",
        source_map.span_to_string(span.clone()),
        source_map.span_to_snippet(span.clone()).unwrap()
    );
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
