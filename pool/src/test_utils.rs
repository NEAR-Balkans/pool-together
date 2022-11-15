use near_sdk::AccountId;

use crate::{Contract};

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
    );
}
