// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, fmt::Debug};

use crate::{
    cfgir::{
        absint::{AbstractDomain, AbstractInterpreter, JoinResult, TransferFunctions},
        cfg::BlockCFG,
        CFGContext,
    },
    command_line::compiler::Visitor,
    diagnostics::{Diagnostic, Diagnostics},
    hlir::ast::{
        Command, Command_, Exp, ExpListItem, LValue, LValue_, Label, ModuleCall, Type, Type_,
        UnannotatedExp_, Var,
    },
};
use move_ir_types::location::*;

pub type AbsIntVisitorFn = Box<dyn FnMut(&CFGContext, &BlockCFG) -> Diagnostics>;

/// A trait for custom abstract interpreter visitors. Use `SimpleAbsIntVisitor` if extensive custom
/// logic is not needed. For example, if just visiting specific function calls,
/// `SimpleAbsIntVisitor` should be sufficient.
pub trait AbsIntVisitor: AbstractInterpreter + Sized {
    /// Called to initialize the abstract state before any code is visited
    fn init_state(context: &CFGContext) -> <Self as TransferFunctions>::State;

    /// Construct an abstract interpreter pass given the initial state
    fn new(context: &CFGContext, init_state: &mut <Self as TransferFunctions>::State) -> Self;

    /// A hook for an additional processing after visiting all codes. The `final_states` are the
    /// pre-states for each block (keyed by the label for the block). The `diags` are collected from
    /// all code visited
    fn finish(
        &mut self,
        final_states: BTreeMap<Label, <Self as TransferFunctions>::State>,
        diags: Diagnostics,
    ) -> Diagnostics;

    fn visitor() -> Visitor {
        Visitor::AbsIntVisitor(Box::new(|context, cfg| {
            let mut init_state = Self::init_state(context);
            let mut ai = Self::new(context, &mut init_state);
            let (final_state, ds) = ai.analyze_function(cfg, init_state);
            ai.finish(final_state, ds)
        }))
    }
}

//**************************************************************************************************
// simple visitor
//**************************************************************************************************

/// The reason why a local variable is unavailable (mostly useful for error messages)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnavailableReason {
    Unassigned,
    Moved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// The state of a local variable, with its abstract value if it has one.
pub enum LocalState<V: Clone + Debug + Default> {
    Unavailable(Loc, UnavailableReason),
    Available(Loc, V),
    MaybeUnavailable {
        available: Loc,
        unavailable: Loc,
        unavailable_reason: UnavailableReason,
    },
}

/// A trait for a the context when visiting a `Command` in a block. At a minimum it must hold the diagnostics
/// and the abstract state
pub trait SimpleExecutionContext {
    /// Add a diagnostic
    fn add_diag(&mut self, diag: Diagnostic);
}

/// The domain used for the simple abstract interpreter template. Accessors for the local variables
/// must be provided, but it will manage the joining of the locals (given a way to join values).
pub trait SimpleDomain: AbstractDomain {
    /// The non-default abstract value
    type Value: Clone + Debug + Default + Eq;

    /// Constructs a new domain, given all locals where unassiagned locals have
    /// `LocalState::Unavailable` and parameters have
    /// `LocalState::Available(_, SimpleValue::Default)`
    fn new(context: &CFGContext, locals: BTreeMap<Var, LocalState<Self::Value>>) -> Self;

    /// Mutable access for the states of local variables
    fn locals_mut(&mut self) -> &mut BTreeMap<Var, LocalState<Self::Value>>;

    /// Immutable access for the states of local variables
    fn locals(&self) -> &BTreeMap<Var, LocalState<Self::Value>>;

    /// Joining values. Called during joining if a local is available in both states
    fn join_value(v1: &Self::Value, v2: &Self::Value) -> Self::Value;

    /// `join_impl` is called after joining locals in `join` if any custom joining logic is needed
    fn join_impl(&mut self, other: &Self, result: &mut JoinResult);
}

