use crate::TestTransactionSender;
use crate::get_gas_obj;
use crate::get_gas_obj_one_layer;
use crate::build_tx::DataAndSender;
use crate::ContractInfo;
use anyhow::Ok;
use tokio;
use sui_json_rpc_types::SuiTransactionBlockResponse;
use std::thread;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::error::Error;
use sui_json::SuiJsonValue;
use futures::{future, stream::StreamExt};
use sui_config::{
    sui_config_dir, Config, PersistedConfig, SUI_CLIENT_CONFIG, SUI_KEYSTORE_FILENAME,
};
use tokio::time::sleep;
use sui_json_rpc_types::SuiTransactionBlockEffectsAPI;
use sui_json_rpc_types::{Coin, SuiObjectDataOptions, SuiTypeTag, ObjectChange, SuiExecutionStatus};
use sui_keys::keystore::{AccountKeystore, FileBasedKeystore};
use sui_sdk::types::object::Object;
use sui_sdk::{
    sui_client_config::{SuiClientConfig, SuiEnv},
    wallet_context::WalletContext, types::coin,
};
use tracing::info;

use reqwest::Client;
use serde_json::json;
use sui_sdk::types::{
    base_types::{ObjectID, SuiAddress},
    crypto::SignatureScheme::ED25519,
    digests::TransactionDigest,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{Argument, Command, Transaction, TransactionData},
};

use sui_sdk::{rpc_types::SuiTransactionBlockResponseOptions, SuiClient, SuiClientBuilder};
use std::time::{SystemTime, Duration};
use std::path::Path;
use std::path::PathBuf;
use rand::prelude::*;
use rand_distr::Exp;
use rand::rngs::StdRng;
use tokio::task::JoinHandle;
use std::collections::HashSet;
use tokio::runtime::Builder;

#[derive(Clone)]
pub struct ExecutionReturn
{
    pub if_sucess: usize,
    pub start_time: f64,
    pub end_time: f64,
}
impl ExecutionReturn{
    pub fn new(if_sucess: usize, start_time: f64, end_time: f64) -> Self {
        Self {
            if_sucess,
            start_time,
            end_time,
        }
    }
}

pub struct ExperimentReturn
{
    pub success: usize,
    pub fail: usize,
    pub average_latency: f64,
}
impl ExperimentReturn{
    pub fn new(success: usize, fail: usize, average_latency: f64) -> Self {
        Self {
            success,
            fail,
            average_latency,
        }
    }
}

async fn call_swap(client: SuiClient, sender: SuiAddress, contract_info: ContractInfo, 
    gas_obj: ObjectID, coin_id: ObjectID, origin_time: SystemTime)
    -> Result<ExecutionReturn, anyhow::Error>
{
    let USDT_id = format!("{}::coins::USDT",contract_info.coin_package).to_string();
    let XBTC_id = format!("{}::coins::XBTC",contract_info.coin_package).to_string();
    let module = "interface";
    let function = "swap";
    let type_args = vec![
        SuiTypeTag::new(XBTC_id),
        SuiTypeTag::new(USDT_id),        
    ];
    let call_args = vec![
        contract_info.global.to_string().parse::<SuiJsonValue>()?,        
        coin_id.to_string().parse::<SuiJsonValue>()?,
        "1".to_string().parse::<SuiJsonValue>()?,
        ];
   
    let test_transaction_sender = TestTransactionSender::new(sender, gas_obj, client);
    let start_time = SystemTime::now().duration_since(origin_time)?.as_secs_f64();
    // for test
    // let duration = Duration::from_secs_f64(1.0);
    // sleep(duration).await;
    // let end_time = SystemTime::now().duration_since(origin_time)?.as_secs_f64();
    // Ok(ExecutionReturn::new(1,start_time,end_time))
    
    // let transaction_response: sui_json_rpc_types::SuiTransactionBlockResponse = 
    // test_transaction_sender.move_call(contract_info.packageid, module, function, type_args, call_args).await?;

    let transaction_response_result: Result<sui_json_rpc_types::SuiTransactionBlockResponse, anyhow::Error> = 
    test_transaction_sender.move_call(contract_info.packageid, module, function, type_args, call_args).await;
    let transaction_response = transaction_response_result.unwrap_or_else(|err| {
        // eprintln!("Error: {}", err);
        SuiTransactionBlockResponse::default()
    });
    let mut if_success;
    if transaction_response.clone().effects.is_none()
    {
        if_success = 0;
    }
    else 
    {
        if_success = if transaction_response.clone().effects.unwrap().status().is_ok() 
        {
            1 as usize
        }
        else
        {
            0 as usize
        };
    }

    // println!("{:?}", transaction_response);

    let end_time = SystemTime::now().duration_since(origin_time)?.as_secs_f64();
    
    Ok(ExecutionReturn::new(if_success,start_time,end_time))
}


