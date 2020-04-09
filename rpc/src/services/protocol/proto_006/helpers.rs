// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::bail;
use getset::{Getters, Setters, CopyGetters};

use crypto::blake2b;
use storage::context_action_storage::contract_id_to_contract_address_for_index;
use tezos_messages::protocol::proto_006::delegate::{BalanceByCycle};
use tezos_encoding::binary_reader::BinaryReader;
use tezos_encoding::de;
use tezos_encoding::encoding::Encoding;
use num_bigint::{BigInt, Sign};


/// Return cycle in which is given level
///
/// # Arguments
///
/// * `level` - level to specify cycle for
/// * `blocks_per_cycle` - context constant
///
/// Level 0 (genesis block) is not part of any cycle (cycle 0 starts at level 1),
/// hence the blocks_per_cycle - 1 for last cycle block.
pub fn cycle_from_level(level: i64, blocks_per_cycle: i32) -> Result<i64, failure::Error> {
    // check if blocks_per_cycle is not 0 to prevent panic
    if blocks_per_cycle > 0 {
        Ok((level - 1) / (blocks_per_cycle as i64))
    } else {
        bail!("wrong value blocks_per_cycle={}", blocks_per_cycle)
    }
}

/// convert zarith encoded bytes to BigInt
pub(crate) fn from_zarith(zarith_num: &Vec<u8>) -> Result<BigInt, failure::Error> {
    // decode the bytes using the BinaryReader
    let intermediate = BinaryReader::new().read(&zarith_num, &Encoding::Mutez).unwrap();
    
    // deserialize from intermediate form
    let mut deserialized = de::from_value::<String>(&intermediate).unwrap();

    // fill in the bytes in case of odd string len
    if deserialized.len() % 2 != 0 {
        deserialized.insert(0, '0');
    }

    Ok(BigInt::from_bytes_be(Sign::Plus, &hex::decode(deserialized)?))
}

#[inline]
pub fn create_index_from_contract_id(contract_id: &str) -> Result<Vec<String>, failure::Error> {
    const INDEX_SIZE: usize = 6;
    let mut index = Vec::new();

    // input validation is handled by the contract_id_to_address function 
    let address = contract_id_to_contract_address_for_index(contract_id)?;

    let hashed = hex::encode(blake2b::digest_256(&address));

    for elem in (0..INDEX_SIZE * 2).step_by(2) {
        index.push(hashed[elem..elem + 2].to_string());
    }

    Ok(index)
}

pub type DelegatedContracts = Vec<String>;

#[derive(Getters)]
pub struct DelegateRawContextData {
    #[get = "pub(crate)"]
    balance: BigInt,

    #[get = "pub(crate)"]
    change: BigInt,

    #[get = "pub(crate)"]
    frozen_balance_by_cycle: Vec<BalanceByCycle>,

    #[get = "pub(crate)"]
    delegator_list: Vec<String>,
    
    #[get = "pub(crate)"]
    delegate_activity: DelegateActivity,
}

impl DelegateRawContextData {
    pub fn new(
        balance: BigInt,
        change: BigInt,
        frozen_balance_by_cycle: Vec<BalanceByCycle>,
        delegator_list: Vec<String>,
        delegate_activity: DelegateActivity,
    ) -> Self {
        Self {
            balance,
            change,
            frozen_balance_by_cycle,
            delegator_list,
            delegate_activity,
        }
    }
}

#[derive(Setters, Getters, CopyGetters)]
pub struct DelegateActivity {
    #[set = "pub(crate)"]
    #[get_copy = "pub(crate)"]
    deactivated: bool,

    #[get_copy = "pub(crate)"]
    grace_period: i32,
}

impl DelegateActivity {
    pub fn new(
        grace_period: i32,
        deactivated: bool,

    ) -> Self {
        Self {
            grace_period,
            deactivated,
        }
    }

    pub fn is_active(&self, current_cycle: i64) -> bool {
        if self.deactivated {
            false
        } else {
            (self.grace_period as i64) > current_cycle
        }
    }
}