impl<V: SimpleDomain> AbstractDomain for V {
    fn join(&mut self, other: &Self) -> JoinResult {
        use LocalState as L;
        let self_locals = self.locals();
        let other_locals = other.locals();
        assert!(
            self_locals.keys().all(|v| other_locals.contains_key(v)),
            "ICE. Incorrectly implemented abstract interpreter. \
            Local variables should not be removed from the map"
        );
        assert!(
            other_locals.keys().all(|v| self_locals.contains_key(v)),
            "ICE. Incorrectly implemented abstract interpreter. \
            Local variables should not be removed from the map"
        );
        let mut result = JoinResult::Unchanged;
        for (local, other_state) in other_locals {
            match (self.locals().get(&local).unwrap(), other_state) {
                // both available, join the value
                (L::Available(loc, v1), L::Available(_, v2)) => {
                    let loc = *loc;
                    let joined = Self::join_value(v1, v2);
                    if v1 != &joined {
                        result = JoinResult::Changed
                    }
                    self.locals_mut().insert(*local, L::Available(loc, joined));
                }
                // equal so nothing to do
                (L::Unavailable(_, _), L::Unavailable(_, _))
                | (L::MaybeUnavailable { .. }, L::MaybeUnavailable { .. }) => (),
                // if its partially assigned, stays partially assigned
                (L::MaybeUnavailable { .. }, _) => (),

                // if was partially assigned in other, its now partially assigned
                (_, L::MaybeUnavailable { .. }) => {
                    result = JoinResult::Changed;
                    self.locals_mut().insert(*local, other_state.clone());
                }

                // Available in one but not the other, so maybe unavailable
                (L::Available(available, _), L::Unavailable(unavailable, reason))
                | (L::Unavailable(unavailable, reason), L::Available(available, _)) => {
                    result = JoinResult::Changed;
                    let available = *available;
                    let unavailable = *unavailable;
                    let state = L::MaybeUnavailable {
                        available,
                        unavailable,
                        unavailable_reason: *reason,
                    };
                    self.locals_mut().insert(*local, state);
                }
            }
        }
        self.join_impl(other, &mut result);
        result
    }
}

/// Trait for simple abstract interpreter passes. Custom hooks can be implemented with additional
/// logic as needed. The provided implementation will do all of the plumbing of abstract values
/// through the expressions, commands, and locals.
pub trait SimpleAbsInt: Sized {
    type State: SimpleDomain;
    /// The execution context local to a command
    type ExecutionContext: SimpleExecutionContext;

    /// Given the initial state/domain, construct a new abstract interpreter.
    /// Return None if it should not be run given this context
    fn new(context: &CFGContext, init_state: &mut Self::State) -> Option<Self>;

    fn verify(context: &CFGContext, cfg: &BlockCFG) -> Diagnostics {
        let mut locals = context
            .locals
            .key_cloned_iter()
            .map(|(v, _)| {
                (
                    v,
                    LocalState::Unavailable(v.0.loc, UnavailableReason::Unassigned),
                )
            })
            .collect::<BTreeMap<_, _>>();
        for (param, _) in &context.signature.parameters {
            locals.insert(
                *param,
                LocalState::Available(
                    param.0.loc,
                    <<Self as SimpleAbsInt>::State as SimpleDomain>::Value::default(),
                ),
            );
        }
        let mut init_state = Self::State::new(context, locals);
        let Some(mut ai) = Self::new(context, &mut init_state) else {
            return Diagnostics::new();
        };
        let (final_state, ds) = ai.analyze_function(cfg, init_state);
        ai.finish(final_state, ds)
    }

    fn visitor() -> Visitor {
        Visitor::AbsIntVisitor(Box::new(|context, cfg| Self::verify(context, cfg)))
    }

    /// A hook for an additional processing after visiting all codes. The `final_states` are the
    /// pre-states for each block (keyed by the label for the block). The `diags` are collected from
    /// all code visited.
    fn finish(
        &mut self,
        final_states: BTreeMap<Label, Self::State>,
        diags: Diagnostics,
    ) -> Diagnostics;

    /// A hook for any pre-processing at the start of a command
    fn start_command(&self, pre: &mut Self::State) -> Self::ExecutionContext;

    /// A hook for any post-processing after a command has been visited
    fn finish_command(
        &self,
        context: Self::ExecutionContext,
        state: &mut Self::State,
    ) -> Diagnostics;

