use futures::{future, stream::StreamExt};
use sui_config::{
    sui_config_dir, Config, PersistedConfig, SUI_CLIENT_CONFIG, SUI_KEYSTORE_FILENAME,
};
use sui_json::SuiJsonValue;
use sui_json_rpc_types::{Coin, SuiObjectDataOptions, SuiTransactionBlockResponse, SuiTypeTag};
use sui_keys::keystore::{AccountKeystore, FileBasedKeystore};
use sui_sdk::{
    sui_client_config::{SuiClientConfig, SuiEnv},
    wallet_context::WalletContext, types::crypto::Signature,
};
use tracing::info;
use std::time::{SystemTime, Duration};
use reqwest::Client;
use serde_json::{json, Value};
use shared_crypto::intent::Intent;
use sui_sdk::types::{
    signature,
    base_types::{ObjectID, SuiAddress},
    crypto::SignatureScheme::ED25519,
    digests::TransactionDigest,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{Argument, Command, Transaction, TransactionData},
};

use sui_sdk::{rpc_types::SuiTransactionBlockResponseOptions, SuiClient, SuiClientBuilder};

use sui_transaction_builder::{DataReader, TransactionBuilder};
use std::path::PathBuf;
use sui_move_build::BuildConfig;
use sui_test_transaction_builder;
use tokio::time::sleep;
// The struct that send transactions
use crate::build_tx::TestTransactionSender;


#[derive(Clone)]
pub struct DataAndSender
{
    pub sig: Signature,
    pub test_sender: TestTransactionSender,
    pub tx_data: TransactionData,
}

impl DataAndSender
{
    pub fn new(sig: Signature, test_sender: TestTransactionSender, tx_data: TransactionData) -> Self {
        Self {
            sig,
            test_sender,
            tx_data
        }
    }
    pub async fn submit_tx(self) -> Result<SuiTransactionBlockResponse, anyhow::Error>
    {
        Ok(self.test_sender.submit_tx(self.sig, self.tx_data).await?)
    }
}