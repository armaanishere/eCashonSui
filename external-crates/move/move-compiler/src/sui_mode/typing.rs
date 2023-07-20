// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use move_ir_types::location::Loc;
use move_symbol_pool::Symbol;

use crate::{
    diag,
    editions::Flavor,
    expansion::ast::{AbilitySet, Fields, ModuleIdent, Visibility},
    naming::ast::{
        self as N, BuiltinTypeName_, FunctionSignature, TParam, Type, TypeName_, Type_, Var,
    },
    parser::ast::{Ability_, FunctionName, StructName},
    shared::{CompilationEnv, Identifier},
    sui_mode::{
        ASCII_MODULE_NAME, ASCII_TYPE_NAME, CLOCK_MODULE_NAME, CLOCK_TYPE_NAME,
        ENTRY_FUN_SIGNATURE_DIAG, ID_TYPE_NAME, INIT_CALL_DIAG, INIT_FUN_DIAG, OBJECT_MODULE_NAME,
        OPTION_MODULE_NAME, OPTION_TYPE_NAME, OTW_DECL_DIAG, OTW_USAGE_DIAG, SCRIPT_DIAG,
        STD_ADDR_NAME, SUI_ADDR_NAME, UTF_MODULE_NAME, UTF_TYPE_NAME,
    },
    typing::{
        ast as T,
        core::{ability_not_satisfied_tips, error_format, error_format_, ProgramInfo, Subst},
        visitor::TypingVisitor,
    },
};

use super::{TX_CONTEXT_MODULE_NAME, TX_CONTEXT_TYPE_NAME};

//**************************************************************************************************
// Visitor
//**************************************************************************************************

pub struct SuiTypeChecks;

impl TypingVisitor for SuiTypeChecks {
    fn visit(&mut self, env: &mut CompilationEnv, info: &ProgramInfo, prog: &mut T::Program) {
        program(env, info, prog)
    }
}

//**************************************************************************************************
// Context
//**************************************************************************************************

#[allow(unused)]
struct Context<'a> {
    env: &'a mut CompilationEnv,
    info: &'a ProgramInfo,
    current_module: ModuleIdent,
    upper_module: Symbol,
    one_time_witness: Option<Result<StructName, ()>>,
    in_test: bool,
}

impl<'a> Context<'a> {
    fn new(
        env: &'a mut CompilationEnv,
        info: &'a ProgramInfo,
        current_module: ModuleIdent,
    ) -> Self {
        let upper_module: Symbol =
            Symbol::from(current_module.value.module.0.value.as_str().to_uppercase());
        Context {
            env,
            current_module,
            upper_module,
            one_time_witness: None,
            info,
            in_test: false,
        }
    }
}

const OTW_NOTE: &str = "One-time witness types are structs with the following requirements: \
                        their name is the upper-case version of the module's name, \
                        they have no fields (or a single boolean field), \
                        they have no type parameters, \
                        and they have only the 'drop' ability.";

//**************************************************************************************************
// Entry
//**************************************************************************************************

pub fn program(env: &mut CompilationEnv, info: &ProgramInfo, prog: &T::Program) {
    let T::Program { modules, scripts } = prog;
    for script in scripts.values() {
        let config = env.package_config(script.package_name);
        if config.flavor != Flavor::Sui {
            continue;
        }

        // TODO point to PTB docs?
        let msg = "'scripts' are not supported on Sui. \
        Consider removing or refactoring into a 'module'";
        env.add_diag(diag!(SCRIPT_DIAG, (script.loc, msg)))
    }
    for (mident, mdef) in modules.key_cloned_iter() {
        module(env, info, mident, mdef);
    }
}

