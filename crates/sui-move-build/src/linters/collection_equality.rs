// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

//! This analysis flags situations when instances of a sui::table::Table or sui::table_vec::TableVec
//! or sui::bag::Bag are being compared for (in)equality at this type of comparison is not very
//! useful and DOES NOT take into consideration structural (in)equality.

use move_compiler::{
    diag,
    diagnostics::codes::{custom, DiagnosticInfo, Severity},
    naming::ast as N,
    parser::ast as P,
    shared::{CompilationEnv, Identifier},
    typing::{ast as T, core::ProgramInfo, visitor::TypingVisitor},
};

use super::{
    base_type, LinterDiagCategory, BAG_MOD_NAME, BAG_STRUCT_NAME, LINTER_DEFAULT_DIAG_CODE,
    LINT_WARNING_PREFIX, SUI_PKG_NAME, TABLE_MOD_NAME, TABLE_STRUCT_NAME, TABLE_VEC_MOD_NAME,
    TABLE_VEC_STRUCT_NAME,
};

const COLLECTIONS_EQUALITY_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    LinterDiagCategory::CollectionEquality as u8,
    LINTER_DEFAULT_DIAG_CODE,
    "possibly useless collections compare",
);

const COLLECTION_TYPES: &[(&str, &str, &str)] = &[
    (SUI_PKG_NAME, BAG_MOD_NAME, BAG_STRUCT_NAME),
    (SUI_PKG_NAME, TABLE_MOD_NAME, TABLE_STRUCT_NAME),
    (SUI_PKG_NAME, TABLE_VEC_MOD_NAME, TABLE_VEC_STRUCT_NAME),
];

pub struct CollectionEqualityVisitor;

impl TypingVisitor for CollectionEqualityVisitor {
    fn visit_exp_custom(
        &mut self,
        exp: &T::Exp,
        env: &mut CompilationEnv,
        _program_info: &ProgramInfo,
        _program: &T::Program,
    ) -> bool {
        use T::UnannotatedExp_ as E;
        if let E::BinopExp(_, op, t, _) = &exp.exp.value {
            if op.value != P::BinOp_::Eq && op.value != P::BinOp_::Neq {
                // not a comparison
                return false;
            }
            let Some(bt) = base_type(t) else {
                return false;
            };
            let N::Type_::Apply(_, tname, _) = &bt.value else {
                return false;
            };
            let N::TypeName_::ModuleType(mident, sname) = tname.value else {
                return false;
            };

            if let Some((caddr, cmodule, cname)) =
                COLLECTION_TYPES.iter().find(|(caddr, cmodule, cname)| {
                    mident.value.is(*caddr, *cmodule) && sname.value().as_str() == *cname
                })
            {
                let msg = format!(
                    "Comparing collections of type '{caddr}::{cmodule}::{cname}' may yield unexpected result."
                );
                let note_msg =
                    "Equality for collections IS NOT a structural check based on content";
                let mut d = diag!(COLLECTIONS_EQUALITY_DIAG, (op.loc, msg),);
                d.add_note(note_msg);
                env.add_diag(d);
                return true;
            }
        }
        false
    }
}
