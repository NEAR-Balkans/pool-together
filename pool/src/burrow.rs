use crate::*;
use near_sdk::{AccountId, Balance, json_types::{U128}, PromiseError};
use crate::interfaces::defi::IYieldSource;
use near_sdk::serde::{Deserialize};
use crate::utils::gas;

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
    pub balance: Balance,
    /// The number of shares this account holds in the corresponding asset pool
    pub shares: U128,
}

pub (crate) struct BurrowYieldSource{
    pub(crate) address: AccountId,
}

impl IYieldSource for BurrowYieldSource{
    fn get_balance(&self) -> Balance {
        return 0;
    }

    fn transfer(&self, token_id: &AccountId, amount: Balance) {
        ext_fungible_token::ext(token_id.clone())
        .with_static_gas(gas::GAS_FOR_FT_TRANSFER)
        .ft_transfer(self.address.clone(), amount.into(), None);
    }
    
}