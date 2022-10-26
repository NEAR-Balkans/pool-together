use crate::{*, utils::gas::{GAS_FOR_AFTER_FT_TRANSFER, GAS_FOR_FT_TRANSFER}};
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

    #[private]
    pub fn on_after_rewards_claim_from_defi(&mut self, account_id: AccountId, amount: Balance, #[callback_result] result: Result<(), PromiseError>){
        if result.is_err(){
            log!("Error when claiming rewards from defi");
            return;
        }

        // Send to user account
        ext_fungible_token::ft_transfer(account_id, U128(amount), None, self.deposited_token_id.clone(), 1, GAS_FOR_FT_TRANSFER);
    }
}

impl IYieldSource for BurrowYieldSource{
    fn get_reward(&self, account_id: &AccountId) -> Promise {

        ext_defi::show_reward(account_id.clone(), self.address.clone(), 0, gas::GAS_FOR_FT_TRANSFER)
         .then(crate::this_contract::on_get_reward_from_defi(env::current_account_id(), 0, gas::GET_BALANCE_FROM_DEFI))
    }

    fn transfer(&self, token_id: &AccountId, amount: Balance) {
        log!("Transfer to {} {}", self.address, token_id.clone());
        log!("burrow transfer prepaid:{} used: {}", env::prepaid_gas().0, env::used_gas().0);

        ext_fungible_token::ft_transfer_call(self.address.clone(), amount.into(), None, "".to_string(), token_id.clone(), 1, gas::GAS_FOR_TRANSFER_TO_DEFI);
    }


    fn claim(&self, account_id: &AccountId, token_id: &AccountId, amount: Balance) {
        let asset_amount = AssetAmount{ token_id: token_id.clone(), amount: Some(U128(amount)), max_amount: None};
        let action = Action::Withdraw(
            asset_amount
        );

        ext_defi::execute(vec![action], self.address.clone(), 1, gas::MAX_GAS)
        .then(crate::this_contract::on_after_rewards_claim_from_defi(account_id.clone(), amount, token_id.clone(), 1, gas::GAS_FOR_FT_TRANSFER));
    }
    
}