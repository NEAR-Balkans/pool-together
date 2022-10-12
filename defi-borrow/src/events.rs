pub mod events{
    use near_sdk::json_types::U128;
    use near_sdk::{AccountId, Balance, log};
    use near_sdk::serde::{Serialize};
    use near_sdk::serde_json::json;

    #[derive(Serialize)]
    #[serde(crate = "near_sdk::serde")]
    struct Event<'a> {
        pub account_id: &'a AccountId,
        pub amount: U128,
        pub token_id: &'a AccountId,
    }

    fn log_event<T: Serialize>(event: &str, data: T) {
        let event = json!({
            "standard": "defi-borrow",
            "version": "1.0.0",
            "event": event,
            "data": [data]
        });

        log!("EVENT_JSON:{}", event.to_string());
    }

    pub fn withdraw_success(account_id: &AccountId, amount: Balance, token_id: &AccountId){
        log_event(
            "withdraw_success", 
            Event {
                account_id: &account_id,
                amount: U128(amount),
                token_id: &token_id,
            }
        );
    }

    pub fn withdraw_failed(account_id: &AccountId, amount: Balance, token_id: &AccountId){
        log_event(
            "withdraw_failed", 
            Event {
                account_id: &account_id,
                amount: U128(amount),
                token_id: &token_id,
            }
        );
    }
}

