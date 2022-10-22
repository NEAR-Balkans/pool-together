use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;

use near_sdk::collections::{ UnorderedMap};
use near_sdk::json_types::{U128};
use near_sdk::serde::{Serialize, Deserialize};
use near_sdk::{self, near_bindgen, ext_contract, log, env, AccountId, Balance, PromiseResult, Gas, Promise};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

pub const ON_FUNDING_GAS: Gas = near_sdk::Gas(10_000_000_000_000);

pub const GAS_FOR_FT_TRANSFER: Gas = near_sdk::Gas(50_000_000_000_000);

mod events;

pub(crate) type TokenId = AccountId;
pub(crate) type NumShares = Balance;

#[derive(Deserialize, Debug, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct AssetAmount {
    pub token_id: TokenId,
    /// The amount of tokens intended to be used for the action
    /// If 'None', then the maximum will be tried
    pub amount: Option<U128>,
    /// The maximum amount of tokens that can be used for the action
    /// If 'None', then the maximum 'available' amount will be used
    pub max_amount: Option<U128>,
}

#[derive(Deserialize, Debug, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub enum Action{
    Withdraw(AssetAmount)
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct TokensBalances{
    /// first variable shows the balance, second shows the reward tally
    pub token_id_balance: UnorderedMap<AccountId, (NumShares, Balance)>,
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

#[ext_contract(ext_fungible_token)]
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct TokenAmountsView{
    token: TokenId,
    shares: U128,
    rewards: U128
}

#[near_bindgen]
impl Contract{
    #[init]
    pub fn new() -> Self{
        assert!(!env::state_exists(), "Already initialized");

        Contract::default()
    }

    pub fn show_reward(&self, account_id: AccountId) -> Vec<TokenAmountsView>{
        let acc: TokensBalances = self
            .accounts
            .get(&account_id)
            .unwrap_or_default();
        
        return acc.token_id_balance.iter().map(|(token_id, (share, reward))|{
            TokenAmountsView { token: token_id, shares: U128(share), rewards: U128(reward) }
        }).collect::<Vec<TokenAmountsView>>();
    }

    pub fn account_farm_claim_all(&self){}

    #[payable]
    pub fn execute(&mut self, _actions: Vec<Action>){
        let caller = env::predecessor_account_id();
        assert!(env::attached_deposit() >= 1, "1 yocto for ft transfer is needed");

        let acc_amounts = self.accounts.get(&caller).unwrap_or_default();
        for amount in acc_amounts.token_id_balance.to_vec().iter(){
            let _amount_to_send = if acc_amounts.tally_below_zero {
                (amount.1.0 + amount.1.1) / 10
            } else {
                (amount.1.0 - amount.1.1) / 10
            };
            /*
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
             */        
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

#[near_bindgen]
impl FungibleTokenReceiver for Contract{
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: near_sdk::json_types::U128,
        msg: String,
    ) -> near_sdk::PromiseOrValue<near_sdk::json_types::U128> {
        let acc_id: AccountId = if msg.is_empty() {
            sender_id.clone()
        }else{
            AccountId::new_unchecked(msg.clone())
        };

        let token_id = env::predecessor_account_id();
        log!("Predecesor {} Signer {} Sender {} amount {} acc_id {}", env::predecessor_account_id(), env::signer_account_id(), &sender_id, amount.0, acc_id);
        let mut account_tokens: TokensBalances = self.accounts.get(&acc_id).unwrap_or_default();
        let mut token_balance: (NumShares, Balance) = account_tokens.token_id_balance.get(&token_id).unwrap_or_default();
        if msg.is_empty(){
            token_balance.0 += amount.0;
        }else{
            token_balance.1 += amount.0;
        }
        account_tokens.token_id_balance.insert(&token_id, &token_balance);
        self.accounts.insert(&acc_id, &account_tokens);

        return near_sdk::PromiseOrValue::Value(U128(0));
    }
}

#[cfg(test)]
mod tests {
    use near_sdk::{json_types::U128, AccountId};
    use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;

    use crate::Contract;

    #[test]
    fn it_works() {
        let mut contract = Contract::default();
        let res = contract.ft_on_transfer(AccountId::new_unchecked("test.near".to_string()), U128(10), "".to_string());
        let shares = contract.show_reward(AccountId::new_unchecked("test.near".to_string())).first().unwrap().shares;
        assert_eq!(shares.0, 10);
        contract.ft_on_transfer(AccountId::new_unchecked("test.near".to_string()), U128(5), "test.near".to_string());
        let reward = contract.show_reward(AccountId::new_unchecked("test.near".to_string())).first().unwrap().rewards;
        assert_eq!(reward.0, 5);
    }
}