    /// custom visit for a command. It will skip `command` if `command_custom` returns true.
    fn command_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        cmd: &Command,
    ) -> bool;
    fn command(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        cmd: &Command,
    ) {
        use Command_ as C;
        if self.command_custom(context, state, cmd) {
            return;
        }
        let sp!(_, cmd_) = cmd;
        match cmd_ {
            C::Assign(ls, e) => {
                let values = self.exp(context, state, e);
                self.lvalues(context, state, ls, values);
            }
            C::Mutate(el, er) => {
                self.exp(context, state, er);
                self.exp(context, state, el);
            }
            C::JumpIf { cond: e, .. }
            | C::IgnoreAndPop { exp: e, .. }
            | C::Return { exp: e, .. }
            | C::Abort(e) => {
                self.exp(context, state, e);
            }
            C::Jump { .. } => (),
            C::Break | C::Continue => panic!("ICE break/continue not translated to jumps"),
        }
    }

    fn lvalues(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        ls: &[LValue],
        values: Vec<<Self::State as SimpleDomain>::Value>,
    ) {
        // pad with defautl to account for errors
        let padded_values = values.into_iter().chain(std::iter::repeat(
            <Self::State as SimpleDomain>::Value::default(),
        ));
        for (l, value) in ls.iter().zip(padded_values) {
            self.lvalue(context, state, l, value)
        }
    }

    /// custom visit for an lvalue. It will skip `lvalue` if `lvalue_custom` returns true.
    fn lvalue_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        l: &LValue,
        value: &<Self::State as SimpleDomain>::Value,
    ) -> bool;
    fn lvalue(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        l: &LValue,
        value: <Self::State as SimpleDomain>::Value,
    ) {
        use LValue_ as L;
        if self.lvalue_custom(context, state, l, &value) {
            return;
        }
        let sp!(loc, l_) = l;
        match l_ {
            L::Ignore => (),
            L::Var(v, _) => {
                let locals = state.locals_mut();
                locals.insert(*v, LocalState::Available(*loc, value));
            }
            L::Unpack(_, _, fields) => {
                for (_, l) in fields {
                    let v = <Self::State as SimpleDomain>::Value::default();
                    self.lvalue(context, state, l, v)
                }
            }
        }
    }

    /// custom visit for an exp. It will skip `exp` and `call_custom` if `exp_custom` returns Some.
    fn exp_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        parent_e: &Exp,
    ) -> Option<Vec<<Self::State as SimpleDomain>::Value>>;
    fn call_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        loc: &Loc,
        return_ty: &Type,
        f: &ModuleCall,
        args: Vec<<Self::State as SimpleDomain>::Value>,
    ) -> Option<Vec<<Self::State as SimpleDomain>::Value>>;
    fn exp(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        parent_e: &Exp,
    ) -> Vec<<Self::State as SimpleDomain>::Value> {
        use UnannotatedExp_ as E;
        if let Some(vs) = self.exp_custom(context, state, parent_e) {
            return vs;
        }
        let eloc = &parent_e.exp.loc;
        match &parent_e.exp.value {
            E::Move { var, .. } => {
                let locals = state.locals_mut();
                let prev = locals.insert(
                    *var,
                    LocalState::Unavailable(*eloc, UnavailableReason::Moved),
                );
                match prev {
                    Some(LocalState::Available(_, value)) => {
                        vec![value]
                    }
                    // Possible error case
                    _ => default_values(1),
                }
            }
            E::Copy { var, .. } => {
                let locals = state.locals_mut();
                match locals.get(var) {
                    Some(LocalState::Available(_, value)) => vec![value.clone()],
                    // Possible error case
                    _ => default_values(1),
                }
            }
            E::BorrowLocal(_, _) => default_values(1),
            E::Freeze(e)
            | E::Dereference(e)
            | E::Borrow(_, e, _)
            | E::Cast(e, _)
            | E::UnaryExp(_, e) => {
                self.exp(context, state, e);
                default_values(1)
            }
            E::Builtin(_, e) => {
                self.exp(context, state, e);
                default_values_for_ty(&parent_e.ty)
            }
            E::Vector(_, n, _, e) => {
                self.exp(context, state, e);
                default_values(*n)
            }
            E::ModuleCall(mcall) => {
                let evalues = self.exp(context, state, &mcall.arguments);
                if let Some(vs) =
                    self.call_custom(context, state, eloc, &parent_e.ty, &mcall, evalues)
                {
                    return vs;
                }

                default_values_for_ty(&parent_e.ty)
            }

            E::Unit { .. } => vec![],
            E::Value(_) | E::Constant(_) | E::Spec(_, _) | E::UnresolvedError => default_values(1),

            E::BinopExp(e1, _, e2) => {
                self.exp(context, state, e1);
                self.exp(context, state, e2);
                default_values(1)
            }
            E::Pack(_, _, fields) => {
                for (_, _, e) in fields {
                    self.exp(context, state, e);
                }
                default_values(1)
            }
            E::ExpList(es) => es
                .iter()
                .flat_map(|item| self.exp_list_item(context, state, item))
                .collect(),

            E::Unreachable => panic!("ICE should not analyze dead code"),
        }
    }

    fn exp_list_item(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        item: &ExpListItem,
    ) -> Vec<<Self::State as SimpleDomain>::Value> {
        match item {
            ExpListItem::Single(e, _) | ExpListItem::Splat(_, e, _) => self.exp(context, state, e),
        }
    }
}

/// Provides default values depending on the arity of the type
pub fn default_values_for_ty<V: Clone + Default>(ty: &Type) -> Vec<V> {
    match &ty.value {
        Type_::Unit => vec![],
        Type_::Single(_) => default_values(1),
        Type_::Multiple(ts) => default_values(ts.len()),
    }
}

#[inline(always)]
/// A simple constructor for n default values
pub fn default_values<V: Clone + Default>(c: usize) -> Vec<V> {
    vec![V::default(); c]
}

impl<V: SimpleAbsInt> TransferFunctions for V {
    type State = V::State;

    fn execute(
        &mut self,
        pre: &mut Self::State,
        _lbl: Label,
        _idx: usize,
        cmd: &Command,
    ) -> Diagnostics {
        let mut context = self.start_command(pre);
        self.command(&mut context, pre, cmd);
        self.finish_command(context, pre)
    }
}
impl<V: SimpleAbsInt> AbstractInterpreter for V {}
