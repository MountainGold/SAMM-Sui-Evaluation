mod faucet;
use faucet::{get_gas_obj, request_five_gas_obj, get_gas_obj_one_layer, get_and_and_split_gas_obj}; 
mod get_client;
use get_client::client_info;
mod build_tx;
use build_tx::TestTransactionSender;
mod build_contract;
use build_contract::{samm_builder,samm_testcoin_builder,ContractInfo, samm_data_builder};
mod execution;
use execution::{ExecutionReturn, execution_pool, execution_pool_new};
use tokio::time::{Duration, Instant};
use tokio::task;
use sui_json::SuiJsonValue;
use futures::{future, stream::StreamExt};
use sui_config::{
    sui_config_dir, Config, PersistedConfig, SUI_CLIENT_CONFIG, SUI_KEYSTORE_FILENAME,
};
use sui_json_rpc_types::{Coin, SuiObjectDataOptions, SuiTypeTag};
use sui_keys::keystore::{AccountKeystore, FileBasedKeystore};
use sui_sdk::{
    sui_client_config::{SuiClientConfig, SuiEnv},
    wallet_context::WalletContext, types::coin,
};
use tracing::info;
use std::thread;
use reqwest::Client;
use serde_json::json;
use shared_crypto::intent::Intent;
use sui_sdk::types::{
    base_types::{ObjectID, SuiAddress},
    crypto::SignatureScheme::ED25519,
    digests::TransactionDigest,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{Argument, Command, Transaction, TransactionData},
};
use std::time::SystemTime;
use sui_sdk::{rpc_types::SuiTransactionBlockResponseOptions, SuiClient, SuiClientBuilder};
use std::env;
use std::fs;
use std::path::Path;
use std::io::{self, Write};
use chrono::{Local, Timelike};
use std::fs::File;
use std::path::PathBuf;
use std::collections::HashSet;
use std::process;
use tokio::time::sleep;
use std::net::TcpListener;

pub const SUI_FAUCET: &str = "http://127.0.0.1:9123/gas";

pub const SUI_FAUCET_STATUS: &str = "http://127.0.0.1:9123/gas/status";



pub const ONE_MINUTE: f64 = 60.0;

fn input_integer(prompt: &str) -> usize {
    print!("{}", prompt);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Failed to read line");

    input.trim().parse().unwrap_or_else(|_| {
        println!("Please enter a valid number!");
        input_integer(prompt)
    })
}

fn input_float(prompt: &str) -> f64 {
    print!("{}", prompt);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Failed to read line");

    input.trim().parse().unwrap_or_else(|_| {
        println!("Please enter a valid number!");
        input_float(prompt)
    })
}

fn check_port(port: u16) -> bool {
    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(_) => false, // Port is available
        Err(_) => true, // Port is in use
    }
}

fn start_command() -> process::Child {
    process::Command::new("sui-test-validator")
        .env("RUST_LOG", "consensus=off")
        .arg("--config-dir")
        .arg("/home/hongyin/suilog")
        .arg("--epoch-duration-ms")
        .arg("999999999")
        .spawn()
        .expect("Failed to start command")
}

fn genesis() -> Result<(), anyhow::Error>
{
    let output1 = process::Command::new("sh")
        .arg("-c")
        .arg("rm -rf ~/suilog/*")
        .output()
        .expect("failed to execute process");

    if output1.status.success() {
        println!("Files deleted successfully");
    } else {
        eprintln!("Error deleting files");
    }

    let output2 = process::Command::new("sh")
    .arg("-c")
    .arg("sui genesis -f --with-faucet --working-dir=/home/hongyin/suilog")
    .output()
    .expect("failed to execute process");

    if output2.status.success() {
        println!("Genesis successful");
    } else {
        eprintln!("Error genesis!");
    }
    Ok(())
}