fn module(
    env: &mut CompilationEnv,
    info: &ProgramInfo,
    mident: ModuleIdent,
    mdef: &T::ModuleDefinition,
) {
    let config = env.package_config(mdef.package_name);
    if config.flavor != Flavor::Sui {
        return;
    }

    // Skip non-source, dependency modules
    if !mdef.is_source_module {
        return;
    }

    let mut context = Context::new(env, info, mident);
    if let Some(sdef) = mdef.structs.get_(&context.upper_module) {
        let valid_fields = if let N::StructFields::Defined(fields) = &sdef.fields {
            invalid_otw_field_loc(fields).is_none()
        } else {
            true
        };
        if valid_fields {
            let name = mdef.structs.get_full_key_(&context.upper_module).unwrap();
            check_otw_type(&mut context, name, sdef)
        }
    }

    if let Some(fdef) = mdef.functions.get_(&symbol!("init")) {
        let name = mdef.functions.get_full_key_(&symbol!("init")).unwrap();
        init_signature(&mut context, name, &fdef.signature)
    }

    for (name, fdef) in mdef.functions.key_cloned_iter() {
        function(&mut context, name, fdef);
    }
}

//**************************************************************************************************
// Functions
//**************************************************************************************************

fn function(context: &mut Context, name: FunctionName, fdef: &T::Function) {
    let T::Function {
        visibility,
        signature,
        acquires: _,
        body,
        warning_filter: _,
        index: _,
        attributes: _,
        entry,
    } = fdef;
    if name.0.value == symbol!("init") {
        init_visibility(context, name, *visibility, *entry);
    }
    if let Some(entry_loc) = entry {
        entry_signature(context, *entry_loc, name, signature);
    }
    if let sp!(_, T::FunctionBody_::Defined(seq)) = body {
        sequence(context, seq)
    }
}

//**************************************************************************************************
// init
//**************************************************************************************************

fn init_visibility(
    context: &mut Context,
    name: FunctionName,
    visibility: Visibility,
    entry: Option<Loc>,
) {
    match visibility {
        Visibility::Public(loc) | Visibility::Friend(loc) => context.env.add_diag(diag!(
            INIT_FUN_DIAG,
            (name.loc(), "Invalid 'init' function declaration"),
            (loc, "'init' functions must be internal to their module"),
        )),
        Visibility::Internal => (),
    }
    if let Some(entry) = entry {
        context.env.add_diag(diag!(
            INIT_FUN_DIAG,
            (name.loc(), "Invalid 'init' function declaration"),
            (entry, "'init' functions cannot be 'entry' functions"),
        ));
    }
}