pub async fn call_swap_before_submit(client: SuiClient, sender: SuiAddress, contract_info: ContractInfo, 
    gas_obj: ObjectID, coin_id: ObjectID)
    -> Result<DataAndSender, anyhow::Error>
{
    let USDT_id = format!("{}::coins::USDT",contract_info.coin_package).to_string();
    let XBTC_id = format!("{}::coins::XBTC",contract_info.coin_package).to_string();
    let module = "interface";
    let function = "swap";
    let type_args = vec![
        SuiTypeTag::new(XBTC_id),
        SuiTypeTag::new(USDT_id),        
    ];
    let call_args = vec![
        contract_info.global.to_string().parse::<SuiJsonValue>()?,        
        coin_id.to_string().parse::<SuiJsonValue>()?,
        "100000".to_string().parse::<SuiJsonValue>()?,
        ];
   
    let test_transaction_sender = TestTransactionSender::new(sender, gas_obj, client);
    let data_from_response = 
    test_transaction_sender.move_call_before_submit(contract_info.packageid, module, function, type_args, call_args).await?;
    Ok(data_from_response)
}

async fn call_swap_fake(client: SuiClient, sender: SuiAddress, contract_info: ContractInfo, 
    gas_obj: ObjectID, coin_id: ObjectID, origin_time: SystemTime)
    -> Result<ExecutionReturn, anyhow::Error>
{
    let USDT_id = format!("{}::coins::USDT",contract_info.coin_package).to_string();
    let XBTC_id = format!("{}::coins::XBTC",contract_info.coin_package).to_string();
    let module = "interface";
    let function = "swap";
    let type_args = vec![
        SuiTypeTag::new(XBTC_id),
        SuiTypeTag::new(USDT_id),        
    ];
    let call_args = vec![
        contract_info.global.to_string().parse::<SuiJsonValue>()?,        
        coin_id.to_string().parse::<SuiJsonValue>()?,
        "1".to_string().parse::<SuiJsonValue>()?,
        ];
   
    let test_transaction_sender = TestTransactionSender::new(sender, gas_obj, client);
    let start_time = SystemTime::now().duration_since(origin_time)?.as_secs_f64();
    // for test
    // let duration = Duration::from_secs_f64(1.0);
    // sleep(duration).await;
    // let end_time = SystemTime::now().duration_since(origin_time)?.as_secs_f64();
    // Ok(ExecutionReturn::new(1,start_time,end_time))
    
    let transaction_response: sui_json_rpc_types::SuiTransactionBlockResponse = 
    test_transaction_sender.move_call_fake(contract_info.packageid, module, function, type_args, call_args).await?;
    // println!("{:?}", transaction_response);
    // let if_success = if transaction_response.clone().effects.unwrap().status().is_ok() 
    // {
    //     1 as usize
    // }
    // else
    // {
    //     0 as usize
    // };
    let if_success = 1;
    let end_time = SystemTime::now().duration_since(origin_time)?.as_secs_f64();
    
    Ok(ExecutionReturn::new(if_success,start_time,end_time))
}


