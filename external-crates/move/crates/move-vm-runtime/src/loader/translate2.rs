// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    loader::{
        arena::{self, ArenaPointer},
        ast::*,
        package_cache::{self, VTableKey},
        CacheCursor, ModuleCache,
    },
    native_functions::NativeFunctions,
};
use move_binary_format::{
    errors::{PartialVMError, PartialVMResult},
    file_format::{
        self as FF, CompiledModule, FunctionDefinition, FunctionDefinitionIndex,
        FunctionHandleIndex, SignatureIndex, TableIndex,
    },
};
use move_core_types::{
    account_address::AccountAddress, identifier::Identifier, language_storage::ModuleId,
    vm_status::StatusCode,
};
use move_vm_types::loaded_data::runtime_types::Type;
use parking_lot::RwLock;
use std::{
    collections::{btree_map, BTreeMap, HashMap},
    fmt::Debug,
    hash::Hash,
};

use super::{
    package_cache::{LoadedPackage, PackageCache},
    type_cache::TypeCache,
};

struct Context<'a> {
    link_context: AccountAddress,
    cache: &'a LoadedPackage,
    module: &'a CompiledModule,
    single_signature_token_map: BTreeMap<SignatureIndex, Type>,
    type_cache: &'a RwLock<TypeCache>,
}

struct DebugFlags {
    function_list_sizes: bool,
    function_resolution: bool,
}

const DEBUG_FLAGS: DebugFlags = DebugFlags {
    function_list_sizes: true,
    function_resolution: true,
};

pub fn alloc_function(
    natives: &NativeFunctions,
    index: FunctionDefinitionIndex,
    def: &FunctionDefinition,
    module: &CompiledModule,
) -> Function {
    let handle = module.function_handle_at(def.function);
    let name = module.identifier_at(handle.name).to_owned();
    let module_id = module.self_id();
    let (native, def_is_native) = if def.is_native() {
        (
            natives.resolve(
                module_id.address(),
                module_id.name().as_str(),
                name.as_str(),
            ),
            true,
        )
    } else {
        (None, false)
    };
    let parameters = handle.parameters;
    let parameters_len = module.signature_at(parameters).0.len();
    // Native functions do not have a code unit
    let (locals_len, jump_tables) = match &def.code {
        Some(code) => (
            parameters_len + module.signature_at(code.locals).0.len(),
            code.jump_tables.clone(),
        ),
        None => (0, vec![]),
    };
    let return_ = handle.return_;
    let return_len = module.signature_at(return_).0.len();
    let type_parameters = handle.type_parameters.clone();
    Function {
        file_format_version: module.version(),
        index,
        code: arena::null_ptr(),
        parameters,
        return_,
        type_parameters,
        native,
        def_is_native,
        module: module_id,
        name,
        parameters_len,
        locals_len,
        return_len,
        jump_tables,
    }
}

fn code(context: &mut Context, code: &[FF::Bytecode]) -> PartialVMResult<*const [Bytecode]> {
    let result: *mut [Bytecode] = context.cache.package_arena.alloc_slice(
        code.iter()
            .map(|bc| bytecode(context, bc))
            .collect::<PartialVMResult<Vec<Bytecode>>>()?
            .into_iter(),
    );
    Ok(result as *const [Bytecode])
}

