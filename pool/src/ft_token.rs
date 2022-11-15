use crate::*;
use near_sdk::ext_contract;

impl Contract{
    pub (crate) fn mint_tokens(&mut self, account_id: AccountId, balance: Balance){       
        if !self.token.accounts.contains_key(&account_id) {
            self.token.internal_register_account(&account_id);
        }
        self.token.internal_deposit(&account_id, balance);
        
        near_contract_standards::fungible_token::events::FtMint {
            owner_id: &account_id,
            amount: &U128(balance),
            memo: Some("Tokens supply is minted"),
        }
        .emit();

        let current_time = env::block_timestamp_ms();
        self.tickets.increase_balance(&account_id, balance, current_time);
        self.tickets.increase_total_supply(balance, current_time);
    }

    pub (crate) fn burn_tokens(&mut self, account_id: AccountId, balance: Balance){
        self.token.internal_transfer(&account_id, &AccountId::new_unchecked(ZERO_ADDRESS.to_string()), balance, Option::None);
        let current_time = env::block_timestamp_ms();

        self.tickets.decrease_balance(&account_id, balance, current_time);
        self.tickets.decrease_total_supply(balance, current_time);
    }

    pub (crate) fn on_tokens_burned(&mut self, account_id: AccountId, amount: Balance) {
        log!("Account @{} burned {}", account_id, amount);
    }

    pub (crate) fn on_account_closed(&mut self, account_id: AccountId, balance: Balance) {
        log!("Closed @{} with {}", account_id, balance);
    }

}

