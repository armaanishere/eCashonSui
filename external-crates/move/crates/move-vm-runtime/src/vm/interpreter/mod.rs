// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    cache::{arena::ArenaPointer, type_cache::TypeCache},
    jit::runtime::ast::Function,
    natives::extensions::NativeContextExtensions,
    vm::interpreter::state::{CallFrame, MachineState, ModuleDefinitionResolver},
    vm::runtime_vtables::RuntimeVTables,
};
use move_binary_format::errors::*;
use move_vm_config::runtime::VMConfig;
use move_vm_profiler::{profile_close_frame, profile_open_frame};
use move_vm_types::{gas::GasMeter, loaded_data::runtime_types::Type, values::Value};
use parking_lot::RwLock;
use std::sync::Arc;

mod eval;
pub(crate) mod state;

/// Entrypoint into the interpreter. All external calls need to be routed through this
/// function.
pub(crate) fn run(
    function: ArenaPointer<Function>,
    ty_args: Vec<Type>,
    args: Vec<Value>,
    vtables: &RuntimeVTables,
    type_cache: Arc<RwLock<TypeCache>>,
    vm_config: Arc<VMConfig>,
    extensions: &mut NativeContextExtensions,
    gas_meter: &mut impl GasMeter,
) -> VMResult<Vec<Value>> {
    let fun_ref = function.to_ref();
    profile_open_frame!(gas_meter, fun_ref.pretty_string());

    if fun_ref.is_native() {
        let return_values = eval::call_native_with_args(
            None,
            &type_cache,
            gas_meter,
            &vm_config.runtime_limits_config,
            extensions,
            fun_ref,
            &ty_args,
            args.into(),
        )
        .map_err(|e| {
            e.at_code_offset(fun_ref.index(), 0)
                .finish(Location::Module(fun_ref.module_id().clone()))
        })?;

        profile_close_frame!(gas_meter, fun_ref.pretty_string());

        Ok(return_values.into_iter().collect())
    } else {
        let module_id = function.to_ref().module_id();
        let resolver = ModuleDefinitionResolver::new(vtables, module_id)
            .map_err(|err| err.finish(Location::Module(fun_ref.module_id().clone())))?;
        let initial_frame = CallFrame::new(resolver, function, ty_args, args);
        let state = MachineState::new(initial_frame);
        eval::run(
            state,
            vtables,
            &type_cache,
            vm_config,
            extensions,
            gas_meter,
        )
    }
}
