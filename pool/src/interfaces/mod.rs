pub mod pool {

    use near_sdk::{AccountId, Balance};

    use super::defi::IYieldSource;
    pub trait IPool{
        fn assert_correct_token_is_send_to_contract(&self, token: &AccountId);
        fn assert_sender_has_payed_for_ft_transfer(&self, account: &AccountId);
        fn get_lottery_asset(&self) -> AccountId;
        fn send_to_dex(&self);
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
    use near_sdk::{Balance, borsh::{self, BorshDeserialize, BorshSerialize}, AccountId, PromiseOrValue, Promise};

    use crate::Contract;

    #[derive(BorshDeserialize, BorshSerialize)]
    pub enum YieldSource{
        Burrow { address: AccountId },
        Metapool { address: AccountId }
    }

    pub trait IYieldSource{
        fn get_reward(&self, account_id: &AccountId) -> Promise;
        fn transfer(&self, token_id: &AccountId, amount: Balance);
        fn claim(&self, account_id: &AccountId, token_id:&AccountId, amount: Balance);
    }
}

pub mod prize_distribution{
    const MAX_TIERS:usize = 16;
    use common::types::{NumPicks, DrawId, WinningNumber};
    use near_sdk::Balance;
    use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
    use near_sdk::json_types::U128;
    use near_sdk::serde::{Serialize, Deserialize};

    #[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(crate = "near_sdk::serde")]
    pub struct PrizeDistribution{
        pub number_of_picks: u64,
        pub draw_id: u128,
        pub cardinality: u8,
        pub bit_range_size: u8,
        pub tiers: [u32; MAX_TIERS],
        pub prize: u128,
        pub max_picks: u128,
        pub start_time: u64,
        pub end_time: u64,
        #[serde(skip_serializing)]
        pub winning_number: WinningNumber,
    }
    pub trait PrizeDistributionActor{
        fn get_prize_distribution(&self, draw_id: u128) -> PrizeDistribution;
        fn add_prize_distribution(&mut self, draw_id: u128, prize_awards: Balance);
        fn claim(&mut self, draw_id: U128, pick: U128) -> u128;
    }
}

pub mod picker{
    use common::types::{DrawId, NumPicks};
    use near_sdk::{PromiseOrValue};
    pub trait Picker{
        fn get_picks(&self, draw_id: DrawId) -> PromiseOrValue<NumPicks> ;
    }
}