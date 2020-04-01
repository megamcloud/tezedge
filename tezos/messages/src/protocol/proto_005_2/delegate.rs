// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use serde::Serialize;

use crate::base::signature_public_key_hash::SignaturePublicKeyHash;
use crate::protocol::{ToRpcJsonMap, UniversalValue};
use tezos_encoding::types::BigInt;

#[derive(Serialize)]
pub struct BalanceByCycle {
    cycle: i32,
    deposit: BigInt,
    fees: BigInt,
    rewards: BigInt,
}

impl BalanceByCycle {
    pub fn new(
        cycle: i32,
        deposit: BigInt,
        fees: BigInt,
        rewards: BigInt,
    ) -> Self {
        Self {
            cycle,
            deposit,
            fees,
            rewards,
        }
    }
}

impl ToRpcJsonMap for BalanceByCycle {
    fn as_map(&self) -> HashMap<&'static str, UniversalValue> {
        let mut ret: HashMap<&'static str, UniversalValue> = Default::default();
        ret.insert("cycle", UniversalValue::num(self.cycle));
        ret.insert("deposit", UniversalValue::big_num(self.deposit.clone()));
        ret.insert("fees", UniversalValue::big_num(self.fees.clone()));
        ret.insert("rewards", UniversalValue::big_num(self.rewards.clone()));
        ret
    }
}

#[derive(Serialize)]
pub struct Delegate {
    balance: BigInt,
    frozen_balance: BigInt,
    frozen_balance_by_cycle: Vec<BalanceByCycle>,
    staking_balance: BigInt,
    delegated_contracts: Vec<String>,
    delegated_balance: BigInt,
    deactivated: bool,
    grace_period: i64,
}

impl Delegate {
    /// Simple constructor to construct VoteListings
    pub fn new(
        balance: BigInt,
        frozen_balance: BigInt,
        frozen_balance_by_cycle: Vec<BalanceByCycle>,
        staking_balance: BigInt,
        delegated_contracts: Vec<String>,
        delegated_balance: BigInt,
        deactivated: bool,
        grace_period: i64,
    ) -> Self {
        Self {
            balance,
            frozen_balance,
            frozen_balance_by_cycle,
            staking_balance,
            delegated_contracts,
            delegated_balance,
            deactivated,
            grace_period
        }
    }
}

impl ToRpcJsonMap for Delegate {
    fn as_map(&self) -> HashMap<&'static str, UniversalValue> {
        let mut ret: HashMap<&'static str, UniversalValue> = Default::default();
        ret.insert("balance", UniversalValue::big_num(self.balance.clone()));
        // ret.insert("delegate", UniversalValue::string(self.delegate.to_string()));
        // ret.insert("slots", UniversalValue::num_list(self.slots.iter()));
        ret.insert("frozen_balance", UniversalValue::big_num(self.frozen_balance.clone()));
        //ret.insert("level", UniversalValue::num(self.level));
        ret.insert("staking_balance", UniversalValue::big_num(self.staking_balance.clone()));
        // ret.insert("delegated_contracts", UniversalValue::num(self.level));
        ret.insert("delegated_balance", UniversalValue::big_num(self.delegated_balance.clone()));
        ret.insert("deactivated", UniversalValue::num(self.deactivated));
        ret.insert("grace_period", UniversalValue::i64(self.grace_period));

        ret
    }
}