async fn execution_single(num_contract: usize, max_paralell: usize, expected_interval: f64, 
    folder_name: String, client: SuiClient, sender: SuiAddress, gas_obj_list: Vec<String>, 
    contract_info_list: Vec<ContractInfo>, coin_list: Vec<Vec<ObjectID>>, time1: f64, 
    time2: f64, time3: f64, origin_time: SystemTime)
-> Result<Vec<ExecutionReturn>, anyhow::Error>
{
    let mut coin_used_vec = vec![];
    let mut results = vec![];
    let mut tasks = vec![];

    let lambda = 1.0 / expected_interval; 

    // let mut rng = thread_rng();
    let mut rng = StdRng::from_entropy();
    let exp = Exp::new(lambda).unwrap();

    for i in 0..num_contract
    {
        coin_used_vec.push(0 as usize);
    }
    let mut st = SystemTime::now().duration_since(origin_time)?.as_secs_f64();
    let mut execution:usize = 0;
    while st < time3
    {

        let chosen_one = rng.gen_range(0..num_contract);
        let chosen_info = contract_info_list[chosen_one].clone();
        if coin_used_vec[chosen_one] >= coin_list[chosen_one].len()
        {
            continue;
        }
        let chosen_coin = coin_list[chosen_one][coin_used_vec[chosen_one]].clone();
        let gas_obj = gas_obj_list[execution % gas_obj_list.len()].clone();
        // let sender_clone = sender.clone();
        let client_clone = client.clone();
        // let origin_time_clone = origin_time.clone();
        // let res = call_swap(client_clone, sender_clone, chosen_info, gas_obj.parse::<ObjectID>()?, chosen_coin, origin_time_clone).await?;
        // let res = call_swap(client_clone, sender_clone, chosen_info, 
        //     gas_obj.parse::<ObjectID>()?, chosen_coin, origin_time_clone).await.unwrap_or_else(|err| 
        //         {
        //             ExecutionReturn::new(0, 0.0, 0.0)
        //         });
        
        // let res = call_swap(client.clone(), sender, chosen_info, gas_obj.parse::<ObjectID>()?, chosen_coin, origin_time).await?;
        // results.push(res);
        // let task = call_swap(client.clone(), sender, chosen_info, gas_obj.parse::<ObjectID>()?, chosen_coin, origin_time);
        // let task= tokio::spawn(async move {
        //     call_swap(client_clone, sender, chosen_info, gas_obj.parse::<ObjectID>()?, chosen_coin, origin_time).await
        //     });
        // let runtime = Builder::new_multi_thread()
        // .worker_threads(2000)
        // .enable_all()
        // .build()
        // .unwrap();
        // let task = runtime.spawn(async move {
        //     call_swap(client_clone, sender, chosen_info, gas_obj.parse::<ObjectID>()?, chosen_coin, origin_time).await
        // });

        let task= tokio::spawn(async move {
            call_swap(client_clone, sender, chosen_info, gas_obj.parse::<ObjectID>()?, chosen_coin, origin_time).await
            });
        tasks.push(task);
        coin_used_vec[chosen_one] += 1;
        execution += 1;
        if execution > max_paralell
        {
            results.push(tasks.remove(0).await.unwrap_or_else(|err| 
                        {
                            Ok(ExecutionReturn::new(0, 0.0, 0.0))
                        }).unwrap());
        }
        let drawn_time = exp.sample(&mut rng);
        let next_time = drawn_time + st;
        if next_time > time3
        {
            break
        }
        let rest_time = next_time - SystemTime::now().duration_since(origin_time)?.as_secs_f64();
        if rest_time > 0.0
        {
            let duration = Duration::from_secs_f64(rest_time);
            sleep(duration).await;
        }
        st = SystemTime::now().duration_since(origin_time)?.as_secs_f64();
    }
    for tmp_task in tasks
    {
        results.push(tmp_task.await.unwrap_or_else(|err| 
            {
                Ok(ExecutionReturn::new(0, 0.0, 0.0))
            }).unwrap());
    }

    Ok(results)
}

