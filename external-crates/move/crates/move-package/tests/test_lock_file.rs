// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use expect_test::expect;
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::PathBuf,
};
use tempfile::TempDir;

use move_compiler::editions::{Edition, Flavor};
use move_package::lock_file::LockFile;
use move_package::BuildConfig;
use move_symbol_pool::Symbol;

#[test]
fn commit() {
    let pkg = create_test_package().unwrap();
    let lock_path = pkg.path().join("Move.lock");

    {
        let mut lock = LockFile::new(
            pkg.path().to_path_buf(),
            /* manifest_digest */ "42".to_string(),
            /* deps_digest */ "7".to_string(),
        )
        .unwrap();
        writeln!(lock, "# Write and commit").unwrap();
        lock.commit(&lock_path).unwrap();
    }

    assert!(lock_path.is_file());

    let lock_contents = {
        let mut lock_file = File::open(lock_path).unwrap();
        let mut buf = String::new();
        lock_file.read_to_string(&mut buf).unwrap();
        buf
    };

    // Check that the content written into the `LockFile` instance above can be found at the path
    // that that lock file was committed to (indicating that the commit actually happened).
    assert!(
        lock_contents.ends_with("# Write and commit\n"),
        "Lock file doesn't have expected content:\n{}",
        lock_contents,
    );
}

#[test]
fn discard() {
    let pkg = create_test_package().unwrap();

    {
        let mut lock = LockFile::new(
            pkg.path().to_path_buf(),
            /* manifest_digest */ "42".to_string(),
            /* deps_digest */ "7".to_string(),
        )
        .unwrap();
        writeln!(lock, "# Write but don't commit").unwrap();
    }

    assert!(!pkg.path().join("Move.lock").is_file());
}

#[test]
fn update_lock_file_toolchain_version() {
    let pkg = create_test_package().unwrap();
    let lock_path = pkg.path().join("Move.lock");

    let lock = LockFile::new(
        pkg.path().to_path_buf(),
        /* manifest_digest */ "42".to_string(),
        /* deps_digest */ "7".to_string(),
    )
    .unwrap();
    lock.commit(&lock_path).unwrap();

    let mut build_config: BuildConfig = Default::default();
    build_config.default_flavor = Some(Flavor::Sui);
    build_config.default_edition = Some(Edition {
        edition: Symbol::from("2024"),
        release: Some(Symbol::from("1")),
    });
    build_config.lock_file = Some(lock_path.clone());
    let _ = build_config.update_lock_file_toolchain_version("0.0.1".into());

    let lock_contents = {
        let mut lock_file = File::open(lock_path).unwrap();
        let mut buf = String::new();
        lock_file.read_to_string(&mut buf).unwrap();
        buf
    };

    let mut toml = lock_contents.parse::<toml_edit::Document>().unwrap();
    let move_table = toml["move"].as_table_mut().unwrap();
    let compiler_flags = move_table["compiler-flags"].as_inline_table_mut().unwrap();
    compiler_flags["lock_file"] = "...".into();

    let expected = expect![[r##"
        # @generated by Move, please check-in and do not edit manually.

        [move]
        version = 0
        manifest_digest = "42"
        deps_digest = "7"
        compiler-version = "0.0.1"
        compiler-flags = { additional_named_addresses = {}, default_edition = "2024.1", default_flavor = "sui", deps_as_root = false, dev_mode = false, fetch_deps_only = false, force_recompilation = false, generate_docs = false, lock_file = "...", silence_warnings = false, skip_fetch_latest_git_deps = false, test_mode = false, warnings_are_errors = false }
    "##]];
    expected.assert_eq(&toml.to_string());
}

/// Create a simple Move package with no sources (just a manifest and an output directory) in a
/// temporary directory, and return it.
fn create_test_package() -> io::Result<TempDir> {
    let dir = tempfile::tempdir()?;

    let toml_path: PathBuf = [".", "tests", "test_sources", "basic_no_deps", "Move.toml"]
        .into_iter()
        .collect();

    fs::copy(toml_path, dir.path().join("Move.toml"))?;
    Ok(dir)
}
