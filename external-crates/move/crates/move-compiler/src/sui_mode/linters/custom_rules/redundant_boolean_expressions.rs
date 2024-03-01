use crate::{
    diag,
    diagnostics::{
        codes::{custom, DiagnosticInfo, Severity},
        WarningFilters,
    },
    expansion::ast::Value_,
    naming::ast::Var_,
    parser::ast::BinOp_,
    shared::{program_info::TypingProgramInfo, CompilationEnv},
    sui_mode::linters::{LinterDiagCategory, LINTER_DEFAULT_DIAG_CODE, LINT_WARNING_PREFIX},
    typing::{
        ast::{self as T, UnannotatedExp_},
        visitor::{TypingVisitorConstructor, TypingVisitorContext},
    },
};
use move_ir_types::location::Loc;
use move_symbol_pool::Symbol;

const REDUNDANT_BOOLEAN_EXP_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    LinterDiagCategory::RedundantBooleanExp as u8,
    LINTER_DEFAULT_DIAG_CODE,
    "",
);

pub struct RedundantBooleanExp;

pub struct Context<'a> {
    env: &'a mut CompilationEnv,
}

impl TypingVisitorConstructor for RedundantBooleanExp {
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
        if let UnannotatedExp_::BinopExp(e1, op, _, e2) = &exp.exp.value {
            // Check if the operation is a logical OR
            if let BinOp_::Or = &op.value {
                match (&e1.exp.value, &e2.exp.value) {
                    (UnannotatedExp_::Value(bool), UnannotatedExp_::Copy { var, .. })
                    | (UnannotatedExp_::Copy { var, .. }, UnannotatedExp_::Value(bool)) => {
                        if &Value_::Bool(true) == &bool.value {
                            add_redundant_bool_expr_diag(self.env,exp.exp.loc, "true", "This expression always evaluates to true regardless of the other operand.");
                            return true;
                        }
                        let Var_ { name, .. } = var.value;
                        add_redundant_bool_expr_diag(self.env,exp.exp.loc, name.as_str(), "This expression always evaluates to true regardless of the other operand.");
                    }
                    _ => {}
                }
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

fn add_redundant_bool_expr_diag(
    env: &mut CompilationEnv,
    loc: Loc,
    simplified: &str,
    message: &str,
) {
    let d = diag!(
        REDUNDANT_BOOLEAN_EXP_DIAG,
        (
            loc,
            format!("{}.Consider refactor it to {}", message, simplified)
        )
    );
    env.add_diag(d);
}