fn bytecode(context: &mut Context, bytecode: &FF::Bytecode) -> PartialVMResult<Bytecode> {
    let bytecode = match bytecode {
        // Calls -- these get compiled to something more-direct here
        FF::Bytecode::Call(ndx) => {
            let call_type = call(context, *ndx)?;
            match call_type {
                CallType::Known(func) => Bytecode::KnownCall(func.to_ref()),
                CallType::Virtual(vtable) => Bytecode::VirtualCall(vtable),
            }
        }

        // For now, generic calls retain an index so we can look up their signature as well.
        FF::Bytecode::CallGeneric(ndx) => Bytecode::CallGeneric(*ndx),

        // Standard Codes
        FF::Bytecode::Pop => Bytecode::Pop,
        FF::Bytecode::Ret => Bytecode::Ret,
        FF::Bytecode::BrTrue(n) => Bytecode::BrTrue(*n),
        FF::Bytecode::BrFalse(n) => Bytecode::BrFalse(*n),
        FF::Bytecode::Branch(n) => Bytecode::Branch(*n),

        FF::Bytecode::LdU256(n) => Bytecode::LdU256(n.clone()),
        FF::Bytecode::LdU128(n) => Bytecode::LdU128(n.clone()),
        FF::Bytecode::LdU16(n) => Bytecode::LdU16(*n),
        FF::Bytecode::LdU32(n) => Bytecode::LdU32(*n),
        FF::Bytecode::LdU64(n) => Bytecode::LdU64(*n),
        FF::Bytecode::LdU8(n) => Bytecode::LdU8(*n),

        FF::Bytecode::LdConst(ndx) => Bytecode::LdConst(*ndx),
        FF::Bytecode::LdTrue => Bytecode::LdTrue,
        FF::Bytecode::LdFalse => Bytecode::LdFalse,

        FF::Bytecode::CopyLoc(ndx) => Bytecode::CopyLoc(*ndx),
        FF::Bytecode::MoveLoc(ndx) => Bytecode::MoveLoc(*ndx),
        FF::Bytecode::StLoc(ndx) => Bytecode::StLoc(*ndx),
        FF::Bytecode::Pack(ndx) => Bytecode::Pack(*ndx),
        FF::Bytecode::PackGeneric(ndx) => Bytecode::PackGeneric(*ndx),
        FF::Bytecode::Unpack(ndx) => Bytecode::Unpack(*ndx),
        FF::Bytecode::UnpackGeneric(ndx) => Bytecode::UnpackGeneric(*ndx),
        FF::Bytecode::ReadRef => Bytecode::ReadRef,
        FF::Bytecode::WriteRef => Bytecode::WriteRef,
        FF::Bytecode::FreezeRef => Bytecode::FreezeRef,
        FF::Bytecode::MutBorrowLoc(ndx) => Bytecode::MutBorrowLoc(*ndx),
        FF::Bytecode::ImmBorrowLoc(ndx) => Bytecode::ImmBorrowLoc(*ndx),
        FF::Bytecode::MutBorrowField(ndx) => Bytecode::MutBorrowField(*ndx),
        FF::Bytecode::MutBorrowFieldGeneric(ndx) => Bytecode::MutBorrowFieldGeneric(*ndx),
        FF::Bytecode::ImmBorrowField(ndx) => Bytecode::ImmBorrowField(*ndx),
        FF::Bytecode::ImmBorrowFieldGeneric(ndx) => Bytecode::ImmBorrowFieldGeneric(*ndx),

        FF::Bytecode::Add => Bytecode::Add,
        FF::Bytecode::Sub => Bytecode::Sub,
        FF::Bytecode::Mul => Bytecode::Mul,
        FF::Bytecode::Mod => Bytecode::Mod,
        FF::Bytecode::Div => Bytecode::Div,
        FF::Bytecode::BitOr => Bytecode::BitOr,
        FF::Bytecode::BitAnd => Bytecode::BitAnd,
        FF::Bytecode::Xor => Bytecode::Xor,
        FF::Bytecode::Or => Bytecode::Or,
        FF::Bytecode::And => Bytecode::And,
        FF::Bytecode::Not => Bytecode::Not,
        FF::Bytecode::Eq => Bytecode::Eq,
        FF::Bytecode::Neq => Bytecode::Neq,
        FF::Bytecode::Lt => Bytecode::Lt,
        FF::Bytecode::Gt => Bytecode::Gt,
        FF::Bytecode::Le => Bytecode::Le,
        FF::Bytecode::Ge => Bytecode::Ge,
        FF::Bytecode::Abort => Bytecode::Abort,
        FF::Bytecode::Nop => Bytecode::Nop,
        FF::Bytecode::Shl => Bytecode::Shl,
        FF::Bytecode::Shr => Bytecode::Shr,

        FF::Bytecode::CastU256 => Bytecode::CastU256,
        FF::Bytecode::CastU128 => Bytecode::CastU128,
        FF::Bytecode::CastU16 => Bytecode::CastU16,
        FF::Bytecode::CastU32 => Bytecode::CastU32,
        FF::Bytecode::CastU64 => Bytecode::CastU64,
        FF::Bytecode::CastU8 => Bytecode::CastU8,

        // Vectors
        FF::Bytecode::VecPack(si, size) => {
            check_vector_type(context, si)?;
            Bytecode::VecPack(*si, *size)
        }
        FF::Bytecode::VecLen(si) => {
            check_vector_type(context, si)?;
            Bytecode::VecLen(*si)
        }
        FF::Bytecode::VecImmBorrow(si) => {
            check_vector_type(context, si)?;
            Bytecode::VecImmBorrow(*si)
        }
        FF::Bytecode::VecMutBorrow(si) => {
            check_vector_type(context, si)?;
            Bytecode::VecMutBorrow(*si)
        }
        FF::Bytecode::VecPushBack(si) => {
            check_vector_type(context, si)?;
            Bytecode::VecPushBack(*si)
        }
        FF::Bytecode::VecPopBack(si) => {
            check_vector_type(context, si)?;
            Bytecode::VecPopBack(*si)
        }
        FF::Bytecode::VecUnpack(si, size) => {
            check_vector_type(context, si)?;
            Bytecode::VecUnpack(*si, *size)
        }
        FF::Bytecode::VecSwap(si) => {
            check_vector_type(context, si)?;
            Bytecode::VecSwap(*si)
        }
        // Structs and Fields

        // Enums and Variants
        FF::Bytecode::PackVariant(ndx) => Bytecode::PackVariant(*ndx),
        FF::Bytecode::PackVariantGeneric(ndx) => Bytecode::PackVariantGeneric(*ndx),
        FF::Bytecode::UnpackVariant(ndx) => Bytecode::UnpackVariant(*ndx),
        FF::Bytecode::UnpackVariantImmRef(ndx) => Bytecode::UnpackVariantImmRef(*ndx),
        FF::Bytecode::UnpackVariantMutRef(ndx) => Bytecode::UnpackVariantMutRef(*ndx),
        FF::Bytecode::UnpackVariantGeneric(ndx) => Bytecode::UnpackVariantGeneric(*ndx),
        FF::Bytecode::UnpackVariantGenericImmRef(ndx) => Bytecode::UnpackVariantGenericImmRef(*ndx),
        FF::Bytecode::UnpackVariantGenericMutRef(ndx) => Bytecode::UnpackVariantGenericMutRef(*ndx),
        FF::Bytecode::VariantSwitch(ndx) => Bytecode::VariantSwitch(*ndx),

        // Deprecated bytecodes -- bail
        FF::Bytecode::ExistsDeprecated(_)
        | FF::Bytecode::ExistsGenericDeprecated(_)
        | FF::Bytecode::MoveFromDeprecated(_)
        | FF::Bytecode::MoveFromGenericDeprecated(_)
        | FF::Bytecode::MoveToDeprecated(_)
        | FF::Bytecode::MoveToGenericDeprecated(_)
        | FF::Bytecode::MutBorrowGlobalDeprecated(_)
        | FF::Bytecode::MutBorrowGlobalGenericDeprecated(_)
        | FF::Bytecode::ImmBorrowGlobalDeprecated(_)
        | FF::Bytecode::ImmBorrowGlobalGenericDeprecated(_) => {
            unreachable!("Global bytecodes deprecated")
        }
    };
    Ok(bytecode)
}

