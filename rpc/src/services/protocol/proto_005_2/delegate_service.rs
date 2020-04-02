// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT


use std::collections::HashMap;
use std::convert::TryInto;
use std::string::ToString;

use failure::bail;

use storage::num_from_slice;
use storage::persistent::ContextList;
use storage::skip_list::Bucket;
use tezos_messages::base::signature_public_key_hash::SignaturePublicKeyHash;
use tezos_messages::protocol::{RpcJsonMap, ToRpcJsonMap};
use tezos_messages::protocol::proto_005_2::delegate::{BalanceByCycle, Delegate};
use tezos_messages::p2p::binary_message::BinaryMessage;
use num_bigint::{BigInt, ToBigInt};

use crate::encoding::conversions::contract_id_to_address;
use crate::helpers::ContextProtocolParam;
use crate::services::protocol::proto_005_2::helpers::{create_index_from_contract_id, from_zarith, cycle_from_level};


// data/contracts/index/89/8b/61/90/64/9f/0000e394872fcb92d975589fb2c5fd4aab3c7adc80f7/<*>
// * -> manager
//      counter
//      balance

// data/contracts/index/a3/86/6f/a0/50/f6/00001e879a105f4e493c84322bb80051aa0585811e83/frozen_balance/<cycle_number>/<*>
// * -> fees
//      rewards
//      deposits

