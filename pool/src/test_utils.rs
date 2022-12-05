use near_sdk::{AccountId, json_types::U128, Balance};

use crate::{Contract, twab::AccountsDepositHistory, interfaces::pool::ITwab};

pub fn mmmm() -> AccountId{
    AccountId::new_unchecked("mmmm".to_string())
}

pub fn sec() -> AccountId{
    AccountId::new_unchecked("sec".to_string())
}

pub fn burrow() -> AccountId{
    AccountId::new_unchecked("burrow".to_string())
}

pub fn get_contract() -> Contract{
    return Contract::new_default_meta(
        mmmm(), 
        AccountId::new_unchecked("usdc".to_string()), 
        sec(),
        burrow(),
        AccountId::new_unchecked("usdc".to_string()),
        U128(10000000000000000000000)
    );
}

pub fn mint(tickets: &mut AccountsDepositHistory, acc_id: &AccountId, amount: Balance, time: u64){
    tickets.increase_balance(acc_id, amount, time);
    tickets.increase_total_supply(amount, time);
}

pub fn burn(tickets: &mut AccountsDepositHistory, acc_id: &AccountId, amount: Balance, time: u64){
    tickets.decrease_balance(acc_id, amount, time);
    tickets.decrease_total_supply(amount, time);
}