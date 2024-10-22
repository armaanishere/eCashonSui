// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

// TODO(vm-rewrite): Examine this test -- it seems to be obsolete now since we don't support
// in-place upgrades anymore.
//
// use move_binary_format::{file_format::basic_test_module, file_format_common::VERSION_MAX};
// use move_core_types::{account_address::AccountAddress, vm_status::StatusCode};
// use move_vm_config::runtime::VMConfig;
// use move_vm_runtime::dev_utils::in_memory_test_adapter::InMemoryTestAdapter;
// use move_vm_runtime::dev_utils::vm_test_adapter::VMTestAdapter;
// use move_vm_runtime::natives::move_stdlib::{stdlib_native_functions, GasParameters};
// use move_vm_runtime::runtime::MoveRuntime;
// use move_vm_runtime::shared::gas::UnmeteredGasMeter;
//
// #[test]
// fn test_publish_module_with_custom_max_binary_format_version() {
//     let m = basic_test_module();
//     let mut b_new = vec![];
//     let mut b_old = vec![];
//     m.serialize_with_version(VERSION_MAX, &mut b_new).unwrap();
//     m.serialize_with_version(VERSION_MAX.checked_sub(1).unwrap(), &mut b_old)
//         .unwrap();
//
//     // Should accept both modules with the default settings
//     {
//         let adapter = InMemoryTestAdapter::new();
//         let move_runtime = MoveRuntime::new_with_default_config(
//             stdlib_native_functions(
//                 AccountAddress::from_hex_literal("0x1").unwrap(),
//                 GasParameters::zeros(),
//                 /* silent debug */ true,
//             )
//             .unwrap(),
//         )
//         .unwrap();
//         let m_addr = *m.self_id().address();
//         let linkage = adapter
//             .generate_linkage_context(m_addr, m_addr, vec![b_new.clone()])
//             .unwrap();
//
//         adapter
//             .publish_package(linkage, m_addr, vec![b_new])
//             .unwrap();
//
//         sess.publish_module(
//             b_old.clone(),
//             *m.self_id().address(),
//             &mut UnmeteredGasMeter,
//         )
//         .unwrap();
//     }
//
//     // Should reject the module with newer version with max binary format version being set to VERSION_MAX - 1
//     {
//         let storage = InMemoryStorage::new();
//         let mut vm_config = VMConfig::default();
//         // lower the max version allowed
//         let max_updated = VERSION_MAX.checked_sub(1).unwrap();
//         vm_config.max_binary_format_version = max_updated;
//         vm_config.binary_config.max_binary_format_version = max_updated;
//
//         let vm = MoveVM::new_with_config(
//             move_vm_runtime::natives::move_stdlib::stdlib_native_function_table(
//                 AccountAddress::from_hex_literal("0x1").unwrap(),
//                 move_vm_runtime::natives::move_stdlib::GasParameters::zeros(),
//                 /* silent debug */ true,
//             ),
//             vm_config,
//         )
//         .unwrap();
//         let mut sess = vm.new_session(&storage);
//
//         assert_eq!(
//             sess.publish_module(
//                 b_new.clone(),
//                 *m.self_id().address(),
//                 &mut UnmeteredGasMeter,
//             )
//             .unwrap_err()
//             .major_status(),
//             StatusCode::UNKNOWN_VERSION
//         );
//
//         sess.publish_module(
//             b_old.clone(),
//             *m.self_id().address(),
//             &mut UnmeteredGasMeter,
//         )
//         .unwrap();
//     }
// }
