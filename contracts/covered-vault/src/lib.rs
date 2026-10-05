//! Reference consumer: a single-asset deposit vault that refuses deposits
//! while the asset is not covered according to the coverage ledger.
//! Withdrawals are never blocked.
#![no_std]

use plimsoll_types::{
    CoverageLedgerClient, Tier, INSTANCE_EXTEND_TO, INSTANCE_THRESHOLD, PERSISTENT_EXTEND_TO,
    PERSISTENT_THRESHOLD,
};
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Env,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultConfig {
    pub ledger: Address,
    pub asset: Address,
    pub min_bps: u32,
    pub max_age: u64,
    pub min_tier: Tier,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Config,
    Balance(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VaultError {
    InvalidAmount = 1,
    NotCovered = 2,
    InsufficientBalance = 3,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deposited {
    #[topic]
    pub from: Address,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Withdrawn {
    #[topic]
    pub to: Address,
    pub amount: i128,
}

#[contract]
pub struct CoveredVault;

fn config(env: &Env) -> VaultConfig {
    env.storage()
        .instance()
        .get(&DataKey::Config)
        .expect("config is set in the constructor")
}

fn balance_of(env: &Env, who: &Address) -> i128 {
    let key = DataKey::Balance(who.clone());
    let balance = env.storage().persistent().get::<_, i128>(&key);
    if balance.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_THRESHOLD, PERSISTENT_EXTEND_TO);
    }
    balance.unwrap_or(0)
}

fn set_balance(env: &Env, who: &Address, amount: i128) {
    let key = DataKey::Balance(who.clone());
    if amount == 0 {
        env.storage().persistent().remove(&key);
        return;
    }
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_THRESHOLD, PERSISTENT_EXTEND_TO);
}

#[contractimpl]
impl CoveredVault {
    pub fn __constructor(
        env: Env,
        ledger: Address,
        asset: Address,
        min_bps: u32,
        max_age: u64,
        min_tier: Tier,
    ) {
        let cfg = VaultConfig {
            ledger,
            asset,
            min_bps,
            max_age,
            min_tier,
        };
        env.storage().instance().set(&DataKey::Config, &cfg);
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_THRESHOLD, INSTANCE_EXTEND_TO);
    }

    pub fn deposit(env: Env, from: Address, amount: i128) -> i128 {
        from.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, VaultError::InvalidAmount);
        }
        let cfg = config(&env);
        let covered = CoverageLedgerClient::new(&env, &cfg.ledger).is_covered(
            &cfg.asset,
            &cfg.min_bps,
            &cfg.max_age,
            &cfg.min_tier,
        );
        if !covered {
            panic_with_error!(&env, VaultError::NotCovered);
        }
        token::TokenClient::new(&env, &cfg.asset).transfer(
            &from,
            env.current_contract_address(),
            &amount,
        );
        let new_balance = balance_of(&env, &from)
            .checked_add(amount)
            .unwrap_or_else(|| panic_with_error!(&env, VaultError::InvalidAmount));
        set_balance(&env, &from, new_balance);
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_THRESHOLD, INSTANCE_EXTEND_TO);
        Deposited { from, amount }.publish(&env);
        new_balance
    }

    pub fn withdraw(env: Env, to: Address, amount: i128) -> i128 {
        to.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, VaultError::InvalidAmount);
        }
        let current = balance_of(&env, &to);
        if amount > current {
            panic_with_error!(&env, VaultError::InsufficientBalance);
        }
        let new_balance = current - amount;
        set_balance(&env, &to, new_balance);
        let cfg = config(&env);
        token::TokenClient::new(&env, &cfg.asset).transfer(
            &env.current_contract_address(),
            &to,
            &amount,
        );
        Withdrawn { to, amount }.publish(&env);
        new_balance
    }

    pub fn balance(env: Env, of: Address) -> i128 {
        balance_of(&env, &of)
    }

    pub fn config(env: Env) -> VaultConfig {
        config(&env)
    }
}

#[cfg(test)]
mod test;
