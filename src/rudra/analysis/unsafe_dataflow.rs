/*use rustc_hir::{def_id::DefId, BodyId};
use rustc_middle::mir::Operand;
use rustc_middle::ty::{Instance, ParamEnv, TyKind};
use rustc_span::{Span, DUMMY_SP};*/

use crate::rudra::context::{CtxOwner, RudraCtxt};
use snafu::{Backtrace, Snafu};
use termcolor::Color;

//use crate::prelude::*;
use crate::rudra::graph::GraphTaint;
use crate::rudra::report::rudra_report;
use crate::rudra::{
    analysis::{AnalysisError, AnalysisErrorKind, AnalysisKind, IntoReportLevel},
    graph::TaintAnalyzer,
    paths::{self, *},
    report::{Report, ReportLevel},
    utils,
    //visitor::ContainsUnsafe,
};
use bitflags::bitflags;
use if_chain::if_chain;

use crate::rudra::macros::unwrap_or;
use charon_lib::ast::meta::Span;
use charon_lib::formatter::{Formatter, IntoFormatter};
use charon_lib::gast::{Body, BodyId, FunDeclId};
use charon_lib::ids::Vector;
use charon_lib::name_matcher::Pattern;
use charon_lib::types::{GenericArgs, Ty};
use charon_lib::ullbc_ast::{
    BodyContents, Call, FnOperand, FnPtr, FunDecl, FunId, FunIdOrTraitMethodRef, Literal, Operand,
    RawConstantExpr, RawStatement, ScalarValue, TraitRefKind,
};
use tracing::{error, info, warn};

#[derive(Debug, Snafu)]
pub enum UnsafeDataflowError {
    PushPopBlock { backtrace: Backtrace },
    ResolveError { backtrace: Backtrace },
    InvalidSpan { backtrace: Backtrace },
}

impl AnalysisError for UnsafeDataflowError {
    fn kind(&self) -> AnalysisErrorKind {
        use UnsafeDataflowError::*;
        match self {
            PushPopBlock { .. } => AnalysisErrorKind::Unreachable,
            ResolveError { .. } => AnalysisErrorKind::OutOfScope,
            InvalidSpan { .. } => AnalysisErrorKind::Unreachable,
        }
    }
}

#[derive(Clone, Copy)]
pub struct UnsafeDataflowChecker<'tcx> {
    rcx: RudraCtxt<'tcx>,
}

impl<'tcx> UnsafeDataflowChecker<'tcx> {
    pub fn new(rcx: RudraCtxt<'tcx>) -> Self {
        UnsafeDataflowChecker { rcx }
    }

    pub fn analyze(self) {
        // Iterate over all functions
        for decl in self.rcx.crate_data.fun_decls.iter() {
            if let Some(status) = inner::UnsafeDataflowBodyAnalyzer::analyze_body(self.rcx, decl) {
                let behavior_flag = status.behavior_flag();
                if !behavior_flag.is_empty()
                    && behavior_flag.report_level() >= self.rcx.report_level()
                {
                    let mut color_span = if let Some(span) =
                        utils::ColorSpan::new(&self.rcx.crate_data, decl.item_meta.span)
                    {
                        span
                    } else {
                        continue;
                    };

                    for &span in status.strong_bypass_spans() {
                        color_span.add_sub_span(Color::Red, span);
                    }

                    for &span in status.weak_bypass_spans() {
                        color_span.add_sub_span(Color::Yellow, span);
                    }

                    for &span in status.unresolvable_generic_function_spans() {
                        color_span.add_sub_span(Color::Cyan, span);
                    }

                    rudra_report(Report::with_color_span(
                        behavior_flag.report_level(),
                        AnalysisKind::UnsafeDataflow(behavior_flag),
                        format!(
                            "Potential unsafe dataflow issue in `{}`",
                            self.rcx.crate_data.into_fmt().format_object(decl.def_id)
                        ),
                        &color_span,
                    ))
                }
            }
        }
    }
}

mod inner {
    use super::*;

    #[derive(Debug, Default)]
    pub struct UnsafeDataflowStatus {
        strong_bypasses: Vec<Span>,
        weak_bypasses: Vec<Span>,
        unresolvable_generic_functions: Vec<Span>,
        behavior_flag: BehaviorFlag,
    }

