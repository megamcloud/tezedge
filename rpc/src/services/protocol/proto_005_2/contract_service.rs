// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::string::ToString;

use failure::bail;

use storage::num_from_slice;
use storage::skip_list::Bucket;
use storage::context::{TezedgeContext, ContextIndex, ContextApi};
use tezos_messages::protocol::{RpcJsonMap, ToRpcJsonMap,UniversalValue};
use tezos_messages::protocol::proto_005_2::contract::{MichelsonJsonElement, RpcJsonMapVector};
// use tezos_messages::protocol::proto_005_2::delegate::{BalanceByCycle, Delegate, DelegateList};
use tezos_messages::p2p::binary_message::BinaryMessage;
//use tezos_messages::protocol::proto_005_2::

use crate::helpers::ContextProtocolParam;
use crate::services::protocol::proto_005_2::helpers::{construct_indexed_contract_key, from_zarith};

pub(crate) fn get_contract(context_proto_params: ContextProtocolParam, _chain_id: &str, pkh: &str, context: TezedgeContext) -> Result<Option<RpcJsonMap>, failure::Error> {

    // level of the block
    let level = context_proto_params.level;

    let context_index = ContextIndex::new(Some(level), None);
    
    let indexed_contract_key = construct_indexed_contract_key(pkh)?;
    
    let balance_key = vec![indexed_contract_key.clone(), "balance".to_string()];
    let balance;
    // ["data","contracts","index","91","6e","d7","72","4e","49","0000535110affdb82923710d1ec205f26ba8820a2259","balance"]
    if let Some(Bucket::Exists(data)) = context.get_key(&context_index, &balance_key)? {
        balance = from_zarith(&data)?;
    } else {
        bail!("Balance not found");
    }

    // ["data","contracts","index","91","6e","d7","72","4e","49","0000535110affdb82923710d1ec205f26ba8820a2259","data","code"]
    let contract_script_code;
    let contract_script_code_key = vec![indexed_contract_key.clone(), "data/code".to_string()];
    if let Some(Bucket::Exists(data)) = context.get_key(&context_index, &contract_script_code_key)? {
        contract_script_code = Some(tezos_messages::protocol::proto_005_2::contract::Code::from_bytes(data)?);
    } else {
        // Set the value to default as implicit contracts have no script attached
        contract_script_code = None;
    }

    // println!("{:?}", contract_script_code.unwrap().code().simplify().as_map());
    // let tmp_test = contract_script_code.unwrap().code().simplify();
    // let mut json_vec: Vec<MichelsonJsonElement> = Default::default();
    // tmp_test.colapse(&mut json_vec);
    // println!("{:?}", json_vec.iter().map(|elem| elem.as_map()).collect::<RpcJsonMapVector>());

    // ["data","contracts","index","91","6e","d7","72","4e","49","0000535110affdb82923710d1ec205f26ba8820a2259","data","storage"]
    let contract_script_storage;
    if let Some(Bucket::Exists(data)) = context.get_key(&context_index, &vec!["data/votes/participation_ema".to_string()])? {
        contract_script_storage = Some(data);
    } else {
        // Set the value to default as implicit contracts have no script attached
        contract_script_storage = None;
    }

    // ["data","contracts","index","91","6e","d7","72","4e","49","0000535110affdb82923710d1ec205f26ba8820a2259","counter"]
    let contract_counter;
    if let Some(Bucket::Exists(data)) = context.get_key(&context_index, &vec!["data/votes/participation_ema".to_string()])? {
        contract_counter = Some(data);
    } else {
        // Set the value to default as implicit contracts have no script attached
        contract_counter = None;
    }



    Ok(Some(contract_script_code.unwrap().code().simplify().as_map()))
}