fn init_signature(context: &mut Context, name: FunctionName, signature: &FunctionSignature) {
    let FunctionSignature {
        type_parameters,
        parameters,
        return_type,
    } = signature;
    if !type_parameters.is_empty() {
        let tp_loc = type_parameters[0].user_specified_name.loc;
        context.env.add_diag(diag!(
            INIT_FUN_DIAG,
            (name.loc(), "Invalid 'init' function declaration"),
            (tp_loc, "'init' functions cannot have type parameters"),
        ));
    }
    if !matches!(return_type, sp!(_, Type_::Unit)) {
        let msg = format!(
            "'init' functions must have a return type of {}",
            error_format_(&Type_::Unit, &Subst::empty())
        );
        context.env.add_diag(diag!(
            INIT_FUN_DIAG,
            (name.loc(), "Invalid 'init' function declaration"),
            (return_type.loc, msg),
        ))
    }
    let last_loc = parameters
        .last()
        .map(|(_, sp!(loc, _))| *loc)
        .unwrap_or(name.loc());
    let tx_ctx_kind = parameters
        .last()
        .map(|(_, last_param_ty)| tx_context_kind(last_param_ty))
        .unwrap_or(TxContextKind::None);
    if tx_ctx_kind == TxContextKind::None {
        let msg = format!(
            "'init' functions must have their last parameter as \
            '&{a}::{m}::{t}' or '&mut {a}::{m}::{t}'",
            a = SUI_ADDR_NAME,
            m = TX_CONTEXT_MODULE_NAME,
            t = TX_CONTEXT_TYPE_NAME,
        );
        context.env.add_diag(diag!(
            INIT_FUN_DIAG,
            (name.loc(), "Invalid 'init' function declaration"),
            (last_loc, msg),
        ))
    }
    let upper_module: Symbol = Symbol::from(
        context
            .current_module
            .value
            .module
            .0
            .value
            .as_str()
            .to_uppercase(),
    );

    if parameters.len() == 1 && context.one_time_witness.is_some() {
        // if there is 1 parameter, and a OTW, this is an error since the OTW must be used
        let msg = format!(
            "Invalid first parameter to 'init'. \
            Expected this module's one-time witness type '{}::{upper_module}'",
            context.current_module,
        );
        let otw_loc = context
            .info
            .struct_declared_loc_(&context.current_module, &upper_module);
        let otw_msg = "One-time witness declared here";
        let mut diag = diag!(
            INIT_FUN_DIAG,
            (parameters[0].1.loc, msg),
            (otw_loc, otw_msg),
        );
        diag.add_note(OTW_NOTE);
        context.env.add_diag(diag)
    } else if parameters.len() > 1 {
        // if there is more than one parameter, the first must be the OTW
        let (first_var, first_ty) = parameters.first().unwrap();
        let is_otw = matches!(
            first_ty.value.type_name(),
            Some(sp!(_, TypeName_::ModuleType(m, n)))
                if m == &context.current_module && n.value() == upper_module
        );
        if !is_otw {
            let msg = format!(
                "Invalid parameter '{}' of type {}. \
                Expected a one-time witness type, '{}::{upper_module}",
                first_var.value.name,
                error_format(first_ty, &Subst::empty()),
                context.current_module,
            );
            let mut diag = diag!(
                INIT_FUN_DIAG,
                (name.loc(), "Invalid 'init' function declaration"),
                (first_ty.loc, msg)
            );
            diag.add_note(OTW_NOTE);
            context.env.add_diag(diag)
        } else if let Some(sdef) = context
            .info
            .module(&context.current_module)
            .structs
            .get_(&upper_module)
        {
            let name = context
                .info
                .module(&context.current_module)
                .structs
                .get_full_key_(&upper_module)
                .unwrap();
            check_otw_type(context, name, sdef)
        }
    } else if parameters.len() > 2 {
        // no init function can take more than 2 parameters (the OTW and the TxContext)
        let (second_var, _) = &parameters[1];
        context.env.add_diag(diag!(
            INIT_FUN_DIAG,
            (name.loc(), "Invalid 'init' function declaration"),
            (
                second_var.loc,
                "'init' functions can have at most two parameters"
            ),
        ));
    }
}

// While theoretically we could call this just once for the upper cased module struct, we break it
// out into a separate function to help programmers understand the rules for one-time witness types,
// when trying to write an 'init' function.
fn check_otw_type(context: &mut Context, name: StructName, sdef: &N::StructDefinition) {
    if context.one_time_witness.is_some() {
        return;
    }

    let mut valid = true;
    if let Some(tp) = sdef.type_parameters.first() {
        let msg = "One-time witness types cannot have type parameters";
        let mut diag = diag!(
            OTW_DECL_DIAG,
            (name.loc(), "Invalid one-time witness declaration"),
            (tp.param.user_specified_name.loc, msg),
        );
        diag.add_note(OTW_NOTE);
        context.env.add_diag(diag);
        valid = false
    }

    if let N::StructFields::Defined(fields) = &sdef.fields {
        if let Some(invalid_field_loc) = invalid_otw_field_loc(fields) {
            let msg = format!(
                "One-time witness types must have no fields, \
                or exactly one field of type {}",
                error_format(&Type_::bool(name.loc()), &Subst::empty())
            );
            let mut diag = diag!(
                OTW_DECL_DIAG,
                (name.loc(), "Invalid one-time witness declaration"),
                (invalid_field_loc, msg),
            );
            diag.add_note(OTW_NOTE);
            context.env.add_diag(diag);
            valid = false
        }
    }

    let invalid_ability_loc =
        if !sdef.abilities.has_ability_(Ability_::Drop) || sdef.abilities.len() > 1 {
            let loc = sdef
                .abilities
                .iter()
                .find_map(|a| {
                    if a.value != Ability_::Drop {
                        Some(a.loc)
                    } else {
                        None
                    }
                })
                .unwrap_or(name.loc());
            Some(loc)
        } else {
            None
        };
    if let Some(loc) = invalid_ability_loc {
        let msg = format!(
            "One-time witness types can only have the have the '{}' ability",
            Ability_::Drop
        );
        let mut diag = diag!(
            OTW_DECL_DIAG,
            (name.loc(), "Invalid one-time witness declaration"),
            (loc, msg),
        );
        diag.add_note(OTW_NOTE);
        context.env.add_diag(diag);
        valid = false
    }

    context.one_time_witness = Some(if valid { Ok(name) } else { Err(()) })
}

