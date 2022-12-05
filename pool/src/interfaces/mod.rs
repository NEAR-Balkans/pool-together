pub mod pool {

    use near_sdk::{AccountId, Balance};

    use super::defi::IYieldSource;
    pub trait IPool{
        fn assert_correct_token_is_send_to_contract(&self, token: &AccountId);
        fn assert_sender_has_deposited_enough(&self, account: &AccountId, deposit:Balance);
        fn get_lottery_asset(&self) -> AccountId;
        fn get_yield_source(&self) -> Box<dyn IYieldSource>;
    }

    pub trait ITwab{
        fn increase_balance(&mut self, account: &AccountId, amount: Balance, current_time: u64);
        fn decrease_balance(&mut self, account: &AccountId, amount: Balance, current_time: u64);
        fn increase_total_supply(&mut self, amount: Balance, current_time: u64);
        fn decrease_total_supply(&mut self, amount: Balance, current_time: u64);

        fn average_balance_between_timestamps(
            &self, 
            account: &AccountId, 
            start_time: u64, 
            end_time: u64
        ) -> Balance;

        fn average_total_supply_between_timestamps(
            &self, 
            start_time: u64, 
            end_time: u64
        ) -> Balance;
    }

}

pub mod defi {
    use common::types::{DrawId, NumPicks};
    use near_sdk::{Balance, borsh::{self, BorshDeserialize, BorshSerialize}, AccountId, Promise, Gas};

    #[derive(BorshDeserialize, BorshSerialize)]
    pub enum YieldSource{
        Burrow { address: AccountId },
        Metapool { address: AccountId }
    }

    pub enum YieldSourceAction{
        Transfer,
        Claim,
        GetReward,
        Withdraw,
    }

    pub trait IYieldSource{
        fn get_reward(&self) -> Promise;
        fn transfer(&self, token_id: &AccountId, amount: Balance);
        fn claim(&self, account_id: &AccountId, token_id:&AccountId, amount: Balance, draw_id: DrawId, pick: NumPicks);
        fn get_action_required_deposit_and_gas(&self, action: YieldSourceAction) -> (Balance, Gas);
        fn withdraw(&self, receiver_id: &AccountId, token_id: &AccountId, amount: Balance);
    }
}

pub mod prize_distribution{
    const MAX_TIERS:usize = 16;
    use common::types::{DrawId, WinningNumber, NumPicks};
    use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
    use near_sdk::json_types::U128;
    use near_sdk::serde::{Serialize, Deserialize};

    #[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(crate = "near_sdk::serde")]
    pub struct PrizeDistribution{
        pub draw_id: DrawId,
        pub cardinality: u8,
        pub bit_range_size: u8,
        pub tiers: [u32; MAX_TIERS],
        pub prize: u128,
        pub max_picks: NumPicks,
        pub start_time: u64,
        pub end_time: u64,
        pub winning_number: WinningNumber,
    }

    pub trait PrizeDistributionActor{
        fn get_prize_distribution(&self, draw_id: DrawId) -> PrizeDistribution;
        fn add_prize_distribution(&mut self, draw_id: DrawId, prize_awards: U128, cardinality: u8, bit_range_size: u8);
        fn claim(&mut self, draw_id: DrawId, pick: U128) -> U128;
    }
}

pub mod picker{
    use common::types::{DrawId, NumPicks};
    use near_sdk::{PromiseOrValue};
    pub trait Picker{
        fn get_picks(&self, draw_id: DrawId) -> PromiseOrValue<NumPicks>;
    }
}