async fn reset_env() -> Result<process::Child, anyhow::Error>
{
    genesis()?;
    let duration = Duration::from_secs_f64(5.0);
    let mut sui_test_validator_process = start_command();
    loop
    {
        sleep(duration).await;
        match sui_test_validator_process.try_wait()
        {
            Ok(Some(status)) => {
                println!("Sui-test-validator failed, restart!");
                genesis()?;
                sui_test_validator_process = start_command();
            }
            Ok(None) => {
                println!("Sui-test-validator normally started, proceed!");
                break;
            }
            Err(e) => {
                println!("Fail to detect status, try again!");
            }
        }
    }

    sleep(duration).await;
    Ok(sui_test_validator_process)
}


#[tokio::main]
// #[tokio::main(flavor = "multi_thread", worker_threads = 2000)]
async fn main() -> Result<(), anyhow::Error> {

    let mut multi_factor: f64 = 5.0;
    // let test_obj = get_and_and_split_gas_obj(client.clone(), 10, active_address, 200).await?;
    // println!("{:?}",test_obj);


    // let exp_type: usize = input_integer("Please input the type of experiment: 1. swap 2. add liquidity 3. remove");
    let num_clients: usize = input_integer("Please input the number of clients: ");
    // let obj_per_client: usize = input_integer("Please input the number of gas objects per client: ");
    let mut min_tps_per_minute: usize = input_integer("Please input the min_tps_per_minute: ");
    let max_tps_per_minute: usize = input_integer("Please input the max_tps_per_minute: ");
    let tps_interval: usize = input_integer("Please input the tps_interval: ");
    let repeated_time: usize = input_integer("Please input the repeated time of test: ");
    let num_groups: usize = input_integer("Please input the number of groups time of test: ");


    let time_warm_up = 500.0;
    let time_test = 100.0;
    let time_cool_down = 50.0;
    // let max_paralell = obj_per_client / 2;


    let mut num_contracts: Vec<usize> = Vec::new();
    for i in 0..num_groups {
        let tmp: usize = input_integer(&format!("Please input the number of contract in group {}: ", i));
        num_contracts.push(tmp);
    }

    // let current_dir = env::current_dir()?;
    let folder_name = Local::now().format("%Y-%m-%d-%H-%M-%S").to_string();

    let mut info_file_path = PathBuf::from(&folder_name);
    info_file_path.push("info.txt");
    if let Err(e) = std::fs::create_dir_all(&folder_name) {
        eprintln!("Failed to create folder: {}", e);
        return Err(anyhow::Error::msg("Failed to create folder"));
    }
    
    let mut info_file = File::create(&info_file_path)?;

    
    writeln!(&mut info_file, "The number of clients: {}", num_clients).unwrap();
    writeln!(
        &mut info_file,
        "The min_tps_per_minute: {}",
        min_tps_per_minute
    )
    .unwrap();
    writeln!(
        &mut info_file,
        "The max_tps_per_minute: {}",
        max_tps_per_minute
    )
    .unwrap();
    writeln!(&mut info_file, "The tps_interval: {}", tps_interval).unwrap();
    writeln!(
        &mut info_file,
        "The repeated time: {}",
        repeated_time
    )
    .unwrap();
    for ng in num_contracts.clone() 
    {
        writeln!(&mut info_file, "Number of contracts: {}", ng).unwrap();
    }


    for i in 0..num_groups
    {
        // 
        let mut current_frequency = min_tps_per_minute;
        let mut result_path = PathBuf::from(&folder_name);
        let result_file_name = format!("output{}.txt", num_contracts[i].clone());
        result_path.push(result_file_name);
        let mut result_file = File::create(&result_path)?;


        let mut result_raw_folder_path = PathBuf::from(&folder_name);
        result_raw_folder_path.push(format!("raw{}", num_contracts[i].clone()));
        if let Err(e) = std::fs::create_dir_all(&result_raw_folder_path) {
            eprintln!("Failed to create folder: {}", e);
            return Err(anyhow::Error::msg("Failed to create folder"));
        }
        if current_frequency <= 100
        {
            multi_factor = 4.0;
        }
        else if current_frequency <= 200
        {
            multi_factor = 3.0;
        }
        else if current_frequency <= 300
        {
            multi_factor = 2.0;
        }
        else 
        {
            multi_factor = 1.2;    
        }
        let mut flag2s = true;
        while current_frequency <= max_tps_per_minute
        {
            let mut this_success = 0 as usize;
            let mut this_fail = 0 as usize;
            let mut this_latency = 0.0;
            for t in 0..repeated_time
            {
                let mut sui_test_validator_process = reset_env().await?;
                let (client, active_address) = client_info().await?;
                let this_num_contract = num_contracts[i];
                let obj_list = get_gas_obj_one_layer(5, active_address).await?;
                let coin_str = &obj_list[0];
                let gas_object_id = coin_str.parse::<ObjectID>()?;
                let tps_interval = ONE_MINUTE / current_frequency as f64;
                let this_multi_factor = multi_factor / this_num_contract as f64;
                let coin_each_client = (current_frequency as f64 * this_multi_factor*  (time_warm_up + time_cool_down + time_test) / ONE_MINUTE).ceil();
                let execution_queue = samm_data_builder(client.clone(), active_address, this_num_contract, num_clients, gas_object_id, coin_each_client as usize).await?;
                let mut raw_file_path = result_raw_folder_path.clone();
                raw_file_path.push(format!("{}-test{}", current_frequency, t));
                if let Err(e) = std::fs::create_dir_all(&raw_file_path) {
                    eprintln!("Failed to create folder: {}", e);
                    return Err(anyhow::Error::msg("Failed to create folder"));
                }
                //println!("{:?}", converted_coin_list[this_num_contract-1].clone());
                // println!("Contract finished.");
                // let mut input = String::new();
                // io::stdin().read_line(&mut input).expect("no input");
                println!("Execution start!");
                let result = execution_pool_new(num_clients, tps_interval, client.clone(), active_address, execution_queue, time_warm_up, time_cool_down, time_test, raw_file_path.clone()).await?;
                writeln!(
                    &mut result_file,
                    "{}, {}, {}, {}, 0",
                    current_frequency, result.success, result.average_latency, result.fail, 
                )
                .unwrap();
                this_success += result.success;
                this_fail += result.fail;
                this_latency += result.average_latency;
                println!("Number of client: {}", num_clients);
                println!("Number of contracts: {}", this_num_contract);
                println!("Frequency: {}", current_frequency);
                println!("Number of successful transactions: {}", result.success);
                let exp_throughput = num_clients * current_frequency * time_test as usize / ONE_MINUTE as usize;
                println!("Expected number of successful transactions: {}", exp_throughput);
                println!("Number of failed transactions: {}", result.fail);
                println!("Average latency: {}", result.average_latency);
                match sui_test_validator_process.kill() {
                    Ok(_) => println!("Command terminated."),
                    Err(e) => eprintln!("Failed to terminate command: {}", e),
                }
            }
            // to speedup
            // Don't use it during a normal test
            if this_success != 0
            {
                if this_fail as f64 / this_success as f64 > 0.1
                {
                    println!("Too much failures!");
                    break;
                }
            }
            else 
            {
                println!("No successful execution!");
                break;
            }
            let this_latency_ave = this_latency / repeated_time as f64;
            let exp_success = (num_clients * current_frequency) as f64  * time_test / ONE_MINUTE as f64;
            let success_ratio = (this_success as f64 / exp_success) / repeated_time as f64;
            if this_latency_ave < 1.75 && success_ratio > 0.8 && flag2s
            {
                min_tps_per_minute = current_frequency;
            }
            if this_latency_ave >= 1.75
            {
                flag2s = false;
            }
            if this_latency_ave > 10.0
            {
                println!("Too large lantencies!");
                break;
            }

            if success_ratio < 0.8
            {
                println!("Not enough successful executions!");
                break;
            }
            current_frequency += tps_interval;    
        }

    };

    Ok(())
}