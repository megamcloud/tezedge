// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT


use std::collections::HashMap;
use std::convert::TryInto;
use std::string::ToString;

use failure::bail;
use itertools::Itertools;

use storage::num_from_slice;
use storage::persistent::ContextList;
use storage::skip_list::Bucket;
use storage::context_storage::contract_id_to_contract_address_for_index;
use tezos_messages::base::signature_public_key_hash::SignaturePublicKeyHash;
use tezos_messages::protocol::{RpcJsonMap, ToRpcJsonMap, UniversalValue, ToRpcJsonList};
use tezos_messages::protocol::proto_005_2::delegate::{BalanceByCycle, Delegate, DelegateList};
use tezos_messages::p2p::binary_message::BinaryMessage;
use num_bigint::{BigInt, ToBigInt};

use crate::helpers::ContextProtocolParam;
use crate::services::protocol::proto_005_2::helpers::{create_index_from_contract_id, from_zarith, cycle_from_level};

type DelegatedContracts = Vec<String>;
struct DelegateRawContextData {
    balance: BigInt,
    grace_period: i32,
    change: BigInt,
    deactivated: bool,
    frozen_balance_by_cycle: Vec<BalanceByCycle>,
    delegator_list: Vec<String>,
}

impl DelegateRawContextData {
    fn new(
        balance: BigInt,
        grace_period: i32,
        change: BigInt,
        deactivated: bool,
        frozen_balance_by_cycle: Vec<BalanceByCycle>,
        delegator_list: Vec<String>,
    ) -> Self {
        Self {
            balance,
            grace_period,
            change,
            deactivated,
            frozen_balance_by_cycle,
            delegator_list,
        }
    }
}

pub(crate) fn list_delegates(context_proto_params: ContextProtocolParam, _chain_id: &str, active: bool, context_list: ContextList) -> Result<Option<UniversalValue>, failure::Error> {
    let mut delegates: Vec<_> = Default::default();
    {
        let reader = context_list.read().unwrap();
        if let Ok(Some(ctx)) = reader.get(context_proto_params.level.clone()) {
            delegates = ctx.into_iter()
                .filter(|(k, _)| k.starts_with(&"data/delegates/"))
                .collect()
        }
    }

    let mut prefix_test: Vec<_> = Default::default();
    {
        let reader = context_list.read().unwrap();
        if let Some(Bucket::Exists(val)) = reader.get_key(context_proto_params.level.clone(), &"data/delegates/".to_string())? {
            prefix_test.push(val);
        }
    }
    println!("{:?}", prefix_test);

    // cf49f66b9ea137e11818f2a78b4b6fc9895b4e50
    // data/delegates/ed25519/cf/49/f6/6b/9e/a137e11818f2a78b4b6fc9895b4e50
    let mut delegate_list: DelegateList = Default::default();
    for (key, _) in delegates {
        let address = key.split("/").skip(3).take(6).join("");
        let curve = key.split("/").skip(2).take(1).join("");
        
        delegate_list.push(SignaturePublicKeyHash::from_hex_hash_and_curve(&address, &curve)?.to_string())
    }
    
    // let delegate_contract_key = construct_indexed_key(pkh)?;
    const KEY_POSTFIX_INACTIVE: &str = "inactive_delegate";
    let mut inactive_delegates: DelegateList = Default::default();
    let mut active_delegates: DelegateList = Default::default();

    let list = context_list.read().expect("mutex poisoning");
    for element in delegate_list {
        let contract_key = construct_indexed_key(&element)?;
        let activity_key = format!("{}/{}", contract_key, KEY_POSTFIX_INACTIVE);
        
        if let Some(Bucket::Exists(_)) = list.get_key(context_proto_params.level, &activity_key)? {
            inactive_delegates.push(element);
        } else {
            active_delegates.push(element);
        }

    }
    // println!("{:?}", delegate_list);

    if active {
        Ok(Some(active_delegates.as_list()))
    } else {
        Ok(Some(inactive_delegates.as_list()))
    }
}

// data/contracts/index/89/8b/61/90/64/9f/0000e394872fcb92d975589fb2c5fd4aab3c7adc80f7/<*>
// * -> manager
//      counter
//      balance

// data/contracts/index/a3/86/6f/a0/50/f6/00001e879a105f4e493c84322bb80051aa0585811e83/frozen_balance/<cycle_number>/<*>
// * -> fees
//      rewards
//      deposits

fn construct_indexed_key(pkh: &str) -> Result<String, failure::Error> {
    const KEY_PREFIX: &str = "data/contracts/index";
    let index = create_index_from_contract_id(pkh)?.join("/");
    let key = hex::encode(contract_id_to_contract_address_for_index(pkh)?);

    Ok(format!("{}/{}/{}", KEY_PREFIX, index, key))
}