// Find the first invalid field in a one-time witness type, if any.
// First looks for a non-boolean field, otherwise looks for any field after the first.
fn invalid_otw_field_loc(fields: &Fields<Type>) -> Option<Loc> {
    fields
        .iter()
        .find_map(|(loc, _, (idx, ty))| {
            if (*idx == 0) && ty.value.builtin_name()?.value != BuiltinTypeName_::Bool {
                Some(loc)
            } else {
                None
            }
        })
        .or_else(|| {
            fields
                .iter()
                .find(|(_, _, (idx, _))| *idx > 0)
                .map(|(loc, _, _)| loc)
        })
}

//**************************************************************************************************
// entry types
//**************************************************************************************************

fn entry_signature(
    context: &mut Context,
    entry_loc: Loc,
    name: FunctionName,
    signature: &FunctionSignature,
) {
    let FunctionSignature {
        type_parameters: _,
        parameters,
        return_type,
    } = signature;
    let all_non_ctx_parameters = match parameters.last() {
        Some((_, last_param_ty)) if tx_context_kind(last_param_ty) != TxContextKind::None => {
            &parameters[0..parameters.len() - 1]
        }
        _ => &parameters,
    };
    entry_param(context, entry_loc, name, all_non_ctx_parameters);
    entry_return(context, entry_loc, name, return_type);
}

