//! Detect potential overflow scenarios where the number of bits being shifted exceeds the bit width of
//! the variable being shifted, which could lead to unintended behavior or loss of data. If such a
//! potential overflow is detected, a warning is generated to alert the developer.
use crate::{
    diag,
    diagnostics::{
        codes::{custom, DiagnosticInfo, Severity},
        WarningFilters,
    },
    expansion::ast::Value_,
    naming::ast::{BuiltinTypeName_, TypeName_, Type_},
    parser::ast::BinOp_,
    shared::{program_info::TypingProgramInfo, CompilationEnv},
    sui_mode::linters::{LinterDiagCategory, LINTER_DEFAULT_DIAG_CODE, LINT_WARNING_PREFIX},
    typing::{
        ast::{self as T, UnannotatedExp_},
        visitor::{TypingVisitorConstructor, TypingVisitorContext},
    },
};
use move_ir_types::location::Loc;
use std::str::FromStr;

const REDUNDANT_DEREF_REF_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    LinterDiagCategory::RedundantDerefRef as u8,
    LINTER_DEFAULT_DIAG_CODE,
    "",
);

pub struct RedundantDerefRefVisitor;

pub struct Context<'a> {
    env: &'a mut CompilationEnv,
}

impl TypingVisitorConstructor for RedundantDerefRefVisitor {
    type Context<'a> = Context<'a>;

    fn context<'a>(
        env: &'a mut CompilationEnv,
        _program_info: &'a TypingProgramInfo,
        _program: &T::Program_,
    ) -> Self::Context<'a> {
        Context { env }
    }
}

impl TypingVisitorContext for Context<'_> {
    fn visit_exp_custom(&mut self, exp: &mut T::Exp) -> bool {
        if let UnannotatedExp_::Dereference(deref_exp) = &exp.exp.value {
            if let UnannotatedExp_::Borrow(_, _, _) = &deref_exp.exp.value {
                report_deref_ref(self.env, exp.exp.loc);
            }
        }
        false
    }
    fn add_warning_filter_scope(&mut self, filter: WarningFilters) {
        self.env.add_warning_filter_scope(filter)
    }

    fn pop_warning_filter_scope(&mut self) {
        self.env.pop_warning_filter_scope()
    }
}

fn report_deref_ref(env: &mut CompilationEnv, loc: Loc) {
    let diag = diag!(
       REDUNDANT_DEREF_REF_DIAG,
        (loc, "Redundant dereference of a reference detected (`*&` or `*&mut`). Consider simplifying the expression.")
    );
    env.add_diag(diag);
}
