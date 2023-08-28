//! Utility types and functions for working with execution engine tests.

use std::{
    env, fs,
    path::{Path, PathBuf},
    rc::Rc,
};

use once_cell::sync::Lazy;

use casper_execution_engine::{
    core::engine_state::{
        engine_config::{DEFAULT_FEE_HANDLING, DEFAULT_REFUND_HANDLING},
        execution_result::ExecutionResult,
        genesis::{ExecConfig, ExecConfigBuilder, GenesisAccount, GenesisConfig},
        run_genesis_request::RunGenesisRequest,
        Error,
    },
    shared::{additive_map::AdditiveMap, transform::Transform},
};
use casper_types::{account::Account, Gas, Key, StoredValue};

use super::{DEFAULT_ROUND_SEIGNIORAGE_RATE, DEFAULT_SYSTEM_CONFIG, DEFAULT_UNBONDING_DELAY};
use crate::{
    DEFAULT_AUCTION_DELAY, DEFAULT_CHAINSPEC_REGISTRY, DEFAULT_CHAIN_NAME,
    DEFAULT_GENESIS_CONFIG_HASH, DEFAULT_GENESIS_TIMESTAMP_MILLIS,
    DEFAULT_LOCKED_FUNDS_PERIOD_MILLIS, DEFAULT_PROTOCOL_VERSION, DEFAULT_VALIDATOR_SLOTS,
    DEFAULT_WASM_CONFIG,
};

static RUST_WORKSPACE_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("CARGO_MANIFEST_DIR should have parent");
    assert!(
        path.exists(),
        "Workspace path {} does not exists",
        path.display()
    );
    path.to_path_buf()
});
// The location of compiled Wasm files if compiled from the Rust sources within the casper-node
// repo, i.e. 'casper-node/target/wasm32-unknown-unknown/release/'.
static RUST_WORKSPACE_WASM_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let path = RUST_WORKSPACE_PATH
        .join("target")
        .join("wasm32-unknown-unknown")
        .join("release");
    assert!(
        path.exists() || RUST_TOOL_WASM_PATH.exists(),
        "Rust Wasm path {} does not exists",
        path.display()
    );
    path
});
// The location of compiled Wasm files if running from within the 'tests' crate generated by the
// cargo-casper tool, i.e. 'wasm/'.
static RUST_TOOL_WASM_PATH: Lazy<PathBuf> = Lazy::new(|| {
    env::current_dir()
        .expect("should get current working dir")
        .join("wasm")
});
// The location of compiled Wasm files if compiled from the Rust sources within the casper-node
// repo where `CARGO_TARGET_DIR` is set, i.e.
// '<CARGO_TARGET_DIR>/wasm32-unknown-unknown/release/'.
static MAYBE_CARGO_TARGET_DIR_WASM_PATH: Lazy<Option<PathBuf>> = Lazy::new(|| {
    let maybe_target = std::env::var("CARGO_TARGET_DIR").ok();
    maybe_target.as_ref().map(|path| {
        Path::new(path)
            .join("wasm32-unknown-unknown")
            .join("release")
    })
});
// The location of compiled Wasm files if compiled from the Rust sources within the casper-node
// repo, i.e. 'casper-node/target/wasm32-unknown-unknown/release/'.
#[cfg(feature = "use-as-wasm")]
static ASSEMBLY_SCRIPT_WORKSPACE_WASM_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let path = RUST_WORKSPACE_PATH.join("target_as");

    assert!(
        path.exists(),
        "AssemblyScript WASM path {} does not exist.",
        path.display()
    );
    path
});
static WASM_PATHS: Lazy<Vec<PathBuf>> = Lazy::new(get_compiled_wasm_paths);

/// Constructs a list of paths that should be considered while looking for a compiled wasm file.
fn get_compiled_wasm_paths() -> Vec<PathBuf> {
    let mut ret = vec![
        // Contracts compiled with typescript are tried first
        #[cfg(feature = "use-as-wasm")]
        ASSEMBLY_SCRIPT_WORKSPACE_WASM_PATH.clone(),
        RUST_WORKSPACE_WASM_PATH.clone(),
        RUST_TOOL_WASM_PATH.clone(),
    ];
    if let Some(cargo_target_dir_wasm_path) = &*MAYBE_CARGO_TARGET_DIR_WASM_PATH {
        ret.push(cargo_target_dir_wasm_path.clone());
    };
    ret
}

/// Reads a given compiled contract file based on path
pub fn read_wasm_file_bytes<T: AsRef<Path>>(contract_file: T) -> Vec<u8> {
    let mut attempted_paths = vec![];

    if contract_file.as_ref().is_relative() {
        // Find first path to a given file found in a list of paths
        for wasm_path in WASM_PATHS.iter() {
            let mut filename = wasm_path.clone();
            filename.push(contract_file.as_ref());
            if let Ok(wasm_bytes) = fs::read(&filename) {
                return wasm_bytes;
            }
            attempted_paths.push(filename);
        }
    }
    // Try just opening in case the arg is a valid path relative to current working dir, or is a
    // valid absolute path.
    if let Ok(wasm_bytes) = fs::read(contract_file.as_ref()) {
        return wasm_bytes;
    }
    attempted_paths.push(contract_file.as_ref().to_owned());

    let mut error_msg =
        "\nFailed to open compiled Wasm file.  Tried the following locations:\n".to_string();
    for attempted_path in attempted_paths {
        error_msg = format!("{}    - {}\n", error_msg, attempted_path.display());
    }

    panic!("{}\n", error_msg);
}

