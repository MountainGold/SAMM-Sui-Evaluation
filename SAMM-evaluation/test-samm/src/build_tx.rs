// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

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

    pub async fn sign_and_send_fake(self, tx_data: TransactionData) -> Result<SuiTransactionBlockResponse, anyhow::Error>
    {
        let keystore = FileBasedKeystore::new(&sui_config_dir()?.join(SUI_KEYSTORE_FILENAME))?;
        let signature: sui_sdk::types::crypto::Signature = keystore.sign_secure(&self.sender, &tx_data, Intent::sui_transaction())?;
        let signature2: sui_sdk::types::crypto::Signature = keystore.sign_secure(&self.sender, &tx_data, Intent::sui_transaction())?;
        if signature == signature2
        {
            let duration = Duration::from_secs_f64(1.0);
            sleep(duration).await;
        }
        else {
            let duration = Duration::from_secs_f64(1.01);
            sleep(duration).await;
        }
        let transaction_response = SuiTransactionBlockResponse::new(TransactionDigest::default());
        Ok(transaction_response)
        // let transaction_response  = self.client
        //     .quorum_driver_api()
        //     .execute_transaction_block(
        //         Transaction::from_data(tx_data, Intent::sui_transaction(), vec![signature]),
        //         SuiTransactionBlockResponseOptions::new().with_object_changes().with_effects(),
        //         Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        //     )
        //     .await?;
        
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


    pub async fn move_call_fake(self, package_object_id:ObjectID, module:&str, function: &str, type_args: Vec<SuiTypeTag>, call_args: Vec<SuiJsonValue>) -> Result<SuiTransactionBlockResponse, anyhow::Error>
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
        let transaction_response = self.sign_and_send_fake(tx_data).await?;
        Ok(transaction_response)
    }

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


// This example shows how to use programmable transactions to chain multiple
// actions into one transaction. Specifically, the example retrieves two addresses
// from the local wallet, and then
// 1) finds a coin from the active address that has Sui,
// 2) splits the coin into one coin of 1000 MIST and the rest,
// 3  transfers the split coin to second Sui address,
// 4) signs the transaction,
// 5) executes it.
// For some of these actions it prints some output.
// Finally, at the end of the program it prints the number of coins for the
// Sui address that received the coin.
// If you run this program several times, you should see the number of coins
// for the recipient address increases.

// #[allow(unused_assignments)]
// async fn build_and_send_tx(SuiClient: sui, SuiAddress: sender) -> Result<(), anyhow::Error> {
    
//     // 1) get the Sui client, the sender and recipient that we will use
//     // for the transaction, and find the coin we use as gas
//     let (sui, sender, recipient) = setup_for_write().await?;

//     // we need to find the coin we will use as gas
//     let coins = sui
//         .coin_read_api()
//         .get_coins(sender, None, None, None)
//         .await?;
//     let coin = coins.data.into_iter().next().unwrap();

//     // programmable transactions allows the user to bundle a number of actions into one transaction
//     let mut ptb = ProgrammableTransactionBuilder::new();

//     // 2) split coin
//     // the amount we want in the new coin, 1000 MIST
//     let split_coint_amount = ptb.pure(1000u64)?; // note that we need to specify the u64 type
//     ptb.command(Command::SplitCoins(
//         Argument::GasCoin,
//         vec![split_coint_amount],
//     ));

//     // 3) transfer the new coin to a different address
//     let argument_address = ptb.pure(recipient)?;
//     ptb.command(Command::TransferObjects(
//         vec![Argument::Result(0)],
//         argument_address,
//     ));

//     // finish building the transaction block by calling finish on the ptb
//     let builder = ptb.finish();

//     let gas_budget = 5_000_000;
//     let gas_price = sui.read_api().get_reference_gas_price().await?;
//     // create the transaction data that will be sent to the network
//     let tx_data = TransactionData::new_programmable(
//         sender,
//         vec![coin.object_ref()],
//         builder,
//         gas_budget,
//         gas_price,
//     );

//     // 4) sign transaction
//     let keystore = FileBasedKeystore::new(&sui_config_dir()?.join(SUI_KEYSTORE_FILENAME))?;
//     let signature = keystore.sign_secure(&sender, &tx_data, Intent::sui_transaction())?;

//     // 5) execute the transaction
//     print!("Executing the transaction...");
//     let transaction_response = sui
//         .quorum_driver_api()
//         .execute_transaction_block(
//             Transaction::from_data(tx_data, Intent::sui_transaction(), vec![signature]),
//             SuiTransactionBlockResponseOptions::full_content(),
//             Some(ExecuteTransactionRequestType::WaitForLocalExecution),
//         )
//         .await?;
//     print!("done\n Transaction information: ");
//     println!("{:?}", transaction_response);

//     let coins = sui
//         .coin_read_api()
//         .get_coins(recipient, None, None, None)
//         .await?;

//     println!(
//         "After the transfer, the recipient address {recipient} has {} coins",
//         coins.data.len()
//     );
//     Ok(())
// }


