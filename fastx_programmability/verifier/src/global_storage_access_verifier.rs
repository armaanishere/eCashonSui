// Copyright (c) Mysten Labs
// SPDX-License-Identifier: Apache-2.0

use crate::verification_failure;
use fastx_types::error::SuiResult;
use move_binary_format::{
    binary_views::BinaryIndexedView,
    file_format::{Bytecode, CompiledModule},
};

pub fn verify_module(module: &CompiledModule) -> SuiResult {
    verify_global_storage_access(module)
}

/// Global storage in fastx is handled by fastx instead of within Move.
/// Hence we want to forbid any global storage access in Move.
fn verify_global_storage_access(module: &CompiledModule) -> SuiResult {
    let view = BinaryIndexedView::Module(module);
    for func_def in &module.function_defs {
        if func_def.code.is_none() {
            continue;
        }
        let code = &func_def.code.as_ref().unwrap().code;
        let mut invalid_bytecode = vec![];
        for bytecode in code {
            match bytecode {
                Bytecode::MoveFrom(_)
                | Bytecode::MoveFromGeneric(_)
                | Bytecode::MoveTo(_)
                | Bytecode::MoveToGeneric(_)
                | Bytecode::ImmBorrowGlobal(_)
                | Bytecode::MutBorrowGlobal(_)
                | Bytecode::ImmBorrowGlobalGeneric(_)
                | Bytecode::MutBorrowGlobalGeneric(_)
                | Bytecode::Exists(_)
                | Bytecode::ExistsGeneric(_) => {
                    invalid_bytecode.push(bytecode);
                }
                Bytecode::Pop
                | Bytecode::Ret
                | Bytecode::BrTrue(_)
                | Bytecode::BrFalse(_)
                | Bytecode::Branch(_)
                | Bytecode::LdU8(_)
                | Bytecode::LdU64(_)
                | Bytecode::LdU128(_)
                | Bytecode::CastU8
                | Bytecode::CastU64
                | Bytecode::CastU128
                | Bytecode::LdConst(_)
                | Bytecode::LdTrue
                | Bytecode::LdFalse
                | Bytecode::CopyLoc(_)
                | Bytecode::MoveLoc(_)
                | Bytecode::StLoc(_)
                | Bytecode::Call(_)
                | Bytecode::CallGeneric(_)
                | Bytecode::Pack(_)
                | Bytecode::PackGeneric(_)
                | Bytecode::Unpack(_)
                | Bytecode::UnpackGeneric(_)
                | Bytecode::ReadRef
                | Bytecode::WriteRef
                | Bytecode::FreezeRef
                | Bytecode::MutBorrowLoc(_)
                | Bytecode::ImmBorrowLoc(_)
                | Bytecode::MutBorrowField(_)
                | Bytecode::MutBorrowFieldGeneric(_)
                | Bytecode::ImmBorrowField(_)
                | Bytecode::ImmBorrowFieldGeneric(_)
                | Bytecode::Add
                | Bytecode::Sub
                | Bytecode::Mul
                | Bytecode::Mod
                | Bytecode::Div
                | Bytecode::BitOr
                | Bytecode::BitAnd
                | Bytecode::Xor
                | Bytecode::Shl
                | Bytecode::Shr
                | Bytecode::Or
                | Bytecode::And
                | Bytecode::Not
                | Bytecode::Eq
                | Bytecode::Neq
                | Bytecode::Lt
                | Bytecode::Gt
                | Bytecode::Le
                | Bytecode::Ge
                | Bytecode::Abort
                | Bytecode::Nop
                | Bytecode::VecPack(_, _)
                | Bytecode::VecLen(_)
                | Bytecode::VecImmBorrow(_)
                | Bytecode::VecMutBorrow(_)
                | Bytecode::VecPushBack(_)
                | Bytecode::VecPopBack(_)
                | Bytecode::VecUnpack(_, _)
                | Bytecode::VecSwap(_) => {}
            }
        }
        if !invalid_bytecode.is_empty() {
            return Err(verification_failure(format!(
                "Access to Move global storage is not allowed. Found in function {}: {:?}",
                view.identifier_at(view.function_handle_at(func_def.function).name),
                invalid_bytecode,
            )));
        }
    }
    Ok(())
}