fn get_delegate_context_data(context_proto_params: ContextProtocolParam, context: ContextMap, pkh: &str) -> Result<DelegateRawContextData, failure::Error> {
    // construct key for context db
    let block_level = context_proto_params.level;
    let dynamic = tezos_messages::protocol::proto_005_2::constants::ParametricConstants::from_bytes(context_proto_params.constants_data)?;
    let preserved_cycles = dynamic.preserved_cycles();
    let blocks_per_cycle = dynamic.blocks_per_cycle();

    // key pre and postfixes for context database
    
    const KEY_POSTFIX_BALANCE: &str = "balance";
    const KEY_POSTFIX_FROZEN_BALANCE: &str = "frozen_balance";
    const KEY_POSTFIX_DEPOSITS: &str = "deposits";
    const KEY_POSTFIX_FEES: &str = "fees";
    const KEY_POSTFIX_REWARDS: &str = "rewards";
    const KEY_POSTFIX_DELEGATED: &str = "delegated";
    const KEY_POSTFIX_INACTIVE: &str = "inactive_delegate";
    const KEY_POSTFIX_GRACE_PERIOD: &str = "delegate_desactivation";
    const KEY_POSTFIX_CHANGE: &str= "change";
    
    let block_cycle = cycle_from_level(block_level.try_into()?, blocks_per_cycle)?;
    
    let delegate_contract_key = construct_indexed_key(pkh)?;

    let balance_key = format!("{}/{}", delegate_contract_key, KEY_POSTFIX_BALANCE);
    let activity_key = format!("{}/{}", delegate_contract_key, KEY_POSTFIX_INACTIVE);
    let grace_period_key = format!("{}/{}", delegate_contract_key, KEY_POSTFIX_GRACE_PERIOD);
    let change_key = format!("{}/{}", delegate_contract_key, KEY_POSTFIX_CHANGE);

    let balance: BigInt;
    let mut frozen_balance_by_cycle: Vec<BalanceByCycle> = Vec::new();
    let grace_period: i32;
    let change: BigInt;
    let deactivated: bool;

    {
        if let Some(Bucket::Exists(data)) = context.get(&balance_key) {
            // println!("Getting balance with key: {}", &balance_key);
            balance = from_zarith(data)?;
        } else {
            bail!("Balance not found");
        }
        if let Some(Bucket::Exists(data)) =  context.get(&grace_period_key) {
            grace_period = num_from_slice!(data, 0, i32);
        } else {
            bail!("grace_period not found");
        }
        if let Some(Bucket::Exists(data)) =  context.get(&change_key) {
            change = from_zarith(data)?;
            // println!("Getting change with key: {}", &change_key);
        } else {
            bail!("change not found");
        }
        if let Some(Bucket::Exists(_)) =  context.get(&activity_key) {
            deactivated = true
        } else {
            deactivated = false;
        }
    };
    // println!("Balance for {}: {:?}", pkh, balance);

    // frozen balance

    for cycle in block_cycle - preserved_cycles as i64..block_cycle + 1 {
        if cycle >= 0 {
            let frozen_balance_key = format!("{}/{}/{}", delegate_contract_key, KEY_POSTFIX_FROZEN_BALANCE, cycle);

            let frozen_balance_deposits_key = format!("{}/{}", frozen_balance_key, KEY_POSTFIX_DEPOSITS);
            let frozen_balance_fees_key = format!("{}/{}", frozen_balance_key, KEY_POSTFIX_FEES);
            let frozen_balance_rewards_key = format!("{}/{}", frozen_balance_key, KEY_POSTFIX_REWARDS);

            
            let frozen_balance_fees: BigInt;
            let frozen_balance_deposits: BigInt;
            let frozen_balance_rewards: BigInt;

            let mut found_flag: bool = false;
            // get the frozen balance dat for preserved cycles and the current one
            if let Some(Bucket::Exists(data)) =  context.get(&frozen_balance_deposits_key) {
                // println!("Getting frozen balance deposits with key: {}", &frozen_balance_deposits_key);
                frozen_balance_deposits = from_zarith(data)?;
                found_flag = true;
            } else {
                // println!("frozen_balance_deposits not found. Setting default value");
                frozen_balance_deposits = Default::default();
            }
            if let Some(Bucket::Exists(data)) =  context.get(&frozen_balance_fees_key) {
                // println!("Getting frozen balance fees with key: {}", &frozen_balance_fees_key);
                frozen_balance_fees = from_zarith(data)?;
                found_flag = true;
            } else {
                // println!("Frozen balance fees not found. Setting default value");
                frozen_balance_fees = Default::default();
            }
            if let Some(Bucket::Exists(data)) =  context.get(&frozen_balance_rewards_key) {
                // println!("Getting frozen balance rewards with key: {}", &frozen_balance_rewards_key);
                frozen_balance_rewards = from_zarith(data)?;
                found_flag = true;
            } else {
                // println!("frozen_balance_rewards not found. Setting default value");
                frozen_balance_rewards = Default::default();
            }
            // ocaml behavior
            // corner case - carthagenet - blocks <1, 6> including an empty array
            // in block 7, deposits and rewards are set, so push to vector with the fetched values and set the rest to default
            // we should push to this vec only when at least one value is found (is set in context) otherwise do not push
            if found_flag {
                frozen_balance_by_cycle.push(BalanceByCycle::new(cycle.try_into()?, frozen_balance_deposits.try_into()?, frozen_balance_fees.try_into()?, frozen_balance_rewards.try_into()?));
            }
        }
    }
    // Full key to the delegated balances looks like the following
    // "data/contracts/index/ad/af/43/23/f9/3e/000003cb7d7842406496fc07288635562bfd17e176c4/delegated/72/71/28/a2/ba/a4/000049c9bce2a9d04f7b38d32398880d96e8756a1d5c"
    // we get all delegated contracts to the delegate by filtering the context with prefix:
    // "data/contracts/index/ad/af/43/23/f9/3e/000003cb7d7842406496fc07288635562bfd17e176c4/delegated"
    let delegated_contracts_key_prefix = format!("{}/{}", delegate_contract_key, KEY_POSTFIX_DELEGATED);
    let delegated_contracts: DelegatedContracts = context.clone()
            .into_iter()
            .filter(|(k, _)| k.starts_with(&delegated_contracts_key_prefix))
            .map(|(k, _)| SignaturePublicKeyHash::from_tagged_hex_string(&k.split("/").last().unwrap().to_string()).unwrap().to_string())
            .collect();

    Ok(DelegateRawContextData::new(balance, grace_period, change, deactivated, frozen_balance_by_cycle, delegated_contracts))
}

