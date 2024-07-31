use crate::TestTransactionSender;
use crate::faucet::get_and_and_split_gas_obj;
use crate::get_gas_obj_one_layer;
use crate::execution::call_swap_before_submit;
use crate::build_tx::DataAndSender;
use rand::seq::SliceRandom;
use anyhow::Ok;
use std::time:: Duration;
use tokio::time::sleep;
use tokio;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use sui_json::SuiJsonValue;
use sui_json_rpc_types::SuiTransactionBlockEffectsAPI;
use sui_json_rpc_types::{SuiTypeTag, ObjectChange};
use sui_sdk::types::base_types::{ObjectID, SuiAddress};
use sui_sdk::SuiClient;


pub const MAX_COIN_PER_PROCESS: usize = 1000;


pub const ONECOIN: usize = 100000000;
pub const POOLCOIN: usize = 1000000000000;
pub const COIN_EACH_OBJ: usize = 10000000;
pub const MAX_SPLIT_COUNT: usize = 1000;
pub const GAS_SPLIT: usize = 1000;




#[derive(Clone)]
pub struct ContractInfo{
    pub packageid: ObjectID,
    pub global: ObjectID,
    pub coin_package: ObjectID,
    pub faucet_id: ObjectID,
}
impl ContractInfo {
    pub fn new(packageid: ObjectID, global: ObjectID, coin_package: ObjectID, faucet_id: ObjectID,) -> Self {
        Self {
            packageid,
            global,
            coin_package,
            faucet_id,
        }
    }
}



pub async fn get_one_coin_obj(client: SuiClient, sender: SuiAddress, coin_package:ObjectID, 
    faucet_id: ObjectID, gas_object: ObjectID, coin_type: String, coin_amount: String)
    -> Result<ObjectID, anyhow::Error>
{
    let test_transaction_sender = TestTransactionSender::new(sender, gas_object, client.clone());
    let mut coin_obj = "0x".parse::<ObjectID>()?;
    let call_args = vec![
        faucet_id.to_string().parse::<SuiJsonValue>()?,     
        coin_amount.parse::<SuiJsonValue>()?,  
    ];
    let type_args = vec![
        SuiTypeTag::new(coin_type),        
    ];
    let transaction_response = test_transaction_sender.move_call(coin_package, "faucet", "force_claim", type_args, call_args).await?;
    //println!("{:?}", transaction_response.object_changes);
    if transaction_response.clone().effects.unwrap().status().is_ok() != true
    {
        panic!();
    }
    let obj_changes = transaction_response.clone().object_changes.unwrap();
    for item in &obj_changes {
        match item {
            ObjectChange::Created {
                sender,
                owner,
                object_type,
                object_id,
                version,
                digest,
            } => {
                coin_obj = *object_id;
            }
            _ => {
                
            }
        }
    }
    Ok(coin_obj)
}


pub async fn split_coins(client: SuiClient, sender: SuiAddress, coin_id:ObjectID, gas_object: ObjectID, split_count: u64)
    -> Result<Vec<ObjectID>, anyhow::Error>
{
    let mut coin_list: Vec<ObjectID> = vec![];
    coin_list.push(coin_id);
    if split_count > 1
    {
        let test_transaction_sender = TestTransactionSender::new(sender, gas_object, client.clone());
        let transaction_response = test_transaction_sender.split_coin_equal(coin_id, split_count ).await?;
        // println!("{:?}", transaction_response);
        if transaction_response.clone().effects.unwrap().status().is_ok() != true
        {
            panic!();
        }
        let obj_changes = transaction_response.clone().object_changes.unwrap();
        for item in &obj_changes {
            match item {
                ObjectChange::Created {
                    sender,
                    owner,
                    object_type,
                    object_id,
                    version,
                    digest,
                } => {
                    coin_list.push(*object_id);
                }
                _ => {
                    
                }
            }
        }
    }
    
    // println!("{:?}", coin_list);
    Ok(coin_list)
}

fn ceil_divide(num1: u64, num2: u64)->Result<u64, anyhow::Error>
{
    let result = if num1 % (num2 as u64) == 0 {
        num1 /  (num2 as u64)
    } else {
        num1 / num2 as u64 + 1
    };
    Ok(result)
}


