use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;

use near_sdk::collections::{ UnorderedMap};
use near_sdk::json_types::{U128, ValidAccountId};
use near_sdk::{self, near_bindgen, ext_contract, log, env, AccountId, Balance, PromiseResult, Gas};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

pub const ON_FUNDING_GAS: Gas = 10_000_000_000_000;

pub const GAS_FOR_FT_TRANSFER: Gas = 50_000_000_000_000;

mod events;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct TokensBalances{
    /// first variable shows the balance, second shows the reward tally
    pub token_id_balance: UnorderedMap<AccountId, (Balance, Balance)>,
    pub tally_below_zero: bool,
}

impl Default for TokensBalances {
    fn default() -> Self {
        Self { 
            token_id_balance: UnorderedMap::new(b"tib".to_vec()),
            tally_below_zero: false 
        }
    }
}


#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Contract {
    pub accounts: UnorderedMap<AccountId, TokensBalances>,
} 

impl Default for Contract{
    fn default() -> Self {
        Self {
            accounts:UnorderedMap::new(b"a"),
        }
    }
}

#[ext_contract(ext_contract)]
pub trait ExtFt {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: String, memo: Option<String>);
    fn ft_transfer_call(
        receiver_id: AccountId,
        amount: String,
        memo: Option<String>,
        msg: String,
    ) -> Promise;
}

#[ext_contract(ext_self)]
pub trait ExtSelf{
    fn on_after_ft_transfer(&mut self, account_id: AccountId, token_id: AccountId, amount: U128) -> bool;
}

#[near_bindgen]
impl Contract{
    #[init]
    pub fn new() -> Self{
        assert!(!env::state_exists(), "Already initialized");

        Contract::default()
    }

    pub fn withdraw(&mut self, _account_id: &AccountId, _amount: Balance){
       
    }

    #[payable]
    pub fn get_reward(&mut self){
        let caller = env::predecessor_account_id();
        assert!(env::attached_deposit() >= 1, "1 yocto for ft transfer is needed");

        let acc_amounts = self.accounts.get(&caller).unwrap_or_default();
        for amount in acc_amounts.token_id_balance.to_vec().iter(){
            let amount_to_send = if acc_amounts.tally_below_zero {
                (amount.1.0 + amount.1.1) / 10
            } else {
                (amount.1.0 - amount.1.1) / 10
            };

            if amount_to_send > 0{
                ext_contract::ft_transfer_call(
                    caller.clone(),
                    amount_to_send.to_string(),
                    Option::None,
                    "".to_string(),
                    &amount.0,
                    1,
                    GAS_FOR_FT_TRANSFER,
                ).then(ext_self::on_after_ft_transfer(
                    caller.clone(),
                    amount.0.clone(),
                    amount_to_send.into(),
                    &env::current_account_id(),
                    0,
                    ON_FUNDING_GAS,
                ));
            }            
        }
    }
 
 
    #[private]
    pub fn on_after_ft_transfer(&mut self, account_id: AccountId, token_id: AccountId, amount: U128) -> bool {
        let promise_result = env::promise_result(0);
        let transfer_success = match promise_result {
            PromiseResult::Successful(_) => true,
            _ => false
        };

        if transfer_success{
            events::events::withdraw_success(&account_id, amount.0, &token_id);

            let mut acc_amount = self.accounts.get(&account_id).unwrap_or_default();
            let mut amounts = acc_amount.token_id_balance.get(&token_id).unwrap_or_default();
            amounts.1 += amount.0;
            acc_amount.token_id_balance.insert(&token_id, &amounts);
            self.accounts.insert(&account_id, &acc_amount);

        }else {
            events::events::withdraw_failed(&account_id, amount.0, &token_id);
        }

        return transfer_success;
    }
 
    /*
    pub fn get_reward(&mut self) -> Promise {
        let caller = env::predecessor_account_id();

        ext_ft::ft_transfer()

        let acc_amounts = self.accounts.get(&caller).unwrap_or_default();
        for x in acc_amounts.token_id_balance.to_vec().iter(){
            
        }


        
        if amount > 0 {
            ext_ft::ft_transfer_call(
                caller.clone(),
                amount.to_string(),
                None,
                "reward".to_string(),

            ).


            ext_ft::ft_transfer_call(
                self.lender.clone(),
                format!(
                    "{}",
                    self.collateral.1 - self.shares.collateral_to_liquidate as u128
                ),
                None,
                "liquidation".to_string(),
                self.collateral.0.clone(),
                SECURITY_DEPOSIT,
                gas::TRANSFER_GAS,
            )
            .then(ext_self::on_user_transfer(
                self.state.clone(),
                env::current_account_id(),
                NO_DEPOSIT,
                gas::CALLBACK_GAS,
            ))

    }*/
}

impl FungibleTokenReceiver for Contract{
    fn ft_on_transfer(
        &mut self,
        sender_id: ValidAccountId,
        amount: near_sdk::json_types::U128,
        _msg: String,
    ) -> near_sdk::PromiseOrValue<near_sdk::json_types::U128> {
        let acc_id = sender_id.clone();
        let token_id = env::predecessor_account_id();
        log!("Predecesor {} Signer {} Sender {}", env::predecessor_account_id(), env::signer_account_id(), sender_id);
        let mut account_tokens = self.accounts.get(&acc_id.to_string()).unwrap_or_default();
        let mut token_balance = account_tokens.token_id_balance.get(&token_id).unwrap_or_default();
        token_balance.0 += amount.0;

        account_tokens.token_id_balance.insert(&token_id, &token_balance);
        self.accounts.insert(&acc_id.to_string(), &account_tokens);

        return near_sdk::PromiseOrValue::Value(U128(0));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
