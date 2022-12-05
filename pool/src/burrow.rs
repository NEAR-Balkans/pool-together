use crate::{*, utils::gas::{GAS_FOR_FT_TRANSFER, GAS_FOR_TRANSFER_TO_DEFI}, interfaces::defi::YieldSourceAction};
use near_sdk::{AccountId, Balance, Gas, json_types::{U128}, PromiseError, Promise};
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
    pub fn on_get_reward_from_defi(&self, #[callback_result] call_result: Result<Option<AccountDetailedView>, PromiseError>)-> U128{
        if call_result.is_err(){
            log!("Error when getting account");
            return U128(0);
        }

        let opt_account_detailed_view = call_result.unwrap();
        if opt_account_detailed_view.is_none(){
            log!("Account is not present in burrow");
            return U128(0);
        }

        let binding = opt_account_detailed_view.unwrap();
        let supplied_asset = binding.supplied.iter().find(|el| el.token_id == self.deposited_token_id);

        if supplied_asset.is_none(){
            return U128(0);
        }

        return U128(supplied_asset.unwrap().balance.0 - self.tickets.total_supply.balance);
    }

    #[private]
    pub fn on_after_rewards_claim_from_defi(&mut self, account_id: AccountId, amount: U128, draw_id: DrawId, pick: U128, #[callback_result] result: Result<(), PromiseError>){
        if result.is_err(){
            log!("Error when claiming rewards from defi");
            self.acc_picks.remove_claimed_pick_for_draw(&account_id, &draw_id, pick.0);
            return;
        }

        // Send to user account
        ext_fungible_token::ft_transfer(account_id, amount, None, self.deposited_token_id.clone(), 1, GAS_FOR_FT_TRANSFER);
    }

    #[private]
    pub fn on_after_withdraw_tokens_from_defi(&mut self, receiver_id: AccountId, amount: U128, #[callback_result] result: Result<(), PromiseError>){
        if result.is_err(){
            log!("Error when withdrawing rewards from defi");
            return;
        }

        self.burn_tokens(receiver_id.clone(), amount.0);

        // Send to user account
        ext_fungible_token::ft_transfer(receiver_id, amount, None, self.deposited_token_id.clone(), 1, GAS_FOR_FT_TRANSFER);
    }
}

impl IYieldSource for BurrowYieldSource{
    fn get_reward(&self) -> Promise {

        ext_defi::get_account(env::current_account_id(), self.address.clone(), 0, gas::GET_BALANCE_FROM_DEFI)
         .then(crate::this_contract::on_get_reward_from_defi(env::current_account_id(), 0, gas::GET_BALANCE_FROM_DEFI))
    }

    fn transfer(&self, token_id: &AccountId, amount: Balance) {
        log!("Transfer to {} {}", self.address, token_id.clone());
        log!("burrow transfer prepaid:{} used: {}", env::prepaid_gas().0, env::used_gas().0);

        ext_fungible_token::ft_transfer_call(self.address.clone(), amount.into(), None, "".to_string(), token_id.clone(), 1, gas::GAS_FOR_TRANSFER_TO_DEFI);
    }

    fn withdraw(&self, receiver_id: &AccountId, token_id: &AccountId, amount: Balance) {
        let asset_amount = AssetAmount{ token_id: token_id.clone(), amount: Some(U128(amount))};
        let action = Action::Withdraw(
            asset_amount
        );

        ext_defi::execute(vec![action], self.address.clone(), 1, gas::WITHDRAW_TOKENS_EXTERNAL_DEFI)
        .then(crate::this_contract::on_after_withdraw_tokens_from_defi(receiver_id.clone(), amount.into(), env::current_account_id(), 0, gas::WITHDRAW_TOKENS_FROM_DEFI_CALLBACK));
    }


    fn claim(&self, account_id: &AccountId, token_id: &AccountId, amount: Balance, draw_id: DrawId, pick: NumPicks) {
        let asset_amount = AssetAmount{ token_id: token_id.clone(), amount: Some(U128(amount))};
        let action = Action::Withdraw(
            asset_amount
        );

        ext_defi::execute(vec![action], self.address.clone(), 1, gas::CLAIM_REWARDS_EXTERNAL_DEFI)
        .then(crate::this_contract::on_after_rewards_claim_from_defi(account_id.clone(), amount.into(), draw_id, pick.into(), env::current_account_id(), 0, gas::CLAIM_REWARDS_CALLBACK));
    }

    fn get_action_required_deposit_and_gas(&self, action: YieldSourceAction) -> (Balance, Gas){
        let result:(Balance, Gas) = match action {
            YieldSourceAction::GetReward => (0, gas::GET_BALANCE_FROM_DEFI + gas::GET_BALANCE_FROM_DEFI),
            YieldSourceAction::Claim => (1+1, gas::CLAIM_REWARDS_CALLBACK + gas::CLAIM_REWARDS_EXTERNAL_DEFI),
            YieldSourceAction::Transfer => (1, GAS_FOR_TRANSFER_TO_DEFI),
            YieldSourceAction::Withdraw => (1+1, gas::WITHDRAW_TOKENS_EXTERNAL_DEFI + gas::WITHDRAW_TOKENS_FROM_DEFI_CALLBACK)
        };

        return result;
    }
    
}