fn call(
    context: &mut Context,
    function_handle_index: FunctionHandleIndex,
) -> PartialVMResult<CallType> {
    let func_handle = context.module.function_handle_at(function_handle_index);
    let func_name = context.module.identifier_at(func_handle.name);
    let module_handle = context.module.module_handle_at(func_handle.module);
    let runtime_id = context.module.module_id_for_handle(module_handle);
    let vtable_key = VTableKey {
        package_key: *runtime_id.address(),
        module_name: runtime_id.name().to_owned(),
        function_name: func_name.to_owned(),
    };
    if DEBUG_FLAGS.function_resolution {
        println!("Resolving function: {:?}", vtable_key);
    }
    Ok(match context.cache.try_resolve_function(&vtable_key) {
        Some(func) => CallType::Known(func),
        None => CallType::Virtual(vtable_key),
    })
}

fn check_vector_type(
    context: &mut Context,
    signature_index: &SignatureIndex,
) -> PartialVMResult<()> {
    if !context
        .single_signature_token_map
        .contains_key(signature_index)
    {
        let ty = match context.module.signature_at(*signature_index).0.first() {
            None => {
                return Err(
                    PartialVMError::new(StatusCode::VERIFIER_INVARIANT_VIOLATION).with_message(
                        "the type argument for vector-related bytecode \
                        expects one and only one signature token"
                            .to_owned(),
                    ),
                );
            }
            Some(sig_token) => sig_token,
        };
        context.single_signature_token_map.insert(
            *signature_index,
            context.type_cache.read().make_type(context.module, ty)?,
        );
    }
    Ok(())
}

