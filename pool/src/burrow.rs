use crate::*;
use near_sdk::{AccountId, Balance, json_types::{U128}, PromiseError, Promise};
use crate::interfaces::defi::IYieldSource;
use near_sdk::serde::{Deserialize};
use crate::utils::gas;
use near_sdk::ext_contract;

// Callback
#[ext_contract(this_contract)]
pub trait ExtSelf {
    fn on_get_account_from_burrow(&mut self, #[callback_result] call_result: Result<AccountDetailedView, PromiseError>);
}

#[derive(Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct AccountDetailedView{
    pub account_id: AccountId,
    /// A list of assets that are supplied by the account (but not used a collateral).
    pub supplied: Vec<AssetView>,
}

#[derive(Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct AssetView {
    pub token_id: AccountId,
    //#[serde(with = "u128_dec_format")]
    pub balance: U128,
    /// The number of shares this account holds in the corresponding asset pool
    pub shares: U128,
}

pub (crate) struct BurrowYieldSource{
    pub(crate) address: AccountId,
}

#[near_bindgen]
impl Contract{
    #[private]
    pub fn on_get_reward_from_defi(&self, #[callback_result] call_result: Result<Vec<TokenAmountsView>, PromiseError>)-> Balance{
        return 10;
    }
}

impl IYieldSource for BurrowYieldSource{
    fn get_reward(&self, account_id: &AccountId) -> Promise {

        ext_defi::ext(self.address.clone())
            .with_static_gas(gas::GAS_FOR_FT_TRANSFER)
            .show_reward(account_id.clone())
        .then(Contract::ext(env::current_account_id())
            .with_static_gas(gas::GET_BALANCE_FROM_DEFI)
            .on_get_reward_from_defi())
    }

    fn transfer(&self, token_id: &AccountId, amount: Balance) {
        log!("Transfer to {} {}", self.address, token_id.clone());
        log!("burrow transfer prepaid:{} used: {}", env::prepaid_gas().0, env::used_gas().0);

        ext_fungible_token::ext(token_id.clone())
        .with_attached_deposit(1)
        .with_static_gas(gas::GAS_FOR_FT_TRANSFER)
        .ft_transfer_call(self.address.clone(), amount.into(), None, "".to_string());
    }
    
}