pub async fn split_coins_paralell(client: SuiClient, sender: SuiAddress, coin_id:ObjectID, gas_object: ObjectID, split_count: u64)
    -> Result<Vec<ObjectID>, anyhow::Error>
{

    let total_process = ceil_divide(split_count, MAX_COIN_PER_PROCESS as u64)?;
    let mut large_coin_list = vec![];
    let mut medium_coin_count:u64= 0;
    let mut result: Vec<ObjectID> = vec![];
    if total_process <= MAX_COIN_PER_PROCESS as u64
    {
        large_coin_list.push(coin_id);
        medium_coin_count = total_process;
    }
    else 
    {
        let large_coin_num = ceil_divide(total_process, MAX_COIN_PER_PROCESS as u64)?;
        medium_coin_count = ceil_divide(total_process, large_coin_num)?;
        large_coin_list = split_coins(client.clone(), sender, coin_id, gas_object, total_process).await?;
    }
    let gas_obj_list = get_gas_obj_one_layer(medium_coin_count as usize, sender).await?;
    for large_coin in large_coin_list
    {
        let mut tasks = Vec::new();   
        let Task_queue = split_coins(client.clone(), sender, large_coin, gas_object, medium_coin_count).await?;
        for i in 0..total_process
        {
            let client_clone = client.clone();
            let this_gas_obj = gas_obj_list.clone()[i as usize].parse::<ObjectID>()?.clone();
            let this_coin_id = Task_queue[i as usize];
            // println!("{:?}", this_coin_id);
            let task = tokio::spawn(async move {
            split_coins(client_clone, sender, this_coin_id, this_gas_obj, MAX_COIN_PER_PROCESS as u64).await
            });
            tasks.push(task);
        }
        for task in tasks 
        {
            let result_part = task.await??;
            result.extend(result_part);
        }
    }
    
    Ok(result)
}