async fn execution_single_backup(num_contract: usize, max_paralell: usize, expected_interval: f64, 
    folder_name: String, client: SuiClient, sender: SuiAddress, gas_obj_list: Vec<String>, 
    contract_info_list: Vec<ContractInfo>, coin_list: Vec<Vec<ObjectID>>, time1: f64, 
    time2: f64, time3: f64, origin_time: SystemTime)
-> Result<Vec<ExecutionReturn>, anyhow::Error>
{
    let mut coin_used_vec = vec![];
    let mut results = vec![];
    // let mut tasks = vec![];

    let lambda = 1.0 / expected_interval; 

    // let mut rng = thread_rng();
    let mut rng = StdRng::from_entropy();
    let exp = Exp::new(lambda).unwrap();

    for i in 0..num_contract
    {
        coin_used_vec.push(0 as usize);
    }
    let mut st = SystemTime::now().duration_since(origin_time)?.as_secs_f64();
    let mut execution:usize = 0;
    while st < time3
    {

        let chosen_one = rng.gen_range(0..num_contract);
        let chosen_info = contract_info_list[chosen_one].clone();
        if coin_used_vec[chosen_one] >= coin_list[chosen_one].len()
        {
            st = SystemTime::now().duration_since(origin_time)?.as_secs_f64();
            continue;
        }
        let chosen_coin = coin_list[chosen_one][coin_used_vec[chosen_one]].clone();
        let gas_obj = gas_obj_list[execution % max_paralell].clone();
        // let sender_clone = sender.clone();
        let client_clone = client.clone();
        let origin_time_clone = origin_time.clone();
        // let res = call_swap(client_clone, sender_clone, chosen_info, gas_obj.parse::<ObjectID>()?, chosen_coin, origin_time_clone).await?;
        let res = call_swap_fake(client_clone, sender, chosen_info, 
            gas_obj.parse::<ObjectID>()?, chosen_coin, origin_time_clone).await.unwrap_or_else(|err| 
                {
                    // println!("{:?}", err);
                    ExecutionReturn::new(0, st, st)
                });
        
        // let res = call_swap(client.clone(), sender, chosen_info, gas_obj.parse::<ObjectID>()?, chosen_coin, origin_time).await?;
        results.push(res);
        // let task = call_swap(client.clone(), sender, chosen_info, gas_obj.parse::<ObjectID>()?, chosen_coin, origin_time);
        // tasks.push(task);
        coin_used_vec[chosen_one] += 1;
        execution += 1;
        // if execution > max_paralell
        // {
        //     results.push(tasks.remove(0).await?);
        // }
        let drawn_time = exp.sample(&mut rng);
        let next_time = drawn_time + st;
        if next_time > time3
        {
            break
        }
        let rest_time = next_time - SystemTime::now().duration_since(origin_time)?.as_secs_f64();
        // println!("{}",drawn_time);
        if rest_time > 0.0
        {
            let duration = Duration::from_secs_f64(rest_time);
            //sleep(duration).await;
        }
        st = SystemTime::now().duration_since(origin_time)?.as_secs_f64();
    }
    // for tmp_task in tasks
    // {
    //     results.push(tmp_task.await?);
    // }

    Ok(results)
}



