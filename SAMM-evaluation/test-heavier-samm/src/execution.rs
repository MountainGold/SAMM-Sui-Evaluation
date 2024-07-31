use crate::TestTransactionSender;
use crate::build_tx::DataAndSender;
use crate::ContractInfo;
use anyhow::Ok;
use tokio;
use sui_json_rpc_types::SuiTransactionBlockResponse;
use std::fs::File;
use std::io::Write;
use sui_json::SuiJsonValue;
use tokio::time::sleep;
use sui_json_rpc_types::SuiTransactionBlockEffectsAPI;
use sui_json_rpc_types::SuiTypeTag;
use sui_sdk::types::base_types::{ObjectID, SuiAddress};
use sui_sdk::SuiClient;
use std::time::{SystemTime, Duration};

use std::path::PathBuf;
use rand::prelude::*;
use rand_distr::Exp;
use rand::rngs::StdRng;

// The result of each execution
#[derive(Clone)]
pub struct ExecutionReturn
{
    // whether the execution is successful
    pub if_sucess: usize,
    // the start time of the execution
    pub start_time: f64,
    // the end time of the execution
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

// The result of the whole experiment
pub struct ExperimentReturn
{
    // the number of successful transactions
    pub success: usize,
    // the number of failed transactions
    pub fail: usize,
    // the average latency of successful transactions
    pub average_latency: f64,
}
impl ExperimentReturn
{
    pub fn new(success: usize, fail: usize, average_latency: f64) -> Self 
    {
        Self {
            success,
            fail,
            average_latency,
        }
    }
}

// Generate the signed transaction (not submitted)
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

// The task of a single client
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
        // use a thread to call the function
        let task= tokio::spawn(async move {
            call_swap_new(execution, origin_time).await
            });
        tasks.push(task);
        // sleep for a random time drawn from an exponential distribution
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

// We don't use num_client since the length of execution_list_total is exactly the number of clients
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
    // Each client is spawned in a separate task
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