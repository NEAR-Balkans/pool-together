use near_sdk::AccountId;
use near_sdk::Balance;

pub fn staking() -> AccountId {
    "staking".parse().unwrap()
}

pub fn alice() -> AccountId {
    "alice".parse().unwrap()
}
pub fn bob() -> AccountId {
    "bob".parse().unwrap()
}
pub fn owner() -> AccountId {
    "owner".parse().unwrap()
}
pub fn charlie() -> AccountId {
    "charlie".parse().unwrap()
}

pub fn a() -> AccountId {
    "aa".parse().unwrap()
}
pub fn b() -> AccountId {
    "bb".parse().unwrap()
}
pub fn c() -> AccountId {
    "cc".parse().unwrap()
}
pub fn d() -> AccountId {
    "dd".parse().unwrap()
}
pub fn e() -> AccountId {
    "ee".parse().unwrap()
}

pub fn ntoy(near_amount: Balance) -> Balance {
    near_amount * 10u128.pow(24)
}

/// Rounds to nearest
pub fn yton(yocto_amount: Balance) -> Balance {
    (yocto_amount + (5 * 10u128.pow(23))) / 10u128.pow(24)
}

/// Checks that two amount are within epsilon
pub fn almost_equal(left: Balance, right: Balance, epsilon: Balance) -> bool {
    println!("{} ~= {}", left, right);
    if left > right {
        (left - right) < epsilon
    } else {
        (right - left) < epsilon
    }
}

#[macro_export]
macro_rules! assert_eq_in_near {
    ($a:expr, $b:expr) => {
        assert_eq!(yton($a), yton($b))
    };
    ($a:expr, $b:expr, $c:expr) => {
        assert_eq!(yton($a), yton($b), $c)
    };
}

#[cfg(test)]
pub mod tests {
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::{testing_env, VMContext};

    use crate::*;

    use super::*;

    pub const ONE_EPOCH_TS: u64 = 12 * 60 * 60 * 1_000_000_000;

    pub struct Emulator {
        pub contract: Contract,
        pub epoch_height: EpochHeight,
        pub block_index: u64,
        pub block_timestamp: u64,
        pub context: VMContext,
    }

    impl Emulator {
        pub fn new(
        ) -> Self {
            let context = VMContextBuilder::new()
                .current_account_id(owner())
                .account_balance(ntoy(10))
                .build();
            testing_env!(context.clone());
            let contract = Contract::new();
            Emulator {
                contract,
                epoch_height: 0,
                block_timestamp: 0,
                block_index: 0,
                context,
            }
        }

        pub fn update_context(&mut self, random_seed: [u8; 32]) {
            self.context = VMContextBuilder::new()
                .current_account_id(staking())
                .epoch_height(self.epoch_height)
                .block_index(self.block_index)
                .block_timestamp(self.block_timestamp)
                .random_seed(random_seed)
                .build();
            testing_env!(self.context.clone());
            println!(
                "Epoch: {}",
                self.epoch_height
            );
        }

        pub fn skip_epochs(&mut self, num: EpochHeight, random_seed: [u8; 32]) {
            self.epoch_height += num;
            self.block_index += num * 12 * 60 * 60;
            self.block_timestamp += num * ONE_EPOCH_TS;
            self.update_context(random_seed);
        }

    }
}