pub async fn execution_pool(num_client:usize, num_contract: usize, max_paralell: usize, expected_interval: f64, 
    folder_name: String, client: SuiClient, sender: SuiAddress, gas_obj_list: Vec<Vec<String>>, 
    contract_info_list: Vec<ContractInfo>, coin_list: Vec<Vec<Vec<ObjectID>>>, time_warm_up: f64, 
    time_cool_down: f64, time_test: f64)
    -> Result<ExperimentReturn, anyhow::Error>
{
    let mut total_latency = 0.0;
    let mut success = 0;
    let mut fail = 0;

    let origin_time = SystemTime::now();
    let time1 = time_warm_up;
    let time2 = time1 + time_test;
    let time3 = time2 + time_cool_down;
    let mut tasks = Vec::new();
    // let contract_info_list_spine = &contract_info_list;
    // let runtime = Builder::new_multi_thread()
    // .thread_stack_size(32 * 1024 * 1024)
    // .worker_threads(2000)
    // .enable_all()
    // .build()
    // .unwrap();
    //runtime.block_on(
    // runtime.block_on(async{
    for i in 0..num_client
    {
        let folder_name_clone = folder_name.clone();
        let client_clone = client.clone();
        let gas_obj_clone = gas_obj_list[i].clone();
        let coin_list_clone = coin_list[i].clone();
        let contract_info_list_clone = contract_info_list.clone();
        let origin_time_clone = origin_time.clone();
        // let task= runtime.spawn(async move {
        //     execution_single(num_contract, max_paralell, expected_interval, folder_name_clone, client_clone, sender, gas_obj_clone, 
        //     contract_info_list_clone, coin_list_clone, time1.clone(), time2.clone(), time3.clone(), origin_time_clone).await
        //     });
        let task= tokio::spawn(async move {
            execution_single(num_contract, max_paralell, expected_interval, folder_name_clone, client_clone, sender, gas_obj_clone, 
            contract_info_list_clone, coin_list_clone, time1.clone(), time2.clone(), time3.clone(), origin_time_clone).await
            });
        // let task =  execution_single(num_contract, max_paralell, expected_interval, folder_name_clone, 
        //     client_clone, sender, gas_obj_clone,contract_info_list_clone, coin_list_clone, time1, time2, time3, origin_time);
        tasks.push(task);
    }
    // for i in 0..num_client
    // {
    //     let folder_name_clone = folder_name.clone();
    //     let client_clone = client.clone();
    //     let gas_obj_clone = gas_obj_list[i].clone();
    //     let coin_list_clone = coin_list[i].clone();
    //     let contract_info_list_clone = contract_info_list.clone();
    //     let origin_time_clone = origin_time.clone();
    //     let task= runtime.spawn(async move {
    //         execution_single(num_contract, max_paralell, expected_interval, folder_name_clone, client_clone, sender, gas_obj_clone, 
    //         contract_info_list_clone, coin_list_clone, time1.clone(), time2.clone(), time3.clone(), origin_time_clone).await
    //         });
    //     // let task= tokio::spawn(async move {
    //     //     execution_single(num_contract, max_paralell, expected_interval, folder_name_clone, client_clone, sender, gas_obj_clone, 
    //     //     contract_info_list_clone, coin_list_clone, time1.clone(), time2.clone(), time3.clone(), origin_time_clone).await
    //     //     });
    //     // let task =  execution_single(num_contract, max_paralell, expected_interval, folder_name_clone, 
    //     //     client_clone, sender, gas_obj_clone,contract_info_list_clone, coin_list_clone, time1, time2, time3, origin_time);
    //     tasks.push(task);
    // }

    for task in tasks 
    {
        //let result_part = task.await?;
        let result_part = task.await??;
        for res in result_part
        {
            if res.if_sucess == 1
            {
                if res.start_time > time1 && res.start_time < time2
                {
                    success += 1;
                    total_latency += res.end_time - res.start_time;
                }            
            }
            else if res.start_time > time1 && res.start_time < time2
            {
                fail += 1;
            }
        }
    }

    // let contract_info = contract_info_list.get(0).unwrap();
    // let res = call_swap( client, sender, ContractInfo::new(contract_info.packageid, contract_info.global, contract_info.coin_package, contract_info.faucet_id), gas_obj_list[0][0].parse::<ObjectID>()?, coin_list[0][0][0], origin_time).await?;
    
    let mut latency = 30.0;
    if success != 0
    {
        latency = total_latency / success as f64;
    }

    Ok(ExperimentReturn::new(success,fail,latency))
}