fn update_toml_samm(package_id: ObjectID, sender: SuiAddress)->Result<(), anyhow::Error>
{
    // We need to update the Move.toml file with the package_id and the sender address
    // Replace with the path to the Move.toml file
    let destination_file = "../samm-boost/Move.toml"; 
    let package_amm = package_id.to_string(); 
    let address = sender.to_string();
    let file = File::open(destination_file)?;
    let reader = BufReader::new(file);
    let mut new_lines = Vec::new();

    for line in reader.lines() {
        let line = line?;
        new_lines.push(line.clone());

        if line.contains(r#"version = "1.0.0""#) {
            new_lines.push(format!("published-at=\"{}\"", package_amm));
        }

        if line.contains("swap = ") {
            if let Some(last_line) = new_lines.last_mut() {
                *last_line = format!(r#"swap = "{}""#, package_amm);
            }
        }

        if line.contains("controller = ") {
            if let Some(last_line) = new_lines.last_mut() {
                *last_line = format!(r#"controller = "{}""#, address);
            }
        }

        if line.contains("beneficiary = ") {
            if let Some(last_line) = new_lines.last_mut() {
                *last_line = format!(r#"beneficiary = "{}""#, address);
            }
        }
    }

    let mut file = File::create(destination_file)?;
    for line in new_lines {
        writeln!(file, "{}", line)?;
    }
    Ok(())
}


pub async fn samm_builder(client: SuiClient, sender: SuiAddress, gas_object: ObjectID)-> Result<ContractInfo, anyhow::Error>
{
    // Initiate Move.toml file (will change due to test coins after each experiement)
    let source_file_path = "../samm-boost/Move_backup.toml";
    let target_file_path = "../samm-boost/Move.toml";
    fs::copy(source_file_path, target_file_path)?;
    // println!("test");
    // Results needed
    let mut packageid = "0x1".parse::<ObjectID>()?;
    let mut global = "0x1".parse::<ObjectID>()?;
    let mut coin_package = "0x1".parse::<ObjectID>()?;
    let mut faucet_id = "0x1".parse::<ObjectID>()?;

    let test_transaction_sender = TestTransactionSender::new(sender, gas_object, client.clone());
    let transaction_response = test_transaction_sender.publish_package("samm-boost").await?;
    // println!("{:?}", transaction_response.clone());
    // println!("{:?}", transaction_response);
    let obj_changes = transaction_response.clone().object_changes.unwrap();

    for item in &obj_changes {
        match item {
            ObjectChange::Published {
                package_id,
                version,
                digest,
                modules,
            } => {
                packageid = *package_id;
            }
            ObjectChange::Created {
                sender,
                owner,
                object_type,
                object_id,
                version,
                digest,
            } => {
                if object_type.name.to_string() == "Global"
                {
                    global = *object_id;
                }
            }
            _ => {
                
            }
        }
    }
    // Publish test coins
    update_toml_samm(packageid, sender);
    let test_transaction_sender = TestTransactionSender::new(sender, gas_object, client.clone());
    let transaction_response = test_transaction_sender.publish_package("samm-boost/test_coins").await?;
    // rintln!("{:?}", transaction_response.clone());
    // let x = transaction_response.clone().effects.unwrap().status().is_ok();
    if transaction_response.clone().effects.unwrap().status().is_ok() != true
    {
        panic!();
    }
    let obj_changes = transaction_response.clone().object_changes.unwrap();
    // println!("{:?}", obj_changes);
    for item in &obj_changes {
        match item {
            ObjectChange::Published {
                package_id,
                version,
                digest,
                modules,
            } => {
                coin_package = *package_id;
            }
            ObjectChange::Created {
                sender,
                owner,
                object_type,
                object_id,
                version,
                digest,
            } => {
                if object_type.name.to_string() == "Faucet"
                {
                    faucet_id = *object_id;
                }
            }
            _ => {
                
            }
        }
    }

    // println!("1111");
    let USDT_id = format!("{}::coins::USDT",coin_package).to_string();
    // println!("{}",USDT_id);
    let XBTC_id = format!("{}::coins::XBTC",coin_package).to_string();
    let mut USDT_object = "0x1".parse::<ObjectID>()?;
    let mut XBTC_object = "0x1".parse::<ObjectID>()?;
    // println!("{}",XBTC_id);
    // add admin
    let test_transaction_sender = TestTransactionSender::new(sender, gas_object, client.clone());
    let call_args = vec![
        faucet_id.to_string().parse::<SuiJsonValue>()?,        
        sender.to_string().parse::<SuiJsonValue>()?,
    ];
    let transaction_response = test_transaction_sender.move_call(coin_package, "faucet", "add_admin", vec![], call_args).await?;
    // println!("{:?}", transaction_response);
    if transaction_response.clone().effects.unwrap().status().is_ok() != true
    {
        panic!();
    }

    //Get coins and put it in the pool
    // command = f"sui client call  --gas-budget 100000000 --package {package_coin} --module faucet --function force_claim --args {faucetID} {math.ceil(POOLCOIN / ONECOIN)} --type-args {USDT_id}"
    
    let USDT_object = get_one_coin_obj(client.clone(), sender, coin_package, faucet_id, gas_object, USDT_id.clone(), (POOLCOIN/ONECOIN).to_string()).await?;
    
    let XBTC_object = get_one_coin_obj(client.clone(), sender, coin_package, faucet_id, gas_object, XBTC_id.clone(), (POOLCOIN/ONECOIN).to_string()).await?;


    // println!("{:?}", XBTC_object);
    // Add liquidity to the pool
    // command = f"sui client call --gas-budget 100000000 --package={package_AMM} --module=interface --function=add_liquidity --args {GlobalID} {USDT_obj} 1 {XBTC_obj} 1 --type-args {USDT_id} {XBTC_id}"
    let test_transaction_sender = TestTransactionSender::new(sender, gas_object, client.clone());
    let call_args = vec![
        global.to_string().parse::<SuiJsonValue>()?,     
        USDT_object.to_string().parse::<SuiJsonValue>()?,
        "1".parse::<SuiJsonValue>()?,
        XBTC_object.to_string().parse::<SuiJsonValue>()?,
        "1".parse::<SuiJsonValue>()?,
    ];
    let type_args = vec![
        SuiTypeTag::new(USDT_id.clone()),
        SuiTypeTag::new(XBTC_id.clone()),        
    ];
    let transaction_response = test_transaction_sender.move_call(packageid, "interface", "add_liquidity", type_args, call_args).await?;
    //println!("{:?}", transaction_response.clone());
    if transaction_response.clone().effects.unwrap().status().is_ok() != true
    {
        panic!();
    }

    // println!("package_id: {:?}", packageid);
    // println!("global: {:?}", global);
    // println!("coin_package: {:?}", coin_package);
    // println!("faucet_id: {:?}", faucet_id);
    Ok(ContractInfo::new(packageid, global, coin_package, faucet_id))
}






pub async fn samm_data_builder(client: SuiClient, sender: SuiAddress, num_contracts: usize, num_clients: usize,
    gas_object: ObjectID,  coin_each_client: usize)
    -> Result<Vec<Vec<DataAndSender>>, anyhow::Error>
{
    let mut execution_queque_raw = vec![];
    // generate the contracts and coins in each contract
    for i in 0..num_contracts
    {
        println!("Start to generate contract {}", i);
        let contractinfo = samm_builder(client.clone(), sender, gas_object).await?;
        let coin_package = contractinfo.coin_package;
        let faucet_id = contractinfo.faucet_id;
        let XBTC_id = format!("{}::coins::XBTC",coin_package).to_string();
        let mut coin_list: Vec<Vec<ObjectID>> = vec![];
        let mut total_coin = (num_clients * coin_each_client) as u64;
        total_coin = MAX_COIN_PER_PROCESS as u64 * ceil_divide(total_coin, MAX_COIN_PER_PROCESS as u64)?;
        // let total_coin_raw = num_clients * coin_each_client;
        // let total_coin = if total_coin_raw % MAX_COIN_PER_PROCESS == 0 {
        //     total_coin_raw as u64
        // } else {
        //     ((total_coin_raw / MAX_COIN_PER_PROCESS + 1) * MAX_COIN_PER_PROCESS) as u64
        // };
        // faucet from the testcoin
        let large_coin = get_one_coin_obj(client.clone(), sender, coin_package, faucet_id, gas_object, XBTC_id.clone(), ((total_coin as usize) * COIN_EACH_OBJ /ONECOIN).to_string()).await?;
        // split the coins into small coins
        let coin_list_raw = split_coins_paralell(client.clone(), sender, large_coin, gas_object, total_coin).await?;
        let num_large_gas = ceil_divide(total_coin, GAS_SPLIT as u64)?;
        let gas_list = get_and_and_split_gas_obj(client.clone(), num_large_gas as usize, sender, GAS_SPLIT).await?;
        let mut tasks = vec![];
        for (coin, gas_obj) in coin_list_raw.iter().zip(gas_list.iter())
        {
            let client_clone = client.clone();
            let contract_info = contractinfo.clone();
            let gas_obj_clone = (*gas_obj).clone();
            let coin_clone = (*coin).clone();
            // generate the signed transaction
            let task= tokio::spawn(async move {
                call_swap_before_submit(client_clone, sender, contract_info, gas_obj_clone, coin_clone).await
                });
            // let task =  execution_single(num_contract, max_paralell, expected_interval, folder_name_clone, 
            //     client_clone, sender, gas_obj_clone,contract_info_list_clone, coin_list_clone, time1, time2, time3, origin_time);
            tasks.push(task);
            let duration = Duration::from_secs_f64(0.0001);
            sleep(duration).await;
        }
        for task in tasks 
        {
            //let result_part = task.await?;
            let result_part = task.await??;
            execution_queque_raw.push(result_part);
        }
        println!("Contract {} finished.", i);
    }
    let mut rng = rand::thread_rng();
    execution_queque_raw.shuffle(&mut rng);
    let mut execution_queue = vec![];
    let total_coin_each_client = coin_each_client * num_contracts;
    for i in 0..num_clients 
    {
        let start = total_coin_each_client * i;
        let end = total_coin_each_client + start;
        let sub_vec = execution_queque_raw[start..end].to_vec();
        execution_queue.push(sub_vec);
    }
    Ok(execution_queue)
}