use sui_config::{
    sui_config_dir, Config, PersistedConfig, SUI_CLIENT_CONFIG, SUI_KEYSTORE_FILENAME,
};
use sui_keys::keystore::{AccountKeystore, FileBasedKeystore};
use sui_sdk::{
    sui_client_config::{SuiClientConfig, SuiEnv},
    wallet_context::WalletContext,
};
use tracing::info;

use sui_sdk::types::{
    base_types::SuiAddress,
    crypto::SignatureScheme::ED25519
};

use sui_sdk::{SuiClient, SuiClientBuilder};

// if you use the sui-test-validator and use the local network; if it does not work, try with port 5003.
// const SUI_FAUCET: &str = "http://127.0.0.1:9123/gas";


/// Return a sui client to interact with the APIs and an active address from the local wallet.
///
/// This function sets up a wallet in case there is no wallet locally,
/// and ensures that the active address of the wallet has SUI on it.
/// If there is no SUI owned by the active address, then it will request
/// SUI from the faucet.
pub async fn client_info() -> Result<(SuiClient, SuiAddress), anyhow::Error> {
    // let client = SuiClientBuilder::default().build_testnet().await?;

    let client = SuiClientBuilder::default().max_concurrent_requests(500_000).build_localnet().await?;
    // let client = SuiClientBuilder::default().max_concurrent_requests(500_000).build("http://132.68.60.223:9200").await?;

    
    // println!("Sui localnet version is: {}", client.api_version());
    let mut wallet = retrieve_wallet().await?;
    assert!(wallet.get_addresses().len() >= 2);
    let active_address = wallet.active_address()?;

    // println!("Wallet active address is: {active_address}");
    Ok((client, active_address))
}

pub async fn retrieve_wallet() -> Result<WalletContext, anyhow::Error> {
    let wallet_conf = sui_config_dir()?.join(SUI_CLIENT_CONFIG);
    let keystore_path = sui_config_dir()?.join(SUI_KEYSTORE_FILENAME);

    // check if a wallet exists and if not, create a wallet and a sui client config
    if !keystore_path.exists() {
        let keystore = FileBasedKeystore::new(&keystore_path)?;
        keystore.save()?;
    }

    if !wallet_conf.exists() {
        let keystore = FileBasedKeystore::new(&keystore_path)?;
        let mut client_config = SuiClientConfig::new(keystore.into());

        client_config.add_env(SuiEnv::testnet());
        client_config.add_env(SuiEnv::devnet());
        client_config.add_env(SuiEnv::localnet());

        if client_config.active_env.is_none() {
            client_config.active_env = client_config.envs.first().map(|env| env.alias.clone());
        }

        client_config.save(&wallet_conf)?;
        info!("Client config file is stored in {:?}.", &wallet_conf);
    }

    let mut keystore = FileBasedKeystore::new(&keystore_path)?;
    let mut client_config: SuiClientConfig = PersistedConfig::read(&wallet_conf)?;

    let default_active_address = if let Some(address) = keystore.addresses().first() {
        *address
    } else {
        keystore.generate_and_add_new_key(ED25519, None, None)?.0
    };

    if keystore.addresses().len() < 2 {
        keystore.generate_and_add_new_key(ED25519, None, None)?;
    }

    client_config.active_address = Some(default_active_address);
    client_config.save(&wallet_conf)?;

    let wallet =
        WalletContext::new(&wallet_conf, Some(std::time::Duration::from_secs(60)), None).await?;

    Ok(wallet)
}