    impl UnsafeDataflowStatus {
        pub fn behavior_flag(&self) -> BehaviorFlag {
            self.behavior_flag
        }

        pub fn strong_bypass_spans(&self) -> &Vec<Span> {
            &self.strong_bypasses
        }

        pub fn weak_bypass_spans(&self) -> &Vec<Span> {
            &self.weak_bypasses
        }

        pub fn unresolvable_generic_function_spans(&self) -> &Vec<Span> {
            &self.unresolvable_generic_functions
        }
    }

    pub struct UnsafeDataflowBodyAnalyzer<'a, 'tcx> {
        rcx: RudraCtxt<'tcx>,
        body: &'a BodyContents,
        status: UnsafeDataflowStatus,
        ptr_read_set: PathSet,
        ptr_write_set: PathSet,
        vec_set_len: Pattern,
    }

    impl<'a, 'tcx> UnsafeDataflowBodyAnalyzer<'a, 'tcx> {
        fn new(rcx: RudraCtxt<'tcx>, body: &'a BodyContents) -> Self {
            UnsafeDataflowBodyAnalyzer {
                rcx,
                body,
                status: Default::default(),
                ptr_read_set: PathSet::new(&[&PTR_READ[..], &PTR_DIRECT_READ[..]]),
                ptr_write_set: PathSet::new(&[&PTR_WRITE[..], &PTR_DIRECT_WRITE[..]]),
                vec_set_len: Pattern::parse(&crate::rudra::paths::slice_to_string(&VEC_SET_LEN))
                    .unwrap(),
            }
        }

        pub fn analyze_body(rcx: RudraCtxt<'tcx>, decl: &FunDecl) -> Option<UnsafeDataflowStatus> {
            let path_discovery_set = PathSet::new(&[
                &["rudra_paths_discovery"],
                &["PathsDiscovery"],
                &["discover"],
            ]);
            let body_id = if let Ok(id) = decl.body {
                id
            } else {
                return None;
            };
            let body = if let Some(body) = rcx.crate_data.bodies.get(body_id) {
                body
            } else {
                return None;
            };

            if path_discovery_set
                .contains(rcx, &decl.item_meta.name)
                .is_some()
            {
                // Special case for paths discovery
                trace_calls_in_body(rcx, body);
                None
            }
            // We don't check if there is unsafe code
            else
            /*if ContainsUnsafe::contains_unsafe(rcx.tcx(), body_id)*/
            {
                /*match rcx.translate_body(body_did).as_ref() {
                        Err(e) => {
                            // MIR is not available for def - log it and continue
                            e.log();
                            None
                        }
                        Ok(body) => {
                            let body_analyzer = UnsafeDataflowBodyAnalyzer::new(rcx, body);
                            Some(body_analyzer.analyze())
                        }
                }*/
                let body_analyzer =
                    UnsafeDataflowBodyAnalyzer::new(rcx, &body.as_unstructured().unwrap().body);
                Some(body_analyzer.analyze())
            } /*else {
                  // We don't perform interprocedural analysis,
                  // thus safe functions are considered safe
                  Some(Default::default())
              }*/
        }