type ContextMap = HashMap<String, Bucket<Vec<u8>>>;
fn get_context(block_level: usize, context_list: ContextList) -> Result<ContextMap, failure::Error> {
    let context: ContextMap;
    {
        let reader = context_list.read().unwrap();
        if let Ok(Some(ctx)) = reader.get(block_level) {
            context = ctx
        } else {
            bail!("Context not found")
        }
    }
    Ok(context)
}

fn get_roll_count(pkh: &str, context: ContextMap) -> Result<i32, failure::Error> {
    // Note somethig similar is in the rigths

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
    Ok(roll_count)
}

pub(crate) fn get_delegate(context_proto_params: ContextProtocolParam, _chain_id: &str, pkh: &str, context_list: ContextList) -> Result<Option<RpcJsonMap>, failure::Error> {
    // get block level first
    let block_level = context_proto_params.level;
    let dynamic = tezos_messages::protocol::proto_005_2::constants::ParametricConstants::from_bytes(context_proto_params.constants_data.clone())?;
    let tokens_per_roll: BigInt = dynamic.tokens_per_roll().try_into()?;
    
    // we need to get the whole context down the line, so we can just fetch the whole from the sotrage here
    let context = get_context(block_level, context_list)?;

    // fetch delegate data from the context
    let delegate_data = get_delegate_context_data(context_proto_params, context.clone(), pkh)?;

    // staking_balance
    let roll_count = get_roll_count(pkh, context)?;
    
    let staking_balance: BigInt;
    staking_balance = tokens_per_roll * roll_count + delegate_data.change;

    // delegated balance

    // calculate the sums of deposits, fees an rewards accross all preserved cycles, including the current one
    // unwraps are safe, we are creating a BigInt from zero, so it allways fits
    let frozen_deposits: BigInt = delegate_data.frozen_balance_by_cycle.iter()
        .map(|val| val.deposit().into())
        .fold(ToBigInt::to_bigint(&0).unwrap(), |acc, elem: BigInt| acc + elem);

    let frozen_fees: BigInt = delegate_data.frozen_balance_by_cycle.iter()
        .map(|val| val.fees().into())
        .fold(ToBigInt::to_bigint(&0).unwrap(), |acc, elem: BigInt| acc + elem);

    let frozen_rewards: BigInt = delegate_data.frozen_balance_by_cycle.iter()
        .map(|val| val.rewards().into())
        .fold(ToBigInt::to_bigint(&0).unwrap(), |acc, elem: BigInt| acc + elem);

    // delegated balance is calculetd by subtracting the sum of balance frozen_deposits and frozen_fees
    // balance in this context means the spendable balance
    let delegated_balance: BigInt = &staking_balance - (&delegate_data.balance + &frozen_deposits + &frozen_fees);

    // frozen balance is the sum of all the frozen items
    let frozen_balance: BigInt = frozen_deposits + frozen_fees + frozen_rewards;

    // full balance includes frozen balance as well
    let full_balance: BigInt = &frozen_balance + delegate_data.balance;
    
    let delegates = Delegate::new(
        full_balance.try_into()?,
        frozen_balance.try_into()?,
        delegate_data.frozen_balance_by_cycle,
        staking_balance.try_into()?,
        delegate_data.delegator_list,
        delegated_balance.try_into()?,
        delegate_data.deactivated,
        delegate_data.grace_period
    );
    
    Ok(Some(delegates.as_map()))
}