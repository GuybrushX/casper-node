use casper_engine_test_support::{
    DeployItemBuilder, ExecuteRequestBuilder, InMemoryWasmTestBuilder, UpgradeRequestBuilder,
    DEFAULT_ACCOUNT_ADDR, DEFAULT_ACCOUNT_INITIAL_BALANCE, DEFAULT_ACCOUNT_PUBLIC_KEY,
    DEFAULT_CHAINSPEC_REGISTRY, DEFAULT_EXEC_CONFIG, DEFAULT_GENESIS_CONFIG,
    DEFAULT_GENESIS_CONFIG_HASH, DEFAULT_PAYMENT, DEFAULT_PROTOCOL_VERSION,
    DEFAULT_RUN_GENESIS_REQUEST,
};
use casper_execution_engine::core::engine_state::genesis::{
    ChainspecRegistry, CHAINSPEC_RAW, GENESIS_ACCOUNTS_RAW, GLOBAL_STATE_RAW,
};
use casper_execution_engine::core::engine_state::{
    run_genesis_request::RunGenesisRequest, EngineConfig, GenesisAccount,
};
use casper_hashing::Digest;
use casper_types::bytesrepr::ToBytes;
use casper_types::{CLType, CLValue, EraId, Key, ProtocolVersion};
use once_cell::sync::Lazy;
use rand::Rng;

static OLD_PROTOCOL_VERSION: Lazy<ProtocolVersion> = Lazy::new(|| *DEFAULT_PROTOCOL_VERSION);
static NEW_PROTOCOL_VERSION: Lazy<ProtocolVersion> = Lazy::new(|| {
    ProtocolVersion::from_parts(
        OLD_PROTOCOL_VERSION.value().major,
        OLD_PROTOCOL_VERSION.value().minor,
        OLD_PROTOCOL_VERSION.value().patch + 1,
    )
});

const DEFAULT_ACTIVATION_POINT: EraId = EraId::new(1);

#[ignore]
#[test]
fn should_commit_chainspec_registry_during_genesis() {
    let mut rng = rand::thread_rng();
    let chainspec_bytes_hash = Digest::hash(rng.gen::<[u8; 32]>());
    let genesis_account_hash = Digest::hash(rng.gen::<[u8; 32]>());

    let mut chainspec_registry = ChainspecRegistry::new();
    chainspec_registry.insert(CHAINSPEC_RAW.to_string(), chainspec_bytes_hash);
    chainspec_registry.insert(GENESIS_ACCOUNTS_RAW.to_string(), genesis_account_hash);

    let run_genesis_request = RunGenesisRequest::new(
        *DEFAULT_GENESIS_CONFIG_HASH,
        *DEFAULT_PROTOCOL_VERSION,
        DEFAULT_EXEC_CONFIG.clone(),
        chainspec_registry.clone(),
    );

    let mut builder = InMemoryWasmTestBuilder::default();
    builder.run_genesis(&run_genesis_request);

    let queried_registry = builder
        .query(None, Key::ChainspecRegistry, &vec![])
        .expect("must have entry under Key::ChainspecRegistry")
        .as_cl_value()
        .expect("must have underlying cl_value")
        .to_owned()
        .into_t::<ChainspecRegistry>()
        .expect("must convert to chainspec registry");

    let queried_chainspec_hash = queried_registry
        .get(CHAINSPEC_RAW)
        .expect("must have entry for chainspec_hash");

    assert_eq!(*queried_chainspec_hash, chainspec_bytes_hash);

    let queried_accounts_hash = queried_registry
        .get(GENESIS_ACCOUNTS_RAW)
        .expect("must have entry for genesis accounts");

    assert_eq!(*queried_accounts_hash, genesis_account_hash);
}

#[ignore]
#[test]
#[should_panic]
fn should_fail_to_commit_genesis_when_missing_genesis_accounts_hash() {
    let mut rng = rand::thread_rng();
    let chainspec_bytes_hash = Digest::hash(rng.gen::<[u8; 32]>());

    let mut incomplete_chainspec_registry = ChainspecRegistry::new();
    incomplete_chainspec_registry.insert(CHAINSPEC_RAW.to_string(), chainspec_bytes_hash);

    let run_genesis_request = RunGenesisRequest::new(
        *DEFAULT_GENESIS_CONFIG_HASH,
        *DEFAULT_PROTOCOL_VERSION,
        DEFAULT_EXEC_CONFIG.clone(),
        incomplete_chainspec_registry.clone(),
    );

    let mut builder = InMemoryWasmTestBuilder::default();
    builder.run_genesis(&run_genesis_request);
}

#[ignore]
#[test]
fn should_write_chainspec_registry_during_an_upgrade() {
    let mut rng = rand::thread_rng();

    let mut builder = InMemoryWasmTestBuilder::default();
    builder.run_genesis(&DEFAULT_RUN_GENESIS_REQUEST);

    let mut upgraded_chainspec_registry = ChainspecRegistry::new();
    let chainspec_bytes_hash = Digest::hash(rng.gen::<[u8; 32]>());
    upgraded_chainspec_registry.insert(CHAINSPEC_RAW.to_string(), chainspec_bytes_hash);
    let global_state_toml_hash = Digest::hash(rng.gen::<[u8; 32]>());
    upgraded_chainspec_registry.insert(GLOBAL_STATE_RAW.to_string(), global_state_toml_hash);

    let mut upgrade_request = {
        UpgradeRequestBuilder::new()
            .with_current_protocol_version(*OLD_PROTOCOL_VERSION)
            .with_new_protocol_version(*NEW_PROTOCOL_VERSION)
            .with_activation_point(DEFAULT_ACTIVATION_POINT)
            .with_chainspec_registry(upgraded_chainspec_registry)
            .build()
    };

    let engine_config = EngineConfig::default();

    builder.upgrade_with_upgrade_request(engine_config, &mut upgrade_request);

    let queried_registry = builder
        .query(None, Key::ChainspecRegistry, &vec![])
        .expect("must have entry under Key::ChainspecRegistry")
        .as_cl_value()
        .expect("must have underlying cl_value")
        .to_owned()
        .into_t::<ChainspecRegistry>()
        .expect("must convert to chainspec registry");

    // There should be no entry for the genesis accounts once the upgrade has completed.
    assert!(queried_registry.get(GENESIS_ACCOUNTS_RAW).is_none());

    let queried_chainspec_hash = queried_registry
        .get(CHAINSPEC_RAW)
        .expect("must have entry for chainspec_hash");

    assert_eq!(*queried_chainspec_hash, chainspec_bytes_hash);

    let queried_global_state_toml_hash = queried_registry
        .get(GLOBAL_STATE_RAW)
        .expect("must have entry for genesis accounts");

    assert_eq!(*queried_global_state_toml_hash, global_state_toml_hash);
}