        fn analyze(mut self) -> UnsafeDataflowStatus {
            let mut taint_analyzer = TaintAnalyzer::new(self.body);
            let fmt = &self.rcx.crate_data.into_fmt();
            use charon_lib::pretty::FmtWithCtx;

            for (id, block) in self.body.iter_indexed() {
                for st in &block.statements {
                    match &st.content {
                        RawStatement::Call(Call {
                            func:
                                FnOperand::Regular(FnPtr {
                                    func: FunIdOrTraitMethodRef::Fun(FunId::Regular(callee_did)),
                                    generics,
                                    ..
                                }),
                            args,
                            ..
                        }) => {
                            // Check for lifetime bypass
                            let decl = if let Some(decl) =
                                self.rcx.crate_data.fun_decls.get(*callee_did)
                            {
                                decl
                            } else {
                                warn!("Could not find {callee_did}");
                                break;
                            };
                            let name = &decl.item_meta.name;
                            let name_str = name.fmt_with_ctx(fmt);
                            log::trace!("Analyzing fun call: {name_str}\n");
                            if let Some(pname) =
                                paths::STRONG_LIFETIME_BYPASS_LIST.contains(self.rcx, name)
                            {
                                log::trace!(
                                    "Found potential strong lifetime bypass: {name_str} (block: {id})"
                                );
                                if self.fn_called_on_copy(*callee_did, generics, &self.ptr_read_set)
                                {
                                    // read on Copy types is not a lifetime bypass.
                                    continue;
                                }

                                if self.vec_set_len.matches(&self.rcx.crate_data, name)
                                    && vec_set_len_to_0(args)
                                {
                                    // Leaking data is safe (`vec.set_len(0);`)
                                    continue;
                                }
                                log::trace!(
                                    "Found strong lifetime bypass: {name_str} (block: {id})"
                                );

                                taint_analyzer
                                    .mark_source(id.index(), STRONG_BYPASS_MAP.get(pname).unwrap());
                                self.status.strong_bypasses.push(st.span);
                            } else if let Some(pname) =
                                paths::WEAK_LIFETIME_BYPASS_LIST.contains(self.rcx, name)
                            {
                                if self.fn_called_on_copy(
                                    *callee_did,
                                    generics,
                                    &self.ptr_write_set,
                                ) {
                                    // writing Copy types is not a lifetime bypass.
                                    continue;
                                }
                                log::trace!("Found weak lifetime bypass: {name_str} (block: {id})");

                                taint_analyzer
                                    .mark_source(id.index(), WEAK_BYPASS_MAP.get(pname).unwrap());
                                self.status.weak_bypasses.push(st.span);
                            } else if paths::GENERIC_FN_LIST.contains(self.rcx, name).is_some() {
                                log::trace!(
                                    "Found unresolvable generic function: {name_str} (block: {id})"
                                );
                                taint_analyzer.mark_sink(id.index());
                                self.status.unresolvable_generic_functions.push(st.span);
                            } else {
                                // Check for unresolvable generic function calls
                                // Check if one of the trait obligations resolves to a clause
                                if generics_have_unresolved(generics) {
                                    log::trace!(
                                        "Found call with unresolvable generic parts: {name_str} (block: {id})"
                                    );
                                    taint_analyzer.mark_sink(id.into());
                                    self.status.unresolvable_generic_functions.push(st.span);
                                }

                                /*match Instance::resolve(
                                    self.rcx.tcx(),
                                    self.param_env,
                                    callee_did,
                                    callee_substs,
                                ) {
                                    Err(_e) => log_err!(ResolveError),
                                    Ok(Some(_)) => {
                                        // Calls were successfully resolved
                                    }
                                    Ok(None) => {
                                        // Call contains unresolvable generic parts
                                        // Here, we are making a two step approximation:
                                        // 1. Unresolvable generic code is potentially user-provided
                                        // 2. User-provided code potentially panics
                                        taint_analyzer.mark_sink(id.into());
                                        self.status
                                            .unresolvable_generic_functions
                                            .push(terminator.original.source_info.span);
                                    }
                                }*/
                            }
                        }
                        RawStatement::Call(Call {
                            func:
                                FnOperand::Regular(FnPtr {
                                    func: FunIdOrTraitMethodRef::Trait(tref, item_name, ..),
                                    generics,
                                }),
                            ..
                        }) => {
                            let is_impl_with_unresolved = match &tref.kind {
                                TraitRefKind::TraitImpl(_, impl_generics) => {
                                    generics_have_unresolved(impl_generics)
                                }
                                _ => true,
                            };
                            if is_impl_with_unresolved || generics_have_unresolved(generics) {
                                // Call contains unresolvable generic parts
                                // Here, we are making a two step approximation:
                                // 1. Unresolvable generic code is potentially user-provided
                                // 2. User-provided code potentially panics
                                log::trace!(
                                "Found unresolvable call to trait method: {item_name} (block: {id})"
                            );
                                taint_analyzer.mark_sink(id.into());
                                self.status.unresolvable_generic_functions.push(st.span);
                            }
                        }
                        _ => (),
                    }
                }
            }

            self.status.behavior_flag = taint_analyzer.propagate();
            self.status
        }

