use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;

use near_sdk::collections::{ UnorderedMap};
use near_sdk::json_types::{U128};
use near_sdk::serde::{Serialize, Deserialize};
use near_sdk::serde_json::json;
use near_sdk::{self, near_bindgen, ext_contract, log, env, AccountId, Balance, PromiseResult, Gas, Promise};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use std::vec::Vec;

pub const ON_FUNDING_GAS: Gas = near_sdk::Gas(10_000_000_000_000);

pub const GAS_FOR_FT_TRANSFER: Gas = near_sdk::Gas(10_000_000_000_000);

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
    pub token_id_balance: UnorderedMap<AccountId, (NumShares, Balance)>
}

impl Default for TokensBalances {
    fn default() -> Self {
        Self { 
            token_id_balance: UnorderedMap::new(b"tib".to_vec())
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct AssetView {
    pub token_id: AccountId,
    //#[serde(with = "u128_dec_format")]
    pub balance: U128,
    /// The number of shares this account holds in the corresponding asset pool
    pub shares: U128,
}

impl Default for AssetView{
    fn default() -> Self {
        return Self { token_id: AccountId::new_unchecked("".to_string()), balance: Balance::default().into(), shares: U128(0) };
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct AccountDetailedView {
    pub account_id: AccountId,
    /// A list of assets that are supplied by the account (but not used a collateral).
    pub supplied: Vec<AssetView>,
    /// A list of assets that are used as a collateral.
    pub collateral: Vec<AssetView>,
    /// A list of assets that are borrowed.
    pub borrowed: Vec<AssetView>
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
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
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

#[derive(Serialize)]
    #[serde(crate = "near_sdk::serde")]
    struct AccountAmountToken<'a> {
        pub account_id: &'a AccountId,
        pub amount: U128,
        pub token_id: &'a TokenId,
    }

#[near_bindgen]
impl Contract{
    #[init]
    pub fn new() -> Self{
        assert!(!env::state_exists(), "Already initialized");

        Contract::default()
    }

    pub fn get_account(&self) -> Option<AccountDetailedView>{
        let acc_balances = self.accounts.get(&env::predecessor_account_id()).unwrap_or_default();
        let result = AccountDetailedView{
            account_id: env::predecessor_account_id(),
            supplied: acc_balances
                    .token_id_balance
                    .into_iter()
                    .map(|(token_id, (balance, reward))| AssetView { token_id: token_id, shares: U128(balance), balance: U128(balance + reward) } )
                    .collect(),
            collateral: Vec::<AssetView>::new(),
            borrowed: Vec::<AssetView>::new(),
        };

        log!("Res {:?}", result);

        return Some(result);
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

    fn log_event<T: Serialize>(event: &str, data: T) {
        let event = json!({
            "standard": "burrow",
            "version": "1.0.0",
            "event": event,
            "data": [data]
        });

        log!("EVENT_JSON:{}", event.to_string());
    }

    fn withdraw_started(account_id: &AccountId, amount: Balance, token_id: &TokenId) {
        Self::log_event(
            "withdraw_started",
            AccountAmountToken {
                account_id: &account_id,
                amount: amount.into(),
                token_id: &token_id,
            },
        );
    }

    #[payable]
    pub fn execute(&mut self, actions: Vec<Action>){
        let caller = env::predecessor_account_id();
        assert!(env::attached_deposit() >= 1, "1 yocto for ft transfer is needed");

        let acc_amounts = self.accounts.get(&caller).unwrap_or_default();
        
        for action in actions{
            match action{
                Action::Withdraw(asset_amount) => {
                    let token_amount = acc_amounts.token_id_balance.get(&asset_amount.token_id).unwrap_or_default();

                    if asset_amount.amount.unwrap().0 > token_amount.1{
                        panic!("Trying to send amount that is not available");
                    }
                    log!("DEFI Send {} {} to {}", asset_amount.amount.unwrap().0, asset_amount.token_id, caller.clone());

                    ext_fungible_token::ft_transfer(caller.clone(), asset_amount.amount.unwrap(), None, asset_amount.token_id.clone(), 1, GAS_FOR_FT_TRANSFER);

                    Self::withdraw_started(&caller.clone(), asset_amount.amount.unwrap().0, &asset_amount.token_id);
                }
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
        log!("token balances {:?}", token_balance);
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
        let x = vec![1,2,3];
        let mut contract = Contract::default();
        let res = contract.ft_on_transfer(AccountId::new_unchecked("test.near".to_string()), U128(10), "".to_string());
        let shares = contract.show_reward(AccountId::new_unchecked("test.near".to_string())).first().unwrap().shares;
        assert_eq!(shares.0, 10);
        contract.ft_on_transfer(AccountId::new_unchecked("test.near".to_string()), U128(5), "test.near".to_string());
        let reward = contract.show_reward(AccountId::new_unchecked("test.near".to_string())).first().unwrap().rewards;
        assert_eq!(reward.0, 5);
    }
}
