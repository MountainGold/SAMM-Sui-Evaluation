use reqwest::Client;
use serde_json::{json, Value};
use sui_sdk::types::base_types::SuiAddress;
use crate::build_contract::split_coins;
use sui_sdk::types::base_types::ObjectID;
use tokio;
use std::time::Duration;
use tokio::time::sleep;

pub const SUI_FAUCET: &str = "http://127.0.0.1:9123/gas";
// pub const SUI_FAUCET: &str = "http://132.68.60.223:9123/gas";
pub const MAX_PROCESS: usize = 100;

// Extract id from a json file
fn extract_ids(json_body: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(transferred_gas_objects) = json_body["transferredGasObjects"].as_array() {
        for obj in transferred_gas_objects {
            if let Some(id) = obj["id"].as_str() {
                ids.push(id.to_string());
            }
        }
    }
    ids
}


/// Request tokens from the Faucet for the given address
#[allow(unused_assignments)]
pub async fn request_five_gas_obj(
    address: SuiAddress
) -> Result<Vec<String>, anyhow::Error>{
    let address_str = address.to_string();
    let json_body = json![{
        "FixedAmountRequest": {
            "recipient": &address_str
        }
    }];

    // make the request to the faucet JSON RPC API for coin
    let client = Client::new();
    let resp = client
        .post(SUI_FAUCET)
        .header("Content-Type", "application/json")
        .json(&json_body)
        .send()
        .await?;
    // println!(
    //     "Faucet request for address {address_str} has status: {}",
    //     resp.status()
    // );
    // println!("Waiting for the faucet to complete the gas request...");

    // resp contains addresses
    let json_body: serde_json::Value = resp.json().await?;
    
    let ids = extract_ids(&json_body);
    // for id in ids {
    //     println!("ID: {}", id);
    // }
    Ok(ids)
}


// Return a list of gas objects
pub async fn get_gas_obj(num_client: usize, obj_per_client: usize, address: SuiAddress) -> Result<Vec<Vec<String>>, anyhow::Error> {
    let mut obj_list = Vec::new();
    let mut obj_list_raw = Vec::new();
    let total_client: usize = num_client * obj_per_client;
    while obj_list_raw.len() < total_client
    {
        let ids = request_five_gas_obj(address).await?;
        for id in ids 
        {
            obj_list_raw.push(id.to_string());
        }
    }

    while obj_list.len() < num_client {
        let mut client_objs = Vec::new();
        for _ in 0..obj_per_client {
            if let Some(obj) = obj_list_raw.pop() {
                client_objs.push(obj);
            }
        }
        obj_list.push(client_objs);
    }
    
    Ok(obj_list)
}

pub async fn get_gas_obj_one_layer(num_obj: usize, address: SuiAddress) -> Result<Vec<String>, anyhow::Error> {
    let mut obj_list_raw = Vec::new();
    while obj_list_raw.len() < num_obj
    {
        let ids = request_five_gas_obj(address).await?;
        for id in ids 
        {
            obj_list_raw.push(id.to_string());
        }
    }
    Ok(obj_list_raw)
}

pub async fn get_gas_obj_one_layer_cuncurrent(num_obj: usize, address: SuiAddress) -> Result<Vec<String>, anyhow::Error> {
    let mut obj_list_raw = Vec::new();
    let mut tasks = vec![];
    let mut total_lenth = 0;
    while total_lenth < num_obj
    {
        let task = request_five_gas_obj(address);
        tasks.push(task);
        total_lenth += 5;
    }
    for task in tasks
    {
        let res = task.await?;
        obj_list_raw.extend(res);
    }
    let duration = Duration::from_secs_f64(0.001);
    sleep(duration).await;
    Ok(obj_list_raw)
}

pub async fn get_and_and_split_gas_obj(client: sui_sdk::SuiClient, num_obj: usize, address: SuiAddress, each_split: usize) -> Result<Vec<ObjectID>, anyhow::Error> {
    let mut obj_list_raw = get_gas_obj_one_layer_cuncurrent(num_obj, address).await?;
    let mut currenct_process = num_obj;
    if currenct_process > MAX_PROCESS
    {
        currenct_process = MAX_PROCESS;
    }
    let gas_list = get_gas_obj_one_layer_cuncurrent(currenct_process, address).await?;
    let mut rest = vec![];
    let mut rest_coin_num = num_obj;
    while rest_coin_num != 0
    {
        let mut this_cuncurrency = currenct_process;
        if rest_coin_num < currenct_process
        {
            this_cuncurrency = rest_coin_num;
            rest_coin_num = 0;
        }
        else 
        {
            rest_coin_num -= currenct_process;
        }
        let mut tasks = vec![];
        for i in 0..this_cuncurrency
        {
            let client_clone = client.clone();
            let this_coin = obj_list_raw.remove(0).parse::<ObjectID>()?;
            let this_gas_obj = gas_list[i].parse::<ObjectID>()?;
            let task = tokio::spawn(async move {
                split_coins(client_clone, address, this_coin,
                    this_gas_obj, each_split as u64).await
                });
            tasks.push(task);
        }
        for task in tasks
        {
            let result_part = task.await??;
            rest.extend(result_part);
        }
    }
    Ok(rest)
}