fn tx_context_kind(sp!(_, last_param_ty_): &Type) -> TxContextKind {
    let Type_::Ref(is_mut, inner_ty) = last_param_ty_ else {
        return TxContextKind::None
    };
    let Type_::Apply(_, sp!(_, inner_name), _) = &inner_ty.value else {
        return TxContextKind::None
    };
    if inner_name.is(SUI_ADDR_NAME, TX_CONTEXT_MODULE_NAME, TX_CONTEXT_TYPE_NAME) {
        if *is_mut {
            TxContextKind::Mutable
        } else {
            TxContextKind::Immutable
        }
    } else {
        TxContextKind::None
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum TxContextKind {
    // No TxContext
    None,
    // &mut TxContext
    Mutable,
    // &TxContext
    Immutable,
}

fn entry_param(
    context: &mut Context,
    entry_loc: Loc,
    name: FunctionName,
    parameters: &[(Var, Type)],
) {
    for (var, ty) in parameters {
        entry_param_ty(context, entry_loc, name, var, ty);
    }
}

/// A valid entry param type is
/// - A primitive (including strings, ID, and object)
/// - A vector of primitives (including nested vectors)
///
/// - An object
/// - A reference to an object
/// - A vector of objects
fn entry_param_ty(
    context: &mut Context,
    entry_loc: Loc,
    name: FunctionName,
    param: &Var,
    param_ty: &Type,
) {
    let is_mut_clock = is_mut_clock(param_ty);
    // TODO better error message for cases such as `MyObject<InnerTypeWithoutStore>`
    // which should give a contextual error about `MyObject` having `key`, but the instantiation
    // `MyObject<InnerTypeWithoutStore>` not having `key` due to `InnerTypeWithoutStore` not having
    // `store`
    let is_valid = is_entry_primitive_ty(param_ty) || is_entry_object_ty(param_ty);
    if is_mut_clock || !is_valid {
        let pmsg = format!(
            "Invalid 'entry' parameter type for parameter '{}'",
            param.value.name
        );
        let tmsg = if is_mut_clock {
            format!(
                "{a}::{m}::{n} must be passed by immutable reference, e.g. '&{a}::{m}::{n}'",
                a = SUI_ADDR_NAME,
                m = CLOCK_MODULE_NAME,
                n = CLOCK_TYPE_NAME,
            )
        } else {
            "'entry' parameters must be primitives (by-value), vectors of primitives, objects \
            (by-reference or by-value), or vectors of objects"
                .to_owned()
        };
        let emsg = format!("'{name}' was declared 'entry' here");
        context.env.add_diag(diag!(
            ENTRY_FUN_SIGNATURE_DIAG,
            (param.loc, pmsg),
            (param_ty.loc, tmsg),
            (entry_loc, emsg)
        ));
    }
}

fn is_mut_clock(param_ty: &Type) -> bool {
    match &param_ty.value {
        Type_::Ref(/* mut */ false, _) => false,
        Type_::Ref(/* mut */ true, t) => is_mut_clock(t),
        Type_::Apply(_, sp!(_, n_), _) => n_.is(SUI_ADDR_NAME, CLOCK_MODULE_NAME, CLOCK_TYPE_NAME),
        Type_::Unit
        | Type_::Param(_)
        | Type_::Var(_)
        | Type_::Anything
        | Type_::UnresolvedError => false,
    }
}

fn is_entry_primitive_ty(param_ty: &Type) -> bool {
    use BuiltinTypeName_ as B;
    use TypeName_ as N;

    match &param_ty.value {
        // A bit of a hack since no primitive has key
        Type_::Param(tp) => !tp.abilities.has_ability_(Ability_::Key),
        // nonsensical, but no error needed
        Type_::Apply(_, sp!(_, N::Multiple(_)), ts) => ts.iter().all(is_entry_primitive_ty),
        // Simple recursive cases
        Type_::Ref(_, t) => is_entry_primitive_ty(t),
        Type_::Apply(_, sp!(_, N::Builtin(sp!(_, B::Vector))), targs) => {
            debug_assert!(targs.len() == 1);
            is_entry_primitive_ty(&targs[0])
        }

        // custom "primitives"
        Type_::Apply(_, sp!(_, n), targs)
            if n.is(STD_ADDR_NAME, ASCII_MODULE_NAME, ASCII_TYPE_NAME)
                || n.is(STD_ADDR_NAME, UTF_MODULE_NAME, UTF_TYPE_NAME)
                || n.is(SUI_ADDR_NAME, OBJECT_MODULE_NAME, ID_TYPE_NAME) =>
        {
            debug_assert!(targs.is_empty());
            true
        }
        Type_::Apply(_, sp!(_, n), targs)
            if n.is(STD_ADDR_NAME, OPTION_MODULE_NAME, OPTION_TYPE_NAME) =>
        {
            debug_assert!(targs.len() == 1);
            is_entry_primitive_ty(&targs[0])
        }

        // primitives
        Type_::Apply(_, sp!(_, N::Builtin(_)), targs) => {
            debug_assert!(targs.is_empty());
            true
        }

        // Non primitive
        Type_::Apply(_, sp!(_, N::ModuleType(_, _)), _) => false,
        Type_::Unit => false,

        // Error case nothing to do
        Type_::UnresolvedError | Type_::Anything | Type_::Var(_) => true,
    }
}

fn is_entry_object_ty(param_ty: &Type) -> bool {
    use BuiltinTypeName_ as B;
    use TypeName_ as N;
    match &param_ty.value {
        Type_::Ref(_, t) => is_entry_object_ty_inner(t),
        Type_::Apply(_, sp!(_, N::Builtin(sp!(_, B::Vector))), targs) => {
            debug_assert!(targs.len() == 1);
            is_entry_object_ty_inner(&targs[0])
        }
        _ => is_entry_object_ty_inner(param_ty),
    }
}

fn is_entry_object_ty_inner(param_ty: &Type) -> bool {
    use TypeName_ as N;
    match &param_ty.value {
        Type_::Param(tp) => tp.abilities.has_ability_(Ability_::Key),
        // nonsensical, but no error needed
        Type_::Apply(_, sp!(_, N::Multiple(_)), ts) => ts.iter().all(is_entry_object_ty_inner),
        // Simple recursive cases, shouldn't be hit but no need to error
        Type_::Ref(_, t) => is_entry_object_ty_inner(t),

        // Objects
        Type_::Apply(Some(abilities), _, _) => abilities.has_ability_(Ability_::Key),

        // Error case nothing to do
        Type_::UnresolvedError | Type_::Anything | Type_::Var(_) | Type_::Unit => true,
        // Unreachable cases
        Type_::Apply(None, _, _) => unreachable!("ICE abilities should have been expanded"),
    }
}

fn entry_return(
    context: &mut Context,
    entry_loc: Loc,
    name: FunctionName,
    return_type @ sp!(tloc, return_type_): &Type,
) {
    match return_type_ {
        // unit is fine, nothing to do
        Type_::Unit => (),
        Type_::Ref(_, _) => {
            let fmsg = format!("Invalid return type for entry function '{}'", name);
            let tmsg = "Expected a non-reference type";
            context.env.add_diag(diag!(
                ENTRY_FUN_SIGNATURE_DIAG,
                (entry_loc, fmsg),
                (*tloc, tmsg)
            ))
        }
        Type_::Param(tp) => {
            if !tp.abilities.has_ability_(Ability_::Drop) {
                let declared_loc_opt = Some(tp.user_specified_name.loc);
                let declared_abilities = tp.abilities.clone();
                invalid_entry_return_ty(
                    context,
                    entry_loc,
                    name,
                    return_type,
                    declared_loc_opt,
                    &declared_abilities,
                    std::iter::empty(),
                )
            }
        }
        Type_::Apply(Some(abilities), sp!(_, tn_), ty_args) => {
            if !abilities.has_ability_(Ability_::Drop) {
                let (declared_loc_opt, declared_abilities) = match tn_ {
                    TypeName_::Multiple(_) => (None, AbilitySet::collection(*tloc)),
                    TypeName_::ModuleType(m, n) => (
                        Some(context.info.struct_declared_loc(m, n)),
                        context.info.struct_declared_abilities(m, n).clone(),
                    ),
                    TypeName_::Builtin(b) => (None, b.value.declared_abilities(b.loc)),
                };
                invalid_entry_return_ty(
                    context,
                    entry_loc,
                    name,
                    return_type,
                    declared_loc_opt,
                    &declared_abilities,
                    ty_args.iter().map(|ty_arg| (ty_arg, get_abilities(ty_arg))),
                )
            }
        }
        // Error case nothing to do
        Type_::UnresolvedError | Type_::Anything | Type_::Var(_) => (),
        // Unreachable cases
        Type_::Apply(None, _, _) => unreachable!("ICE abilities should have been expanded"),
    }
}

fn get_abilities(sp!(loc, ty_): &Type) -> AbilitySet {
    use Type_ as T;
    let loc = *loc;
    match ty_ {
        T::UnresolvedError | T::Anything => AbilitySet::all(loc),
        T::Unit => AbilitySet::collection(loc),
        T::Ref(_, _) => AbilitySet::references(loc),
        T::Param(TParam { abilities, .. }) | Type_::Apply(Some(abilities), _, _) => {
            abilities.clone()
        }
        T::Var(_) | Type_::Apply(None, _, _) => {
            unreachable!("ICE abilities should have been expanded")
        }
    }
}

fn invalid_entry_return_ty<'a>(
    context: &mut Context,
    entry_loc: Loc,
    name: FunctionName,
    ty: &Type,
    declared_loc_opt: Option<Loc>,
    declared_abilities: &AbilitySet,
    ty_args: impl IntoIterator<Item = (&'a Type, AbilitySet)>,
) {
    let fmsg = format!("Invalid return type for entry function '{}'", name);
    let mut diag = diag!(ENTRY_FUN_SIGNATURE_DIAG, (entry_loc, fmsg));
    ability_not_satisfied_tips(
        &Subst::empty(),
        &mut diag,
        Ability_::Drop,
        ty,
        declared_loc_opt,
        declared_abilities,
        ty_args,
    );
    context.env.add_diag(diag)
}

//**************************************************************************************************
// Expr
//**************************************************************************************************

fn sequence(context: &mut Context, seq: &T::Sequence) {
    for item in seq {
        sequence_item(context, item)
    }
}

fn sequence_item(context: &mut Context, sp!(_, item_): &T::SequenceItem) {
    match item_ {
        T::SequenceItem_::Seq(e) => exp(context, e),
        T::SequenceItem_::Declare(_) => (),
        T::SequenceItem_::Bind(_, _, e) => exp(context, e),
    }
}

fn exp(context: &mut Context, e: &T::Exp) {
    match &e.exp.value {
        T::UnannotatedExp_::Unit { .. }
        | T::UnannotatedExp_::Value(_)
        | T::UnannotatedExp_::Move { .. }
        | T::UnannotatedExp_::Copy { .. }
        | T::UnannotatedExp_::Use(_)
        | T::UnannotatedExp_::Constant(_, _)
        | T::UnannotatedExp_::Break
        | T::UnannotatedExp_::Continue
        | T::UnannotatedExp_::BorrowLocal(_, _)
        | T::UnannotatedExp_::Spec(_, _)
        | T::UnannotatedExp_::UnresolvedError => (),
        T::UnannotatedExp_::ModuleCall(mcall) => {
            let T::ModuleCall {
                module,
                name,
                arguments,
                ..
            } = &**mcall;
            if name.value() == symbol!("init") {
                let msg = format!(
                    "Invalid call to '{}::{}'. \
                    Module initializers cannot be called directly",
                    module, name
                );
                let mut diag = diag!(INIT_CALL_DIAG, (e.exp.loc, msg));
                diag.add_note(
                    "Module initializers are called implicitly upon publishing. \
                    If you need to reuse this function (or want to call it from a test), \
                    consider extracting the logic into a new function and \
                    calling that instead.",
                );
                context.env.add_diag(diag)
            }
            exp(context, arguments)
        }

        T::UnannotatedExp_::TempBorrow(_, e)
        | T::UnannotatedExp_::Builtin(_, e)
        | T::UnannotatedExp_::Vector(_, _, _, e)
        | T::UnannotatedExp_::Loop { body: e, .. }
        | T::UnannotatedExp_::Assign(_, _, e)
        | T::UnannotatedExp_::Return(e)
        | T::UnannotatedExp_::Abort(e)
        | T::UnannotatedExp_::Dereference(e)
        | T::UnannotatedExp_::UnaryExp(_, e)
        | T::UnannotatedExp_::Borrow(_, e, _)
        | T::UnannotatedExp_::Cast(e, _)
        | T::UnannotatedExp_::Annotate(e, _) => exp(context, e),
        T::UnannotatedExp_::BinopExp(el, _, _, er) | T::UnannotatedExp_::Mutate(el, er) => {
            exp(context, el);
            exp(context, er)
        }
        T::UnannotatedExp_::IfElse(econd, etrue, efalse) => {
            exp(context, econd);
            exp(context, etrue);
            exp(context, efalse)
        }
        T::UnannotatedExp_::While(econd, ebody) => {
            exp(context, econd);
            exp(context, ebody)
        }
        T::UnannotatedExp_::Block(seq) => sequence(context, seq),
        T::UnannotatedExp_::ExpList(es) => exp_list(context, es),

        T::UnannotatedExp_::Pack(m, s, _, fields) => {
            if !context.in_test
                && context.one_time_witness.as_ref().is_some_and(|otw| {
                    otw.as_ref()
                        .is_ok_and(|o| m == &context.current_module && o == s)
                })
            {
                let msg = "Invalid one-time witness construction. One-time witness types \
                    cannot be created manually, but are passed as an argument 'init'";
                let mut diag = diag!(OTW_USAGE_DIAG, (e.exp.loc, msg));
                diag.add_note(OTW_NOTE);
                context.env.add_diag(diag)
            }
            for (_, _, (_, (_, e))) in fields {
                exp(context, e)
            }
        }
    }
}

fn exp_list(context: &mut Context, es: &[T::ExpListItem]) {
    for item in es {
        match item {
            T::ExpListItem::Single(e, _) | T::ExpListItem::Splat(_, e, _) => exp(context, e),
        }
    }
}