async fn call_swap_new(execution: DataAndSender, origin_time: SystemTime)
    -> Result<ExecutionReturn, anyhow::Error>
{
    let start_time = SystemTime::now().duration_since(origin_time)?.as_secs_f64();


    let transaction_response_result: Result<sui_json_rpc_types::SuiTransactionBlockResponse, anyhow::Error> = execution.submit_tx().await;
    let transaction_response = transaction_response_result.unwrap_or_else(|err| {
        // eprintln!("Error: {}", err);
        SuiTransactionBlockResponse::default()
    });
    let mut if_success;
    if transaction_response.clone().effects.is_none()
    {
        if_success = 0;
    }
    else 
    {
        if_success = if transaction_response.clone().effects.unwrap().status().is_ok() 
        {
            1 as usize
        }
        else
        {
            0 as usize
        };
    }

    // println!("{:?}", transaction_response);

    let end_time = SystemTime::now().duration_since(origin_time)?.as_secs_f64();
    
    Ok(ExecutionReturn::new(if_success,start_time,end_time))
}

async fn execution_single_new(expected_interval: f64,
    execution_list:Vec<DataAndSender>, time3: f64, origin_time: SystemTime, folder_path: PathBuf, id: usize)
-> Result<Vec<ExecutionReturn>, anyhow::Error>
{
    let mut results = vec![];
    let lambda = 1.0 / expected_interval; 
    let mut tasks = vec![];
    // let mut rng = thread_rng();
    let mut rng = StdRng::from_entropy();
    let exp = Exp::new(lambda).unwrap();

    let mut st = SystemTime::now().duration_since(origin_time)?.as_secs_f64();
    for execution in execution_list
    {
        let task= tokio::spawn(async move {
            call_swap_new(execution, origin_time).await
            });
        tasks.push(task);
        let drawn_time = exp.sample(&mut rng);
        let next_time = drawn_time + st;
        if next_time > time3
        {
            break
        }
        let rest_time = next_time - SystemTime::now().duration_since(origin_time)?.as_secs_f64();
        if rest_time > 0.0
        {
            let duration = Duration::from_secs_f64(rest_time);
            sleep(duration).await;
        }
        st = SystemTime::now().duration_since(origin_time)?.as_secs_f64();
    }
    for tmp_task in tasks
    {
        results.push(tmp_task.await.unwrap_or_else(|err| 
            {
                Ok(ExecutionReturn::new(0, 0.0, 0.0))
            }).unwrap());
    }
    let mut file_path = folder_path.clone();
    file_path.push(format!("Client {}", id));
    let mut raw_file = File::create(&file_path)?;
    let res_clone = results.clone();
    for res in res_clone
    {
        writeln!(
            &mut raw_file,
            "{}, {}, {}",
            res.if_sucess, res.start_time, res.end_time
        ).unwrap();
    }
    Ok(results)
}


pub async fn execution_pool_new(num_client:usize, expected_interval: f64, client: SuiClient, sender: SuiAddress,
     execution_list_total: Vec<Vec<DataAndSender>>,
    time_warm_up: f64, time_cool_down: f64, time_test: f64, writen_path: PathBuf)
    -> Result<ExperimentReturn, anyhow::Error>
{
    let mut total_latency = 0.0;
    let mut success = 0;
    let mut fail = 0;

    let origin_time = SystemTime::now();
    let time1 = time_warm_up;
    let time2 = time1 + time_test;
    let time3 = time2 + time_cool_down;
    let mut tasks = Vec::new();
    let mut id = 0;
    for execution_list in execution_list_total
    {
        let writen_path_clone = writen_path.clone();
        let task= tokio::spawn(async move {
            execution_single_new(expected_interval, execution_list, time3, origin_time, writen_path_clone, id).await
            });
        tasks.push(task);
        id += 1;
    }

    for task in tasks 
    {
        //let result_part = task.await?;
        let result_part = task.await??;
        for res in result_part
        {
            if res.if_sucess == 1
            {
                if res.start_time > time1 && res.start_time < time2
                {
                    success += 1;
                    total_latency += res.end_time - res.start_time;
                }            
            }
            else if res.start_time > time1 && res.start_time < time2
            {
                fail += 1;
            }
        }
    }

    let mut latency = 30.0;
    if success != 0
    {
        latency = total_latency / success as f64;
    }

    Ok(ExperimentReturn::new(success,fail,latency))
}