pub(crate) fn get_delegate(context_proto_params: ContextProtocolParam,_chain_id: &str, pkh: &str, context_list: ContextList) -> Result<Option<RpcJsonMap>, failure::Error> {
    // get block level first
    let block_level = context_proto_params.level;
    let dynamic = tezos_messages::protocol::proto_005_2::constants::ParametricConstants::from_bytes(context_proto_params.constants_data)?;
    let preserved_cycles = dynamic.preserved_cycles();
    let blocks_per_cycle = dynamic.blocks_per_cycle();
    let tokens_per_roll: BigInt = dynamic.tokens_per_roll().try_into()?;

    let block_cycle = cycle_from_level(block_level.try_into()?, blocks_per_cycle)?;

    // balance in Delegate struct is the full_balance of the delegate (including own funds and delegated funds? (Note: i hope so, investigate!))
    // in context: "data/contracts/index/89/8b/61/90/64/9f/0000e394872fcb92d975589fb2c5fd4aab3c7adc80f7/balance":{"Exists":[167,227,50]}
    //                  ^^ this should be the owned balance
    // 
    // add this to the frozen_balance
    // frozen_balance = frozen_deposit + frozen_fees + frozen_rewards
    // frozen_deposit = 
    
    // construct key for context db
    let key_prefix = "data/contracts/index";
    let index = create_index_from_contract_id(pkh)?.join("/");
    let key = hex::encode(contract_id_to_address(pkh)?);

    let delegate_contract_key = format!("{}/{}/{}", key_prefix, index, key);

    let key_postfix_balance = "balance";
    let key_postfix_frozen_balance = "frozen_balance";
    let key_postfix_deposits = "deposits";
    let key_postfix_fees = "fees";
    let key_postfix_rewards = "rewards";

    let key_postfix_delegated = "delegated";

    let key_postfix_inactive = "inactive_delegate";
    let key_postfix_grace_period = "delegate_desactivation";
    let key_postfix_change = "change";


    let balance_key = format!("{}/{}", delegate_contract_key, key_postfix_balance);
    let activity_key = format!("{}/{}", delegate_contract_key, key_postfix_inactive);
    let grace_period_key = format!("{}/{}", delegate_contract_key, key_postfix_grace_period);
    let change_key = format!("{}/{}", delegate_contract_key, key_postfix_change);    

    let balance: BigInt;
    let mut frozen_balance_by_cycle: Vec<BalanceByCycle> = Vec::new();
    let grace_period: i32;
    let change: BigInt;
    let deactivated: bool;

    {
        let reader = context_list.read().unwrap();
        if let Some(Bucket::Exists(data)) = reader.get_key(block_level, &balance_key)? {
            println!("Getting balance with key: {}", &balance_key);
            balance = from_zarith(data)?;
        } else {
            bail!("Balance not found");
        }
        if let Some(Bucket::Exists(data)) = reader.get_key(block_level, &grace_period_key)? {
            grace_period = num_from_slice!(data, 0, i32);
        } else {
            bail!("grace_period not found");
        }
        if let Some(Bucket::Exists(data)) = reader.get_key(block_level, &change_key)? {
            change = from_zarith(data)?;
            println!("Getting change with key: {}", &change_key);
        } else {
            bail!("change not found");
        }
        if let Some(Bucket::Exists(_)) = reader.get_key(block_level, &activity_key)? {
            deactivated = true
        } else {
            deactivated = false;
        }
    };
    println!("Balance for {}: {:?}", pkh, balance);

    // frozen balance

    for cycle in block_cycle - preserved_cycles as i64..block_cycle + 1 {
        if cycle >= 0 {
            let reader = context_list.read().unwrap();

            let frozen_balance_key = format!("{}/{}/{}", delegate_contract_key, key_postfix_frozen_balance, cycle);

            let frozen_balance_deposits_key = format!("{}/{}", frozen_balance_key, key_postfix_deposits);
            let frozen_balance_fees_key = format!("{}/{}", frozen_balance_key, key_postfix_fees);
            let frozen_balance_rewards_key = format!("{}/{}", frozen_balance_key, key_postfix_rewards);

            
            let frozen_balance_fees: BigInt;
            let frozen_balance_deposits: BigInt;
            let frozen_balance_rewards: BigInt;
            // get the frozen balance dat for preserved cycles and the current one
            if let Some(Bucket::Exists(data)) = reader.get_key(block_level, &frozen_balance_deposits_key)? {
                println!("Getting frozen balance deposits with key: {}", &frozen_balance_deposits_key);
                frozen_balance_deposits = from_zarith(data)?;
            } else {
                // bail!("frozen_balance_deposits not found");
                continue;
            }
            if let Some(Bucket::Exists(data)) = reader.get_key(block_level, &frozen_balance_fees_key)? {
                println!("Getting frozen balance fees with key: {}", &frozen_balance_fees_key);
                frozen_balance_fees = from_zarith(data)?;
            } else {
                // bail!("Frozen balance fees not found");
                continue;

            }
            if let Some(Bucket::Exists(data)) = reader.get_key(block_level, &frozen_balance_rewards_key)? {
                println!("Getting frozen balance rewards with key: {}", &frozen_balance_rewards_key);
                frozen_balance_rewards = from_zarith(data)?;
            } else {
               //  bail!("frozen_balance_rewards not found");
               continue;
            }
            frozen_balance_by_cycle.push(BalanceByCycle::new(cycle.try_into()?, frozen_balance_deposits.try_into()?, frozen_balance_fees.try_into()?, frozen_balance_rewards.try_into()?));
        }
    }

    // staking_balance

    // Note somethig similar is in the rigths
    // make a type alias
    let context: HashMap<String, Bucket<Vec<u8>>>;
    {
        let reader = context_list.read().unwrap();
        if let Ok(Some(ctx)) = reader.get(block_level) {
            context = ctx
        } else {
            bail!("Context not found")
        }
    }

    type ContextMap = HashMap<String, Bucket<Vec<u8>>>;
    // simple counter to count the number of rolls the delegate owns
    let mut roll_count: i32 = 0;
    let data: ContextMap = context.clone()
        .into_iter()
        .filter(|(k, _)| k.contains(&"data/rolls/owner/current")) 
        .collect();

    // iterate through all the owners,the roll_num is the last component of the key, decode the value (it is a public key) to get the public key hash address (tz1...)
    for (_, value) in data.into_iter() {
        // the values are public keys
        if let Bucket::Exists(pk) = value {
            let delegate = SignaturePublicKeyHash::from_tagged_bytes(pk)?.to_string();
            if delegate.eq(pkh) {
                roll_count += 1;
            }
            // roll_owners.entry(delegate)
            // .and_modify(|val| val.push(roll_num.parse().unwrap()))
            // .or_insert(vec![roll_num.parse().unwrap()]);
        } else {
            continue;  // If the value is Deleted then is skipped and it go to the next iteration
        }
    }
    
    let staking_balance: BigInt;
    staking_balance = tokens_per_roll * roll_count + change;

    type DelegatedContracts = Vec<String>;

    // Full key to the delegated balances looks like the following
    // "data/contracts/index/ad/af/43/23/f9/3e/000003cb7d7842406496fc07288635562bfd17e176c4/delegated/72/71/28/a2/ba/a4/000049c9bce2a9d04f7b38d32398880d96e8756a1d5c"
    // we get all delegated contracts to the delegate by filtering the context with prefix:
    // "data/contracts/index/ad/af/43/23/f9/3e/000003cb7d7842406496fc07288635562bfd17e176c4/delegated"
    let delegated_contracts_key_prefix = format!("{}/{}", delegate_contract_key, key_postfix_delegated);
    let delegated_contracts: DelegatedContracts = context.clone()
            .into_iter()
            .filter(|(k, _)| k.starts_with(&delegated_contracts_key_prefix))
            .map(|(k, _)| SignaturePublicKeyHash::from_tagged_hex_string(&k.split("/").last().unwrap().to_string()).unwrap().to_string())
            .collect();

    // delegated balance

    // calculate the sums of deposits, fees an rewards accross all preserved cycles, including the current one
    let frozen_deposits: BigInt = frozen_balance_by_cycle.iter()
        .map(|val| val.deposit().try_into().unwrap())
        .fold(ToBigInt::to_bigint(&0).unwrap(), |acc, elem: BigInt| acc + elem);

    let frozen_fees: BigInt = frozen_balance_by_cycle.iter()
        .map(|val| val.fees().try_into().unwrap())
        .fold(ToBigInt::to_bigint(&0).unwrap(), |acc, elem: BigInt| acc + elem);

    let frozen_rewards: BigInt = frozen_balance_by_cycle.iter()
        .map(|val| val.rewards().try_into().unwrap())
        .fold(ToBigInt::to_bigint(&0).unwrap(), |acc, elem: BigInt| acc + elem);

    let delegated_balance: BigInt = &staking_balance - (&balance + &frozen_deposits + &frozen_fees);
    let frozen_balance: BigInt = frozen_deposits + frozen_fees + frozen_rewards;
    
    let delegates = Delegate::new(balance.try_into()?, frozen_balance.try_into()?, frozen_balance_by_cycle, staking_balance.try_into()?, delegated_contracts, delegated_balance.try_into()?, deactivated, grace_period);
    
    Ok(Some(delegates.as_map()))
    //Ok(None)
}