/// Returns an [`ExecConfig`].
pub fn create_exec_config(accounts: Vec<GenesisAccount>) -> ExecConfig {
    let wasm_config = *DEFAULT_WASM_CONFIG;
    let system_config = *DEFAULT_SYSTEM_CONFIG;
    let validator_slots = DEFAULT_VALIDATOR_SLOTS;
    let auction_delay = DEFAULT_AUCTION_DELAY;
    let locked_funds_period_millis = DEFAULT_LOCKED_FUNDS_PERIOD_MILLIS;
    let round_seigniorage_rate = DEFAULT_ROUND_SEIGNIORAGE_RATE;
    let unbonding_delay = DEFAULT_UNBONDING_DELAY;
    let genesis_timestamp_millis = DEFAULT_GENESIS_TIMESTAMP_MILLIS;
    let refund_handling = DEFAULT_REFUND_HANDLING;
    let fee_handling = DEFAULT_FEE_HANDLING;

    ExecConfigBuilder::default()
        .with_accounts(accounts)
        .with_wasm_config(wasm_config)
        .with_system_config(system_config)
        .with_validator_slots(validator_slots)
        .with_auction_delay(auction_delay)
        .with_locked_funds_period_millis(locked_funds_period_millis)
        .with_round_seigniorage_rate(round_seigniorage_rate)
        .with_unbonding_delay(unbonding_delay)
        .with_genesis_timestamp_millis(genesis_timestamp_millis)
        .with_refund_handling(refund_handling)
        .with_fee_handling(fee_handling)
        .build()
}

/// Returns a [`GenesisConfig`].
pub fn create_genesis_config(accounts: Vec<GenesisAccount>) -> GenesisConfig {
    let name = DEFAULT_CHAIN_NAME.to_string();
    let timestamp = DEFAULT_GENESIS_TIMESTAMP_MILLIS;
    let protocol_version = *DEFAULT_PROTOCOL_VERSION;
    let exec_config = create_exec_config(accounts);

    GenesisConfig::new(name, timestamp, protocol_version, exec_config)
}

/// Returns a [`RunGenesisRequest`].
pub fn create_run_genesis_request(accounts: Vec<GenesisAccount>) -> RunGenesisRequest {
    let exec_config = create_exec_config(accounts);
    RunGenesisRequest::new(
        *DEFAULT_GENESIS_CONFIG_HASH,
        *DEFAULT_PROTOCOL_VERSION,
        exec_config,
        DEFAULT_CHAINSPEC_REGISTRY.clone(),
    )
}

/// Returns a `Vec<Gas>` representing gas consts for an [`ExecutionResult`].
pub fn get_exec_costs<T: AsRef<ExecutionResult>, I: IntoIterator<Item = T>>(
    exec_response: I,
) -> Vec<Gas> {
    exec_response
        .into_iter()
        .map(|res| res.as_ref().cost())
        .collect()
}

/// Returns the success result of the `ExecutionResult`.
/// # Panics
/// Panics if `response` is `None`.
pub fn get_success_result(response: &[Rc<ExecutionResult>]) -> &ExecutionResult {
    response.get(0).expect("should have a result")
}

/// Returns an error if the `ExecutionResult` has an error.
/// # Panics
/// Panics if the result is `None`.
/// Panics if the result does not have a precondition failure.
/// Panics if result.as_error() is `None`.
pub fn get_precondition_failure(response: &[Rc<ExecutionResult>]) -> &Error {
    let result = response.get(0).expect("should have a result");
    assert!(
        result.has_precondition_failure(),
        "should be a precondition failure"
    );
    result.as_error().expect("should have an error")
}

/// Returns a `String` concatenated from all of the error messages from the `ExecutionResult`.
pub fn get_error_message<T: AsRef<ExecutionResult>, I: IntoIterator<Item = T>>(
    execution_result: I,
) -> String {
    let errors = execution_result
        .into_iter()
        .enumerate()
        .filter_map(|(i, result)| {
            if let ExecutionResult::Failure { error, .. } = result.as_ref() {
                Some(format!("{}: {:?}", i, error))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    errors.join("\n")
}

/// Returns `Option<Account>`.
#[allow(clippy::implicit_hasher)]
pub fn get_account(transforms: &AdditiveMap<Key, Transform>, account: &Key) -> Option<Account> {
    transforms.get(account).and_then(|transform| {
        if let Transform::Write(StoredValue::Account(account)) = transform {
            Some(account.to_owned())
        } else {
            None
        }
    })
}
