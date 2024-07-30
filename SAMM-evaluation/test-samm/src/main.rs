mod faucet;
use faucet::{get_gas_obj, get_gas_obj_one_layer}; 
mod get_client;
use get_client::client_info;
mod build_tx;
use build_tx::TestTransactionSender;
mod build_contract;
use build_contract::{ContractInfo, samm_data_builder};
mod execution;
use execution::execution_pool_new;
use tokio::time::Duration;

use sui_sdk::types::base_types::ObjectID;
use std::io::{self, Write};
use chrono::Local;
use std::fs::File;
use std::path::PathBuf;
use std::process;
use tokio::time::sleep;

pub const SUI_FAUCET: &str = "http://127.0.0.1:9123/gas";

pub const SUI_FAUCET_STATUS: &str = "http://127.0.0.1:9123/gas/status";

// Modify the following constants to change the test times
pub const time_warm_up:f64 = 500.0;
pub const time_test:f64 = 100.0;
pub const time_cool_down:f64 = 50.0;

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


fn start_command() -> process::Child {
    process::Command::new("../../sui/target/release/sui-test-validator")
        .env("RUST_LOG", "consensus=off")
        .arg("--config-dir")
        .arg("suilog")
        .arg("--epoch-duration-ms")
        .arg("999999999")
        .spawn()
        .expect("Failed to start command")
}

fn genesis() -> Result<(), anyhow::Error>
{
    let output1 = process::Command::new("sh")
        .arg("-c")
        .arg("rm -rf suilog/*")
        .output()
        .expect("failed to execute process");

    if output1.status.success() {
        println!("Files deleted successfully");
    } else {
        eprintln!("Error deleting files");
    }
    // let working_dir = current_dir.join("../suilog");
    // let working_dir_str = working_dir.to_str().ok_or("Failed to convert working_dir to str");
    // let output2 = process::Command::new("sh")
    // .arg("-c")
    // .arg("../../sui/target/release/sui genesis -f --with-faucet --working-dir=/home/hongyin/suilog")
    // .output()
    // .expect("failed to execute process");
    let output2 = process::Command::new("sh")
        .arg("-c")
        // .arg(format!("../../sui/target/release/sui genesis -f --with-faucet --working-dir={}",working_dir_str))
        .arg(format!("../../sui/target/release/sui genesis -f --with-faucet --working-dir=suilog"))
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
    let num_clients: usize = input_integer("Please input num_clients: ");
    let mut min_tps: usize = input_integer("Please input min_tps: ");
    let max_tps: usize = input_integer("Please input the max_tps: ");
    let tps_interval: usize = input_integer("Please input tps_interval: ");
    let num_repeat: usize = input_integer("Please input num_repeat: ");
    let num_groups: usize = input_integer("Please input num_groups: ");



    let mut num_shards: Vec<usize> = Vec::new();
    for i in 0..num_groups {
        let tmp: usize = input_integer(&format!("Please input the number of contract in group {}: ", i));
        num_shards.push(tmp);
    }


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
        "The min_tps: {}",
        min_tps
    )
    .unwrap();
    writeln!(
        &mut info_file,
        "The max_tps: {}",
        max_tps
    )
    .unwrap();
    writeln!(&mut info_file, "The tps_interval: {}", tps_interval).unwrap();
    writeln!(
        &mut info_file,
        "The repeated time: {}",
        num_repeat
    )
    .unwrap();
    for ng in num_shards.clone() 
    {
        writeln!(&mut info_file, "Number of contracts: {}", ng).unwrap();
    }


    for i in 0..num_groups
    {
        // the expected tps in this test
        let mut current_frequency = min_tps;
        // path of the result file
        let mut result_path = PathBuf::from(&folder_name);
        let result_file_name = format!("output{}.txt", num_shards[i].clone());
        result_path.push(result_file_name);
        let mut result_file = File::create(&result_path)?;
        let mut result_raw_folder_path = PathBuf::from(&folder_name);
        result_raw_folder_path.push(format!("raw{}", num_shards[i].clone()));
        if let Err(e) = std::fs::create_dir_all(&result_raw_folder_path) {
            eprintln!("Failed to create folder: {}", e);
            return Err(anyhow::Error::msg("Failed to create folder"));
        }
        // According to chernoff bound, with higher tps, the possibility of sending multiple times higher than expected tps is lower
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
        while current_frequency <= max_tps
        {
            let mut this_success = 0 as usize;
            let mut this_fail = 0 as usize;
            let mut this_latency = 0.0;
            for t in 0..num_repeat
            {
                let this_num_contract = num_shards[i];
                println!("Start test round: {}", t);
                println!("Number of client: {}", num_clients);
                println!("Number of shards: {}", this_num_contract);
                println!("Expected TPS: {}", current_frequency);
                let mut sui_test_validator_process = reset_env().await?;
                let (client, active_address) = client_info().await?;
                let obj_list = get_gas_obj_one_layer(5, active_address).await?;
                let coin_str = &obj_list[0];
                let gas_object_id = coin_str.parse::<ObjectID>()?;
                let tps_interval = num_clients as f64 / current_frequency as f64;
                let this_multi_factor = multi_factor / this_num_contract as f64;
                let coin_each_client = (current_frequency as f64 * this_multi_factor*  (time_warm_up + time_cool_down + time_test) / num_clients as f64).ceil();
                // Build SAMM smart contract and the transaction queue
                let execution_queue = samm_data_builder(client.clone(), active_address, this_num_contract, num_clients, gas_object_id, coin_each_client as usize).await?;
                let mut raw_file_path = result_raw_folder_path.clone();
                raw_file_path.push(format!("{}-test{}", current_frequency, t));
                if let Err(e) = std::fs::create_dir_all(&raw_file_path) {
                    eprintln!("Failed to create folder: {}", e);
                    return Err(anyhow::Error::msg("Failed to create folder"));
                }
                println!("Execution start!");
                // Initiate trader clients and start the test
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
                println!("Test round: {} finished!", t);
                println!("Number of client: {}", num_clients);
                println!("Number of shards: {}", this_num_contract);
                println!("Expected TPS: {}", current_frequency);
                println!("Test time: {}", time_test);
                println!("Number of successful transactions: {}", result.success);
                println!("True TPS: {}", result.success as f64 / time_test);
                println!("Number of failed transactions: {}", result.fail);
                println!("Average latency: {}", result.average_latency);
                match sui_test_validator_process.kill() {
                    Ok(_) => println!("Command terminated."),
                    Err(e) => eprintln!("Failed to terminate command: {}", e),
                }
            }
            // to speedup
            // If the latency is small and the failure rate is small, we can increase the minimum tps for the next group
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
            let this_latency_ave = this_latency / num_repeat as f64;
            let exp_success = (num_clients * current_frequency) as f64  * time_test / ONE_MINUTE as f64;
            let success_ratio = (this_success as f64 / exp_success) / num_repeat as f64;
            if this_latency_ave < 1.75 && success_ratio > 0.8 && flag2s
            {
                min_tps = current_frequency;
            }
            if this_latency_ave >= 1.75
            {
                flag2s = false;
            }
            // If the latency is too large or the failure rate is too large, we can stop the test of this group
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