pub enum CallType {
    Known(ArenaPointer<Function>),
    Virtual(VTableKey),
}

impl Debug for CallType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallType::Known(fun) => write!(f, "Known({})", fun.to_ref().name),
            CallType::Virtual(vtable) => write!(
                f,
                "Virtual({}::{}::{})",
                vtable.package_key, vtable.module_name, vtable.function_name
            ),
        }
    }
}

pub fn module(
    natives: &NativeFunctions,
    package_id: AccountAddress,
    module: &CompiledModule,
    package_cache: &mut LoadedPackage,
    type_cache: &RwLock<TypeCache>,
) -> Result<LoadedModule, PartialVMError> {
    let self_id = module.self_id();
    println!("Loading module: {}", self_id);

    let mut instantiation_signatures: BTreeMap<SignatureIndex, Vec<Type>> = BTreeMap::new();
    // helper to build the sparse signature vector
    fn cache_signatures(
        instantiation_signatures: &mut BTreeMap<SignatureIndex, Vec<Type>>,
        module: &CompiledModule,
        instantiation_idx: SignatureIndex,
        type_cache: &RwLock<TypeCache>,
    ) -> Result<(), PartialVMError> {
        if let btree_map::Entry::Vacant(e) = instantiation_signatures.entry(instantiation_idx) {
            let instantiation = module
                .signature_at(instantiation_idx)
                .0
                .iter()
                .map(|ty| type_cache.read().make_type(module, ty))
                .collect::<Result<Vec<_>, _>>()?;
            e.insert(instantiation);
        }
        Ok(())
    }

    let mut type_refs = vec![];
    let mut structs = vec![];
    let mut struct_instantiations = vec![];
    let mut enums = vec![];
    let mut enum_instantiations = vec![];
    let mut function_instantiations = vec![];
    let mut field_handles = vec![];
    let mut field_instantiations: Vec<FieldInstantiation> = vec![];

    for datatype_handle in module.datatype_handles() {
        let struct_name = module.identifier_at(datatype_handle.name);
        let module_handle = module.module_handle_at(datatype_handle.module);
        let runtime_id = module.module_id_for_handle(module_handle);
        type_refs.push(
            type_cache
                .read()
                .resolve_type_by_name(&(
                    *runtime_id.address(),
                    runtime_id.name().to_owned(),
                    struct_name.to_owned(),
                ))?
                .0,
        );
    }

    for struct_def in module.struct_defs() {
        let idx = type_refs[struct_def.struct_handle.0 as usize];
        let field_count = type_cache.read().cached_types.binaries[idx.0]
            .get_struct()?
            .fields
            .len() as u16;
        structs.push(StructDef { field_count, idx });
    }

    for struct_inst in module.struct_instantiations() {
        let def = struct_inst.def.0 as usize;
        let struct_def = &structs[def];
        let field_count = struct_def.field_count;

        let instantiation_idx = struct_inst.type_parameters;
        cache_signatures(
            &mut instantiation_signatures,
            module,
            instantiation_idx,
            type_cache,
        )?;
        struct_instantiations.push(StructInstantiation {
            field_count,
            def: struct_def.idx,
            instantiation_idx,
        });
    }

    for enum_def in module.enum_defs() {
        let idx = type_refs[enum_def.enum_handle.0 as usize];
        let datatype = &type_cache.read().cached_types.binaries[idx.0];
        let enum_type = datatype.get_enum()?;
        let variant_count = enum_type.variants.len() as u16;
        let variants = enum_type
            .variants
            .iter()
            .enumerate()
            .map(|(tag, variant_type)| VariantDef {
                tag: tag as u16,
                field_count: variant_type.fields.len() as u16,
                field_types: variant_type.fields.clone(),
            })
            .collect();
        enums.push(EnumDef {
            variant_count,
            variants,
            idx,
        });
    }

    for enum_inst in module.enum_instantiations() {
        let def = enum_inst.def.0 as usize;
        let enum_def = &enums[def];
        let variant_count_map = enum_def.variants.iter().map(|v| v.field_count).collect();
        let instantiation_idx = enum_inst.type_parameters;
        cache_signatures(
            &mut instantiation_signatures,
            module,
            instantiation_idx,
            type_cache,
        )?;

        enum_instantiations.push(EnumInstantiation {
            variant_count_map,
            def: enum_def.idx,
            instantiation_idx,
        });
    }

    if DEBUG_FLAGS.function_list_sizes {
        println!("pushing {} functions", module.function_defs().len());
    }
    let loaded_functions =
        package_cache
            .package_arena
            .alloc_slice(module.function_defs().iter().enumerate().map(|(ndx, fun)| {
                let findex = FunctionDefinitionIndex(ndx as TableIndex);
                alloc_function(natives, findex, fun, module)
            }));

    package_cache.insert_and_make_module_vtable(
        self_id.name().to_owned(),
        arena::mut_to_ref_slice(loaded_functions)
            .iter()
            .map(|function| {
                (
                    function.name.clone(),
                    ArenaPointer::new(function as *const Function),
                )
            }),
    )?;

    if DEBUG_FLAGS.function_list_sizes {
        println!("handle size: {}", module.function_handles().len());
    }

    let single_signature_token_map = BTreeMap::new();
    let mut context = Context {
        link_context: package_id,
        cache: package_cache,
        module,
        single_signature_token_map,
        type_cache,
    };

    for (alloc, fun) in arena::to_mut_ref_slice(loaded_functions)
        .iter_mut()
        .zip(module.function_defs())
    {
        if let Some(code_unit) = &fun.code {
            alloc.code = code(&mut context, &code_unit.code)?;
        }
    }

    for func_inst in context.module.function_instantiations() {
        let handle = call(&mut context, func_inst.handle)?;

        let instantiation_idx = func_inst.type_parameters;
        cache_signatures(
            &mut instantiation_signatures,
            module,
            instantiation_idx,
            type_cache,
        )?;
        let CallType::Known(ptr) = handle else {
            panic!("virtual function instantiation -- not impelemented yet")
        };

        function_instantiations.push(FunctionInstantiation {
            handle: ArenaPointer::new(ptr.to_ref()),
            instantiation_idx,
        });
    }

    for f_handle in module.field_handles() {
        let def_idx = f_handle.owner;
        let owner = structs[def_idx.0 as usize].idx;
        let offset = f_handle.field as usize;
        field_handles.push(FieldHandle { offset, owner });
    }

    for f_inst in module.field_instantiations() {
        let fh_idx = f_inst.handle;
        let owner = field_handles[fh_idx.0 as usize].owner;
        let offset = field_handles[fh_idx.0 as usize].offset;

        field_instantiations.push(FieldInstantiation { offset, owner });
    }

    let Context {
        link_context: _,
        cache: _,
        module,
        single_signature_token_map,
        type_cache: _,
    } = context;

    Ok(LoadedModule {
        id: self_id,
        type_refs,
        structs,
        struct_instantiations,
        enums,
        enum_instantiations,
        function_instantiations,
        field_handles,
        field_instantiations,
        // TODO: Remove this field
        function_map: HashMap::new(),
        single_signature_token_map,
        instantiation_signatures,
        variant_handles: module.variant_handles().to_vec(),
        variant_instantiation_handles: module.variant_instantiation_handles().to_vec(),
    })
}
