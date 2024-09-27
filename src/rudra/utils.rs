use std::io::Write;
use std::rc::Rc;

//use rustc_middle::mir::write_mir_pretty;
//use rustc_middle::ty::{Instance, InstanceDef, TyCtxt};
//use rustc_span::{CharPos, Span};

use charon_lib::ast::meta::{FileName, Loc, Span};
use charon_lib::ast::TranslatedCrate;
use termcolor::{Buffer, Color, ColorSpec, WriteColor};
use tracing::{error, info, warn};

use crate::rudra::lib::compile_time_sysroot;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
struct ColorEventId(usize);

struct ColorStack(Vec<(Color, ColorEventId)>);

impl ColorStack {
    pub fn new() -> Self {
        ColorStack(Vec::new())
    }

    pub fn handle_event(&mut self, event: &ColorEvent) {
        match event.color {
            Some(color) => self.0.push((color, event.id)),
            None => {
                for i in (0..self.0.len()).rev() {
                    if self.0[i].1 == event.id {
                        self.0.remove(i);
                        return;
                    }
                }
            }
        };
    }

    pub fn current_spec(&self) -> ColorSpec {
        let mut spec = ColorSpec::new();

        match self.0.last() {
            Some((color, _)) => spec.set_fg(Some(*color)),
            None => spec.set_reset(true),
        };

        spec
    }
}

#[derive(Clone)]
struct ColorEvent {
    // Some(color) for start, None for clear
    color: Option<Color>,
    line: usize,
    col: usize,
    id: ColorEventId,
}

pub struct ColorSpan<'tcx> {
    crate_data: &'tcx TranslatedCrate,
    pub main_span: Span,
    id_counter: usize,
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

impl<'tcx> ColorSpan<'tcx> {
    pub fn new(crate_data: &'tcx TranslatedCrate, main_span: Span) -> Option<Self> {
        Some(ColorSpan {
            crate_data,
            main_span,
            id_counter: 0,
            sub_span_events: Vec::new(),
        })
    }

    pub fn main_span(&self) -> Span {
        self.main_span
    }

    /// Returns true if span is successfully added
    pub fn add_sub_span(&mut self, color: Color, span: Span) -> bool {
        // Reports from macros may be in another file and we don't handle them
        if span.span.file_id != self.main_span.span.file_id {
            return false;
        }

        let event_id = ColorEventId(self.id_counter);
        self.id_counter += 1;

        self.sub_span_events.push(ColorEvent {
            color: Some(color),
            line: span.span.beg.line,
            col: span.span.beg.col,
            id: event_id,
        });
        self.sub_span_events.push(ColorEvent {
            color: None,
            line: span.span.end.line,
            col: span.span.end.col,
            id: event_id,
        });
        return true;
    }

    pub fn to_colored_string(&self) -> String {
        let mut events = self.sub_span_events.clone();
        events.sort();
        let mut events_iter = events.into_iter().peekable();

        let mut buffer = Buffer::ansi();

        if let Ok(snippet) = span_to_snippet(&self.crate_data, &self.main_span) {
            let start_loc = self.main_span.span.beg;
            let end_loc = self.main_span.span.end;

            while let Some(event) = events_iter.peek() {
                if event.line < start_loc.line {
                    // Discard spans before the start loc
                    events_iter.next();
                } else {
                    break;
                }
            }

            let mut color_stack = ColorStack::new();
            for (line_idx, line_content) in (start_loc.line..=end_loc.line).zip(snippet.into_iter())
            {
                let mut current_col = if line_idx == start_loc.line {
                    start_loc.col
                } else {
                    0
                };

                let mut handle_color_event = |buffer: &mut Buffer, col: usize| {
                    while let Some(event) = events_iter.peek() {
                        if event.line == line_idx && event.col == col {
                            color_stack.handle_event(event);
                            events_iter.next();

                            let spec = color_stack.current_spec();
                            buffer.set_color(&spec).map_err(|e| warn!("{}", e)).ok();
                        } else {
                            break;
                        }
                    }
                };

                for ch in line_content.chars() {
                    handle_color_event(&mut buffer, current_col);
                    write!(buffer, "{}", ch).ok();
                    current_col += 1;
                }

                // Handle reset
                handle_color_event(&mut buffer, current_col);
                write!(buffer, "\n").ok();
            }

            // Reset the color after printing the span just in case
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

// TODO: move to Charon
pub fn span_to_snippet(crate_data: &TranslatedCrate, span: &Span) -> Result<Vec<String>, ()> {
    use std::fs::File;
    use std::io::prelude::*;
    use std::io::BufReader;
    use take_mut::take;
    use FileName::*;
    let file = &crate_data.id_to_file[span.span.file_id];
    match file {
        Local(path) => {
            let file = if let Ok(file) = File::open(path) {
                file
            } else {
                return Err(());
            };
            // This is not meant to be efficient
            let content: Result<Vec<String>, _> =
                BufReader::new(file).lines().into_iter().collect();
            let content = if let Ok(content) = content {
                content
            } else {
                return Err(());
            };
            let mut lines: Vec<_> =
                Vec::from(&content[span.span.beg.line - 1..span.span.end.line - 1]);
            if lines.len() <= 0 {
                return Err(());
            }
            // Shift the columns
            let lines_len = lines.len();
            take(&mut lines[0], |l| {
                l.chars().skip(span.span.beg.col).collect()
            });
            take(&mut lines[lines_len - 1], |l| {
                l.chars().take(span.span.end.col).collect()
            });

            Ok(lines)
        }
        _ => Err(()),
    }
}

pub fn print_span(crate_data: &TranslatedCrate, span: &Span) {
    let snippet = span_to_snippet(crate_data, span).unwrap().join("\n");
    eprintln!("{:?}\n{}\n", span, snippet);
}

pub fn print_span_to_file(crate_data: &TranslatedCrate, span: &Span, output_name: &str) {
    let sysroot = compile_time_sysroot().expect("Failed to fetch sysroot");
    let filename = format!("{}/logs/{}", sysroot, output_name);
    let snippet = span_to_snippet(crate_data, span).unwrap().join("\n");
    let content = format!("{:?}\n{}\n", span, snippet);
    std::fs::write(filename, content).expect("Unable to write file");
}

/*pub fn print_mir<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) {
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
*/
