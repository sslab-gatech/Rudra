use std::io::Write;
use std::rc::Rc;

use rustc_middle::ty::{Instance, InstanceDef, TyCtxt};
use rustc_mir::util::write_mir_pretty;
use rustc_span::{CharPos, Span};

use termcolor::{Buffer, Color, ColorSpec, WriteColor};

use crate::compile_time_sysroot;

#[derive(Clone)]
struct ColorEvent {
    // Some(color) for start, None for clear
    color: Option<Color>,
    line: usize,
    col: CharPos,
}

pub struct NestedColorSpan<'tcx> {
    tcx: TyCtxt<'tcx>,
    main_span: Span,
    main_span_start: rustc_span::Loc,
    main_span_end: rustc_span::Loc,
    sub_span_events: Vec<ColorEvent>,
}

impl PartialEq for ColorEvent {
    fn eq(&self, other: &Self) -> bool {
        self.line == other.line
            && self.col == other.col
            && self.color.is_some() == other.color.is_some()
    }
}

impl Eq for ColorEvent {}

impl PartialOrd for ColorEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ColorEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.line != other.line {
            return self.line.cmp(&other.line);
        }

        if self.col != other.col {
            return self.col.cmp(&other.col);
        }

        if self.color.is_some() != other.color.is_some() {
            return self.color.is_some().cmp(&other.color.is_some());
        }

        std::cmp::Ordering::Equal
    }
}

impl<'tcx> NestedColorSpan<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, main_span: Span) -> Option<Self> {
        let source_map = tcx.sess.source_map();
        if let Ok((main_span_start, main_span_end)) = source_map.is_valid_span(main_span) {
            // Sanity check
            if !Rc::ptr_eq(&main_span_start.file, &main_span_end.file) {
                return None;
            }

            Some(NestedColorSpan {
                tcx,
                main_span,
                main_span_start,
                main_span_end,
                sub_span_events: Vec::new(),
            })
        } else {
            None
        }
    }

    pub fn main_span(&self) -> Span {
        self.main_span
    }

    /// Returns true if span is successfully added
    pub fn add_sub_span(&mut self, color: Color, span: Span) -> bool {
        let source_map = self.tcx.sess.source_map();
        if let Ok((start_loc, end_loc)) = source_map.is_valid_span(span) {
            // Sanity check
            if !Rc::ptr_eq(&start_loc.file, &self.main_span_start.file)
                || !Rc::ptr_eq(&start_loc.file, &self.main_span_end.file)
            {
                return false;
            }
            self.sub_span_events.push(ColorEvent {
                color: Some(color),
                line: start_loc.line,
                col: start_loc.col,
            });
            self.sub_span_events.push(ColorEvent {
                color: None,
                line: end_loc.line,
                col: end_loc.col,
            });
            return true;
        } else {
            return false;
        }
    }

    pub fn to_colored_string(&self) -> String {
        let mut events = self.sub_span_events.clone();
        events.sort();
        let mut events_iter = events.into_iter().peekable();

        let source_map = self.tcx.sess.source_map();
        let mut buffer = Buffer::ansi();

        if let (Ok((start_loc, end_loc)), Ok(snippet)) = (
            source_map.is_valid_span(self.main_span),
            source_map.span_to_snippet(self.main_span),
        ) {
            let start_line = start_loc.line;
            let end_line = end_loc.line;

            for (line_idx, line_content) in (start_line..=end_line).zip(snippet.lines()) {
                let mut current_col = if line_idx == start_line {
                    start_loc.col
                } else {
                    CharPos(0)
                };

                for ch in line_content.chars() {
                    // Handle before-char color event
                    if let Some(event) = events_iter.peek() {
                        if event.line == line_idx
                            && event.col == current_col
                            && event.color.is_some()
                        {
                            buffer
                                .set_color(ColorSpec::new().set_fg(event.color))
                                .map_err(|e| warn!("{}", e))
                                .ok();

                            events_iter.next();
                        }
                    }

                    write!(buffer, "{}", ch).ok();

                    // Handle after-char color event
                    if let Some(event) = events_iter.peek() {
                        if event.line == line_idx
                            && event.col == current_col
                            && event.color.is_none()
                        {
                            buffer
                                .set_color(ColorSpec::new().set_reset(true))
                                .map_err(|e| warn!("{}", e))
                                .ok();

                            events_iter.next();
                        }
                    }

                    current_col.0 += 1;
                }

                write!(buffer, "\n").ok();

                // Final character might be off-by-one
                if let Some(event) = events_iter.peek() {
                    if event.line == line_idx && event.col == current_col && event.color.is_none() {
                        buffer
                            .set_color(ColorSpec::new().set_reset(true))
                            .map_err(|e| warn!("{}", e))
                            .ok();

                        events_iter.next();
                    }
                }
            }

            // Just in case, reset the color
            buffer
                .set_color(ColorSpec::new().set_reset(true))
                .map_err(|e| warn!("{}", e))
                .ok();

            String::from_utf8_lossy(buffer.as_slice()).into()
        } else {
            format!("Unable to get span for {:?}", self.main_span)
        }
    }
}

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
