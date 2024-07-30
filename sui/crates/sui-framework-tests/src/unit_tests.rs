// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use move_cli::base::test::UnitTestResult;
use move_unit_test::UnitTestingConfig;
use std::{fs, io, path::PathBuf};
use sui_move::unit_test::run_move_unit_tests;
use sui_move_build::BuildConfig;

#[test]
#[cfg_attr(msim, ignore)]
fn run_sui_framework_tests() {
    check_move_unit_tests({
        let mut buf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        buf.extend(["..", "sui-framework", "packages", "sui-framework"]);
        buf
    });
}

#[test]
#[cfg_attr(msim, ignore)]
fn run_sui_system_tests() {
    check_move_unit_tests({
        let mut buf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        buf.extend(["..", "sui-framework", "packages", "sui-system"]);
        buf
    });
}

#[test]
#[cfg_attr(msim, ignore)]
fn run_deepbook_tests() {
    check_move_unit_tests({
        let mut buf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        buf.extend(["..", "sui-framework", "packages", "deepbook"]);
        buf
    });
}

#[test]
#[cfg_attr(msim, ignore)]
fn run_examples_move_unit_tests() {
    for example in [
        "basics",
        "defi",
        "capy",
        "fungible_tokens",
        "games",
        "move_tutorial",
        "nfts",
        "objects_tutorial",
    ] {
        let path = {
            let mut buf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            buf.extend(["..", "..", "sui_programmability", "examples", example]);
            buf
        };

        check_package_builds(path.clone());
        check_move_unit_tests(path);
    }
}

#[test]
#[cfg_attr(msim, ignore)]
fn run_docs_examples_move_unit_tests() -> io::Result<()> {
    let examples = {
        let mut buf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        buf.extend(["..", "..", "examples", "move"]);
        buf
    };

    for entry in fs::read_dir(examples)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            check_package_builds(entry.path());
            check_move_unit_tests(entry.path());
        }
    }

    Ok(())
}

#[test]
#[cfg_attr(msim, ignore)]
fn run_book_examples_move_unit_tests() {
    let path = {
        let mut buf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        buf.extend(["..", "..", "doc", "book", "examples"]);
        buf
    };

    check_package_builds(path.clone());
    check_move_unit_tests(path);
}

/// Ensure packages build outside of test mode.
fn check_package_builds(path: PathBuf) {
    let mut config = BuildConfig::new_for_testing();
    config.config.dev_mode = true;
    config.run_bytecode_verifier = true;
    config.print_diags_to_stderr = true;
    config.config.warnings_are_errors = true;
    config.config.silence_warnings = false;

    config
        .build(path.clone())
        .unwrap_or_else(|e| panic!("Building package {}.\nWith error {e}", path.display()));
}

fn check_move_unit_tests(path: PathBuf) {
    let mut config = BuildConfig::new_for_testing();
    // Make sure to verify tests
    config.config.dev_mode = true;
    config.config.test_mode = true;
    config.run_bytecode_verifier = true;
    config.print_diags_to_stderr = true;
    config.config.warnings_are_errors = true;
    config.config.silence_warnings = false;
    let move_config = config.config.clone();
    let testing_config = UnitTestingConfig::default_with_bound(Some(3_000_000));

    // build tests first to enable Sui-specific test code verification
    config
        .build(path.clone())
        .unwrap_or_else(|e| panic!("Building tests at {}.\nWith error {e}", path.display()));

    assert_eq!(
        run_move_unit_tests(path, move_config, Some(testing_config), false).unwrap(),
        UnitTestResult::Success
    );
}
