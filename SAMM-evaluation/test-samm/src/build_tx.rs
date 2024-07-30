// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
use sui_config::{
    sui_config_dir, SUI_KEYSTORE_FILENAME,
};
use sui_json::SuiJsonValue;
use sui_json_rpc_types::{SuiTransactionBlockResponse, SuiTypeTag};
use sui_keys::keystore::{AccountKeystore, FileBasedKeystore};
use sui_sdk::types::crypto::Signature;
use serde_json::Value;
use shared_crypto::intent::Intent;
use sui_sdk::types::{
    base_types::{ObjectID, SuiAddress},
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{Transaction, TransactionData},
};

use sui_sdk::{rpc_types::SuiTransactionBlockResponseOptions, SuiClient};

use std::path::PathBuf;
use sui_move_build::BuildConfig;

// The struct that send transactions
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


// The struct that send our test transactions
// More functions than the original TestTransactionSender
#[derive(Clone)]
pub struct TestTransactionSender {
    pub sender: SuiAddress,
    pub gas_object: ObjectID,
    pub client: SuiClient,
}

struct PublishData {
    path: PathBuf,
    /// Whether to publish unpublished dependencies in the same transaction or not.
    with_unpublished_deps: bool,
}


fn convert_number_to_string(value: Value) -> Value {
    match value {
        Value::Number(n) => Value::String(n.to_string()),
        Value::Array(a) => Value::Array(a.into_iter().map(convert_number_to_string).collect()),
        Value::Object(o) => Value::Object(
            o.into_iter()
                .map(|(k, v)| (k, convert_number_to_string(v)))
                .collect(),
        ),
        _ => value,
    }
}


impl TestTransactionSender{
    pub fn new(sender: SuiAddress, gas_object: ObjectID, client: SuiClient) -> Self {
        Self {
            sender,
            gas_object,
            client,
        }
    }
    // Submit the transaction and its signature
    pub async fn submit_tx(self, sig: Signature, tx_data: TransactionData)-> Result<SuiTransactionBlockResponse, anyhow::Error>
    {
        let transaction_response  = self.client
            .quorum_driver_api()
            .execute_transaction_block(
                Transaction::from_data(tx_data, Intent::sui_transaction(), vec![sig]),
                SuiTransactionBlockResponseOptions::new().with_object_changes().with_effects(),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await?;
        Ok(transaction_response)
    }
    // a move call with signed transaction (not sent)
    pub async fn move_call_before_submit(self, package_object_id:ObjectID, module:&str, function: &str, type_args: Vec<SuiTypeTag>, call_args: Vec<SuiJsonValue>) -> Result<DataAndSender, anyhow::Error>
    {
        let args = call_args
            .into_iter()
            .map(|value| SuiJsonValue::new(convert_number_to_string(value.to_json_value())))
            .collect::<Result<_, _>>()?;

        let type_args = type_args
            .into_iter()
            .map(|arg| arg.try_into())
            .collect::<Result<Vec<_>, _>>()?;        
        let tx_data = self.client
        .transaction_builder()
        .move_call(self.sender, package_object_id, module, function, type_args, args, Some(self.gas_object), 190000000)
        .await?;
        let keystore = FileBasedKeystore::new(&sui_config_dir()?.join(SUI_KEYSTORE_FILENAME))?;
        let signature: Signature = keystore.sign_secure(&self.sender, &tx_data, Intent::sui_transaction())?;
        Ok(DataAndSender::new(signature, self.clone(), tx_data))
    }

    pub async fn sign_and_send(self, tx_data: TransactionData) -> Result<SuiTransactionBlockResponse, anyhow::Error>
    {
        let keystore = FileBasedKeystore::new(&sui_config_dir()?.join(SUI_KEYSTORE_FILENAME))?;
        let signature: Signature = keystore.sign_secure(&self.sender, &tx_data, Intent::sui_transaction())?;

        let transaction_response  = self.client
            .quorum_driver_api()
            .execute_transaction_block(
                Transaction::from_data(tx_data, Intent::sui_transaction(), vec![signature]),
                SuiTransactionBlockResponseOptions::new().with_object_changes().with_effects(),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await?;
        Ok(transaction_response)
    }

    pub async fn transfer_sui(self, gas_object_id: ObjectID) -> Result<SuiTransactionBlockResponse, anyhow::Error>
    {
        let recipient = "0x727b37454ab13d5c1dbb22e8741bff72b145d1e660f71b275c01f24e7860e5e5".parse::<SuiAddress>()?;
        // Create a sui transfer transaction
        let transfer_tx = self.client
            .transaction_builder()
            .transfer_sui(self.sender, gas_object_id, 500000000, recipient, Some(1000))
            .await?;
        let transaction_response = self.sign_and_send(transfer_tx).await?;
        Ok(transaction_response)
    }

    // publish the move package
    pub async fn publish_package(self, package_str: &'static str) -> Result<SuiTransactionBlockResponse, anyhow::Error>
    {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.extend(["..", package_str]);
        let data = PublishData {
            path,
            with_unpublished_deps: false,
        };
        let compiled_package = BuildConfig::new_for_testing().build(data.path).unwrap();
        let all_module_bytes =
            compiled_package.get_package_bytes(data.with_unpublished_deps);
        let dependencies = compiled_package.get_dependency_original_package_ids();

        let tx_data = self.client
        .transaction_builder()
        .publish(self.sender, all_module_bytes, dependencies, Some(self.gas_object), 500000000)
        .await?;
        let transaction_response = self.sign_and_send(tx_data).await?;
        Ok(transaction_response)
    }

    pub async fn move_call(self, package_object_id:ObjectID, module:&str, function: &str, type_args: Vec<SuiTypeTag>, call_args: Vec<SuiJsonValue>) -> Result<SuiTransactionBlockResponse, anyhow::Error>
    {
        let args = call_args
            .into_iter()
            .map(|value| SuiJsonValue::new(convert_number_to_string(value.to_json_value())))
            .collect::<Result<_, _>>()?;

        let type_args = type_args
            .into_iter()
            .map(|arg| arg.try_into())
            .collect::<Result<Vec<_>, _>>()?;        
        let tx_data = self.client
        .transaction_builder()
        .move_call(self.sender, package_object_id, module, function, type_args, args, Some(self.gas_object), 500000000)
        .await?;
        let transaction_response = self.sign_and_send(tx_data).await?;
        Ok(transaction_response)
    }


    // spit a coin into split_count coins with equal amount
    pub async fn split_coin_equal(self, coin_object_id:ObjectID, split_count: u64) -> Result<SuiTransactionBlockResponse, anyhow::Error>
    {
        let tx_data = self.client
        .transaction_builder()
        .split_coin_equal(self.sender, coin_object_id, split_count, Some(self.gas_object), 5000000000)
        .await?;
        let transaction_response = self.sign_and_send(tx_data).await?;
        Ok(transaction_response)
    }

}

