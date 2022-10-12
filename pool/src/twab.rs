use crate::*;
use interfaces::pool::ITwab;
use near_sdk::collections::{UnorderedMap, Vector};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use utils::storage_keys::StorageKeys;

#[derive(Clone, Debug, Copy, Default, BorshDeserialize, BorshSerialize)]
pub struct Twab{
    pub amount: Balance,
    pub timestamp: u64,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct AccountBalance{
    pub balance: Balance,
    pub twabs: Vector<Twab>,
}

impl Default for AccountBalance{
    fn default() -> Self {
        AccountBalance { 
            balance: Balance::default(), 
            twabs: Vector::new(StorageKeys::AccountBalance) 
        }
    }
}

impl AccountBalance{
    fn get_twab_before_or_at(&self, start_time: u64) -> Twab{
        let mut before_idx = 0;
        for idx in 0..self.twabs.len(){
            let current_twab = self.twabs.get(idx).unwrap_or_default();

            if current_twab.timestamp <= start_time {
                before_idx = idx;
            } else {
                break;
            }
        }

        return self.twabs.get(before_idx).unwrap_or_default();
    }

    fn calculate_twab(&self, oldest_twab: &Twab, newest_twab: &Twab, target_timestamp: u64) -> Twab{

        if oldest_twab.timestamp == target_timestamp{
            return *oldest_twab;
        }

        if newest_twab.timestamp == target_timestamp{
            return *newest_twab;
        }

        if oldest_twab.timestamp > target_timestamp {
            return Twab { amount: 0, timestamp: target_timestamp };
        }

        if newest_twab.timestamp < target_timestamp {
            return Twab { 
                amount: self.compute_twab_balance(
                    newest_twab.amount, 
                    self.balance, 
                    target_timestamp, 
                    newest_twab.timestamp
                ),
                timestamp: target_timestamp, 
            }
        }

        let before_or_at = self.get_twab_before_or_at(target_timestamp);
        let after_or_at = self.get_twab_after_or_at(target_timestamp);

        if before_or_at.timestamp == target_timestamp{
            return before_or_at;
        } 

        if after_or_at.timestamp == target_timestamp{
            return after_or_at;
        }

        let held_balance = (after_or_at.amount - before_or_at.amount) / 
            ((after_or_at.timestamp - before_or_at.timestamp) as u128);

        let mut result = Twab::default();
        result.timestamp = target_timestamp;
        result.amount = self.compute_twab_balance(
            before_or_at.amount, 
            held_balance, 
            target_timestamp, 
            before_or_at.timestamp);

        return result;
    }

    fn get_twab_after_or_at(&self, end_time: u64) -> Twab{
        let mut after_idx = 0;
        for idx in (0..self.twabs.len()).rev(){
            let current_twab = self.twabs.get(idx).unwrap_or_default();

            if current_twab.timestamp >= end_time {
                after_idx = idx;
            } else {
                break;
            }
        }

        return self.twabs.get(after_idx).unwrap_or_default();
    }

    fn generate_twab(&mut self, current_time: u64){
        let last_element_option = self.twabs.pop();

        // If there are no elements in the collection
        // just add an element
        if last_element_option.is_none() {
            let twab_amount = self.compute_twab_balance(0, 0, current_time, 0);
            self.twabs.push(&Twab{timestamp: current_time, amount: twab_amount});      
        } else {
            let last_element = last_element_option.unwrap();

            // If this balance increase is for the same timestamp increase the balance
            // because we are using the block_timestamp, 
            // In real life situations it will be hard to enter this case
            if last_element.timestamp == current_time{
                self.twabs.push(&last_element);
            } else {
                let twab_amount = self.compute_twab_balance(
                    last_element.amount, 
                    self.balance, 
                    current_time, 
                    last_element.timestamp
                );
                self.twabs.push(&last_element);
                self.twabs.push(&Twab{timestamp: current_time, amount: twab_amount});
            }
        }
    }

    fn compute_twab_balance(
        &self, 
        last_twab_amount: Balance, 
        current_balance: Balance, 
        current_time: u64, 
        last_twab_timestamp: u64
    ) -> Balance {
        return last_twab_amount + current_balance * ((current_time - last_twab_timestamp) as u128);
    }

}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct AccountsDepositHistory{
    pub accounts: UnorderedMap<AccountId, AccountBalance>,
    pub total_supply: AccountBalance,
}

impl Default for AccountsDepositHistory{
    fn default() -> Self {
        AccountsDepositHistory { 
            accounts: UnorderedMap::new(StorageKeys::AccountBalance),
            total_supply: AccountBalance { 
                balance: Balance::default(), 
                twabs: Vector::new(StorageKeys::TotalSupplyAccountBalance) 
            },
        }
    }
}

impl AccountsDepositHistory {
    fn get_account(&self, account_id: &AccountId) -> AccountBalance{
        return self.accounts.get(&account_id).unwrap_or_else(|| {
            AccountBalance { 
                balance: Balance::default(), 
                twabs: Vector::new(
                    StorageKeys::SubAccountBalance { 
                        account_hash: utils::utils::get_hash(&account_id) 
                    }) 
            }
        });
    }
}

impl ITwab for AccountsDepositHistory{
    fn increase_balance(&mut self, account: &AccountId, amount: Balance, current_time: u64) {
        let mut acc_balance = self.get_account(&account);
        acc_balance.generate_twab(current_time);

        acc_balance.balance += amount;
        self.accounts.insert(&account, &acc_balance);
    }

    fn decrease_balance(&mut self, account: &AccountId, amount: Balance, current_time: u64) {
        let mut acc_balance = self.get_account(&account);
        acc_balance.generate_twab(current_time);
        
        acc_balance.balance -= amount;
        self.accounts.insert(&account, &acc_balance);
    }

    fn increase_total_supply(&mut self, amount: Balance, current_time: u64) {
        self.total_supply.generate_twab(current_time);
        self.total_supply.balance += amount;
    }

    fn decrease_total_supply(&mut self, amount: Balance, current_time: u64) {
        self.total_supply.generate_twab(current_time);
        self.total_supply.balance -= amount;
    }

    fn average_total_supply_between_timestamps(
        &self,
        start_time: u64,
        end_time: u64
    ) -> Balance{
        assert!(start_time < end_time);
        let oldest_twab = self.total_supply.twabs.get(0).unwrap_or_default();
        let newest_twab = self.total_supply.twabs.get(self.total_supply.twabs.len() - 1).unwrap_or_default();

        let start_twab: Twab = self.total_supply.calculate_twab(&oldest_twab, &newest_twab, start_time);
        let end_twab: Twab = self.total_supply.calculate_twab(&oldest_twab, &newest_twab, end_time);
            
        return (end_twab.amount - start_twab.amount)/((end_twab.timestamp - start_twab.timestamp) as u128);
    }

    fn average_balance_between_timestamps(
            &self, 
            account: &AccountId, 
            start_time: u64, 
            end_time: u64
        ) -> Balance {
        assert!(start_time < end_time);
        let account_balance = self.accounts.get(&account).unwrap_or_default();
        let oldest_twab = account_balance.twabs.get(0).unwrap_or_default();
        let newest_twab = account_balance.twabs.get(account_balance.twabs.len() - 1).unwrap_or_default();

        let start_twab: Twab = account_balance.calculate_twab(&oldest_twab, &newest_twab, start_time);
        let end_twab: Twab = account_balance.calculate_twab(&oldest_twab, &newest_twab, end_time);
            
        return (end_twab.amount - start_twab.amount)/((end_twab.timestamp - start_twab.timestamp) as u128);
    }
}