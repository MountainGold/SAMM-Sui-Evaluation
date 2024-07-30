pub async fn samm_builder(client: SuiClient, sender: SuiAddress, gas_object: ObjectID)-> Result<ContractInfo, anyhow::Error>
{
    // Initiate move package
    let source_file_path = "../samm/Move_backup.toml";
    let target_file_path = "../samm/Move.toml";
    fs::copy(source_file_path, target_file_path)?;
    // println!("test");
    // Results needed
    let mut packageid = "0x1".parse::<ObjectID>()?;
    let mut global = "0x1".parse::<ObjectID>()?;
    let mut coin_package = "0x1".parse::<ObjectID>()?;
    let mut faucet_id = "0x1".parse::<ObjectID>()?;

    let test_transaction_sender = TestTransactionSender::new(sender, gas_object, client.clone());
    let transaction_response = test_transaction_sender.publish_package("samm").await?;
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
    update_toml(packageid, sender);
    let test_transaction_sender = TestTransactionSender::new(sender, gas_object, client.clone());
    let transaction_response = test_transaction_sender.publish_package("samm/test_coins").await?;
    // println!("{:?}", transaction_response.clone());
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


fn update_toml(package_id: ObjectID, sender: SuiAddress)->Result<(), anyhow::Error>
{
    let destination_file = "../samm/Move.toml"; // 替换为目标文件的路径
    let package_amm = package_id.to_string(); // 替换为 package_AMM 的值
    let address = sender.to_string(); // 替换为 address 的值

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


pub async fn samm_testcoin_builder(client: SuiClient, sender: SuiAddress, 
    coin_package:ObjectID, faucet_id: ObjectID, gas_object: ObjectID, num_clients: usize, coin_each_client: usize)
    -> Result<Vec<Vec<ObjectID>>, anyhow::Error>
{
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
    
    let large_coin = get_one_coin_obj(client.clone(), sender, coin_package, faucet_id, gas_object, XBTC_id.clone(), ((total_coin as usize) * COIN_EACH_OBJ /ONECOIN).to_string()).await?;

    let coin_list_raw = split_coins_paralell(client, sender, large_coin, gas_object, total_coin).await?;

    // let coin_list_raw = split_coins_test(client, sender, large_coin, gas_object, total_coin).await?;
    // Results needed

    for i in 0..num_clients {
        let start = coin_each_client * i;
        let end = coin_each_client + start;
        let sub_vec = coin_list_raw[start..end].to_vec();
        coin_list.push(sub_vec);
    }
    Ok(coin_list)
}