        fn fn_called_on_copy(
            &self,
            callee_did: FunDeclId,
            generics: &GenericArgs,
            paths: &PathSet,
        ) -> bool {
            if let Some(decl) = self.rcx.crate_data.fun_decls.get(callee_did) {
                if paths.contains(self.rcx, &decl.item_meta.name).is_some() {
                    // Just check the first type argument
                    return self.rcx.is_copyable(generics.types.get(0.into()).unwrap());
                }
            }
            false

            /*for path in paths.iter() {
                if ext.match_def_path(callee_did, path) {
                    for arg in callee_args.iter() {
                        if_chain! {
                            if let Operand::Move(place) = arg;
                            let place_ty = place.ty(self.body, tcx);
                            if let TyKind::RawPtr(ty_and_mut) = place_ty.ty.kind();
                            let pointed_ty = ty_and_mut.ty;
                            if pointed_ty.is_copy_modulo_regions(tcx.at(DUMMY_SP), self.param_env);
                            then {
                                return true;
                            }
                        }
                        // No need to inspect beyond first arg of the
                        // target bypass functions.
                        break;
                    }
                }
            }
            false*/
        }
    }

    fn trace_calls_in_body(rcx: RudraCtxt, body: &Body) {
        warn!("Paths discovery function has been detected");
        for block in &body.as_unstructured().unwrap().body {
            for st in &block.statements {
                match &st.content {
                    RawStatement::Call(Call {
                        func:
                            FnOperand::Regular(FnPtr {
                                func: FunIdOrTraitMethodRef::Fun(FunId::Regular(id)),
                                ..
                            }),
                        ..
                    }) => {
                        println!("{}", rcx.crate_data.into_fmt().format_object(*id));
                    }
                    _ => (),
                }
            }
        }
    }

    // Check if the argument of `Vec::set_len()` is 0_usize.
    fn vec_set_len_to_0(args: &[Operand]) -> bool {
        for arg in args.iter() {
            if let Operand::Const(x) = arg {
                if let RawConstantExpr::Literal(Literal::Scalar(ScalarValue::Usize(x))) = &x.value {
                    if *x == 0 {
                        // Leaking(`vec.set_len(0);`) is safe.
                        return true;
                    }
                }
            }
        }
        false
    }
}

// Unsafe Dataflow BypassKind.
// Used to associate each Unsafe-Dataflow bug report with its cause.
bitflags! {
    #[derive(Default, Debug, Clone, Copy)]
    pub struct BehaviorFlag: u16 {
        const READ_FLOW = 0b00000001;
        const COPY_FLOW = 0b00000010;
        const VEC_FROM_RAW = 0b00000100;
        const TRANSMUTE = 0b00001000;
        const WRITE_FLOW = 0b00010000;
        const PTR_AS_REF = 0b00100000;
        const SLICE_UNCHECKED = 0b01000000;
        const SLICE_FROM_RAW = 0b10000000;
        const VEC_SET_LEN = 0b100000000;
    }
}

impl IntoReportLevel for BehaviorFlag {
    fn report_level(&self) -> ReportLevel {
        use BehaviorFlag as Flag;

        let high = Flag::VEC_FROM_RAW | Flag::VEC_SET_LEN;
        let med = Flag::READ_FLOW | Flag::COPY_FLOW | Flag::WRITE_FLOW;

        if !(*self & high).is_empty() {
            ReportLevel::Error
        } else if !(*self & med).is_empty() {
            ReportLevel::Warning
        } else {
            ReportLevel::Info
        }
    }
}

impl GraphTaint for BehaviorFlag {
    fn is_empty(&self) -> bool {
        self.is_all()
    }

    fn contains(&self, taint: &Self) -> bool {
        self.contains(*taint)
    }

    fn join(&mut self, taint: &Self) {
        *self |= *taint;
    }
}

/// Return true if some trait refs are not resolved (they link to clauses)
fn generics_have_unresolved(generics: &GenericArgs) -> bool {
    generics
        .trait_refs
        .iter()
        .any(|tr| !matches!(&tr.kind, TraitRefKind::TraitImpl(..)))
}
