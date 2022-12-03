use burrow::BurrowYieldSource;
use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC,
};
use near_contract_standards::fungible_token::FungibleToken;
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LazyOption, LookupMap, UnorderedSet};
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, log, near_bindgen, AccountId, Balance, Promise, PanicOnDefault, PromiseOrValue, ext_contract, PromiseError};
use interfaces::pool::{IPool, ITwab};
use interfaces::defi::IYieldSource;
use picks::AccountsPicks;
use twab::AccountsDepositHistory;
use prize::{PrizeBuffer};
use common::types::{DrawId, NumPicks, WinningNumber};
use interfaces::defi::{YieldSource, YieldSourceAction};
use utils::gas;

const ZERO_ADDRESS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

pub mod external;
pub use crate::external::*;

mod ft_token;
mod interfaces;
mod twab;
mod prize;
mod picks;
mod utils;
mod test_utils;
mod burrow;

const PROTOCOL_FT_SYMBOL: &str = "PTTICK";
const PROTOCOL_FT_NAME: &str = "Pool Together Ticket";

#[derive(Deserialize, Debug, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct AssetAmount {
    pub token_id: AccountId,
    /// The amount of tokens intended to be used for the action
    /// If 'None', then the maximum will be tried
    pub amount: Option<U128>,
}

#[derive(Deserialize, Debug, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub enum Action{
    Withdraw(AssetAmount)
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    token: FungibleToken,
    metadata: LazyOption<FungibleTokenMetadata>,
    // the token that is going to be used
    deposited_token_id: AccountId,
    // token that rewards will be payed into
    reward_token_id: AccountId,
    tickets: AccountsDepositHistory,
    prizes: PrizeBuffer,
    draw_contract: AccountId,
    acc_picks: AccountsPicks,
    yield_source: YieldSource,
    user_near_deposit: LookupMap<AccountId, Balance>,
    owner_id: AccountId,
    min_pick_cost: Balance,
    paused: bool,
    pauser_users: UnorderedSet<AccountId>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct TokenAmountsView{
    token: near_sdk::AccountId,
    shares: U128,
    rewards: U128
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct AssetView {
    pub token_id: AccountId,
    pub balance: Balance
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

const DATA_IMAGE_SVG_NEAR_ICON: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 288 288'%3E%3Cg id='l' data-name='l'%3E%3Cpath d='M187.58,79.81l-30.1,44.69a3.2,3.2,0,0,0,4.75,4.2L191.86,103a1.2,1.2,0,0,1,2,.91v80.46a1.2,1.2,0,0,1-2.12.77L102.18,77.93A15.35,15.35,0,0,0,90.47,72.5H87.34A15.34,15.34,0,0,0,72,87.84V201.16A15.34,15.34,0,0,0,87.34,216.5h0a15.35,15.35,0,0,0,13.08-7.31l30.1-44.69a3.2,3.2,0,0,0-4.75-4.2L96.14,186a1.2,1.2,0,0,1-2-.91V104.61a1.2,1.2,0,0,1,2.12-.77l89.55,107.23a15.35,15.35,0,0,0,11.71,5.43h3.13A15.34,15.34,0,0,0,216,201.16V87.84A15.34,15.34,0,0,0,200.66,72.5h0A15.35,15.35,0,0,0,187.58,79.81Z'/%3E%3C/g%3E%3C/svg%3E";

#[derive(Deserialize, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct Draw {
    pub draw_id: DrawId,
    pub winning_random_number: WinningNumber,
    pub started_at: u64,
    pub completed_at: u64,
}

impl IPool for Contract{
    fn assert_correct_token_is_send_to_contract(&self, token: &AccountId) {
        assert_eq!(token, &self.get_lottery_asset());
    }

    fn get_lottery_asset(&self) -> AccountId {
        self.deposited_token_id.clone()
    }

    fn get_yield_source(&self) -> Box<dyn IYieldSource> {
        return match &self.yield_source{
            YieldSource::Burrow { address } =>
                Box::new(BurrowYieldSource{address: address.clone()}),
            YieldSource::Metapool { address } =>
                panic!("Not implemented for Metapool"),
            _ => panic!("No default option"),
        };
    }

    fn assert_sender_has_deposited_enough(&self, account: &AccountId, deposit: Balance) {
        assert!(self.user_near_deposit.get(account).unwrap_or_default() >= deposit);
    }
}

#[near_bindgen]
impl Contract {
    /// Initializes the contract with the given total supply owned by the given `owner_id` with
    /// default metadata (for example purposes only).
    #[init]
    pub fn new_default_meta(owner_id: AccountId, token_for_deposit: AccountId, draw_contract: AccountId, burrow_address: AccountId, reward_token: AccountId, min_pick_cost: U128) -> Self {
        Self::new(
            owner_id,
            token_for_deposit,
            FungibleTokenMetadata {
                spec: FT_METADATA_SPEC.to_string(),
                name: PROTOCOL_FT_NAME.to_string(),
                symbol: PROTOCOL_FT_SYMBOL.to_string(),
                icon: Some(DATA_IMAGE_SVG_NEAR_ICON.to_string()),
                reference: None,
                reference_hash: None,
                decimals: 3,
            },
            draw_contract,
            burrow_address,
            reward_token,
            min_pick_cost,
        )
    }

    /// Initializes the contract with the given total supply owned by the given `owner_id` with
    /// the given fungible token metadata.
    /// Dont provide initial supply
    /// The contract should not have initial supply at the beginning
    /// With every deposit of tokens to the contract, tokens will be minted
    #[init]
    pub fn new(
        owner_id: AccountId,
        deposited_token_id: AccountId,
        metadata: FungibleTokenMetadata,
        draw_contract: AccountId,
        burrow_address: AccountId,
        reward_token: AccountId,
        min_pick_cost: U128,
    ) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        metadata.assert_valid();
        assert!(
            env::is_valid_account_id(owner_id.as_bytes()),
            "The owner ID is invalid"
        );
        assert!(
            env::is_valid_account_id(draw_contract.as_bytes()),
            "The draw ID is invalid"
        );
        assert!(
            env::is_valid_account_id(burrow_address.as_bytes()),
            "Burrow contract ID is invalid"
        );
        assert!(
            env::is_valid_account_id(deposited_token_id.as_bytes()),
            "The token ID is invalid"
        );
        let mut this = Self {
            token: FungibleToken::new(utils::storage_keys::StorageKeys::Token),
            metadata: LazyOption::new(utils::storage_keys::StorageKeys::TokenMetadata, Some(&metadata)),
            deposited_token_id: deposited_token_id,
            reward_token_id: reward_token,
            tickets: AccountsDepositHistory::default(),
            prizes: PrizeBuffer::new(),
            draw_contract: draw_contract,
            acc_picks: AccountsPicks::default(),
            yield_source: YieldSource::Burrow { address: burrow_address },
            user_near_deposit: LookupMap::new(utils::storage_keys::StorageKeys::UserNearDeposit),
            owner_id: owner_id.clone(),
            min_pick_cost: min_pick_cost.0,
            paused: false,
            pauser_users: UnorderedSet::new(utils::storage_keys::StorageKeys::PauserUser),
        };

        this.token.internal_register_account(&owner_id);
        this.token.internal_register_account(&AccountId::new_unchecked(ZERO_ADDRESS.to_string()));
        this.pauser_users.insert(&owner_id);
        
        near_contract_standards::fungible_token::events::FtMint {
            owner_id: &owner_id,
            amount: &U128(0),
            memo: Some("Initial tokens supply is minted"),
        }
        .emit();

        this
    }

    pub fn get_asset(&self) -> AccountId {
        self.deposited_token_id.clone()
    }

    #[payable]
    pub fn accept_deposit_for_future_fungible_token_transfers(&mut self) {
        let mut user_balance = self.user_near_deposit.get(&env::predecessor_account_id()).unwrap_or_default();
        user_balance += env::attached_deposit();
        self.user_near_deposit.insert(&env::predecessor_account_id(), &user_balance);
    }

    fn decrement_user_near_deposit(&mut self, account: &AccountId, amount: Option<Balance>){
        let mut user_balance = self.user_near_deposit.get(account).unwrap_or_default();
        user_balance = if amount.is_none(){
            user_balance - 1
        }else{
            user_balance - amount.unwrap()
        };
        
        self.user_near_deposit.insert(account, &(user_balance));
    }

    pub fn get_reward(&self) -> Promise{
        let ys = self.get_yield_source();
        let (_, gas) = ys.get_action_required_deposit_and_gas(YieldSourceAction::GetReward);
        assert!(env::prepaid_gas() >= gas);

        return ys.get_reward();
    }

    pub fn get_deposited_amount(&self) -> U128{
        return self.token.total_supply.into();
    }

    #[payable]
    pub fn withdraw(&mut self, ft_tokens_amount: U128){
        if self.paused{
            return;
        }

        let caller = env::signer_account_id();
        let acc_supplied_balance = self.token.ft_balance_of(caller.clone());
        assert!(ft_tokens_amount.0 <= acc_supplied_balance.0);
        
        let ys = self.get_yield_source();
        let (deposit, gas) = ys.get_action_required_deposit_and_gas(YieldSourceAction::Withdraw);

        if env::attached_deposit() < deposit{
            self.assert_sender_has_deposited_enough(&caller, deposit);
            self.decrement_user_near_deposit(&caller, Some(deposit));
        }
        assert!(env::prepaid_gas() >= gas);
        
        ys.withdraw(&caller, &self.deposited_token_id, ft_tokens_amount.0);
    }

    /// Add authorized user to pause/unpause the contract
    pub fn add_pauser_user(&mut self, account_id: AccountId) {
        self.assert_owner();
        self.pauser_users.insert(&account_id);
    }

    /// Remove authorized user to pause/unpause the contract
    pub fn remove_pauser_user(&mut self, account_id: AccountId) {
        self.assert_owner();
        self.pauser_users.remove(&account_id);
    }

    pub fn pause(&mut self){
        self.assert_pauser_user();
        assert!(!self.paused, "Contract is already paused");

        self.paused = true;
    }

    pub fn resume(&mut self){
        self.assert_pauser_user();
        assert!(self.paused, "Contract is already resumed");

        self.paused = false;
    }

}

near_contract_standards::impl_fungible_token_core!(Contract, token, on_tokens_burned);
near_contract_standards::impl_fungible_token_storage!(Contract, token, on_account_closed);

#[near_bindgen]
impl FungibleTokenMetadataProvider for Contract {
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        self.metadata.get().unwrap()
    }
}

#[near_bindgen]
impl FungibleTokenReceiver for Contract{
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        if self.paused{
            return PromiseOrValue::Value(amount);
        }

        log!("{} ft_on_transfer", env::current_account_id());
        self.assert_correct_token_is_send_to_contract(&env::predecessor_account_id());
        let (deposit, gas) = self.get_yield_source().get_action_required_deposit_and_gas(YieldSourceAction::Transfer);
        self.assert_sender_has_deposited_enough(&sender_id, deposit);
        assert!(env::prepaid_gas() >= gas);

        self.decrement_user_near_deposit(&sender_id, Some(deposit));
        
        self
            .get_yield_source()
            .transfer(&env::predecessor_account_id(), amount.0);

        self.mint_tokens(sender_id, amount.0);

        return PromiseOrValue::Value(U128(0));
    }
}

#[cfg(test)]
mod tests {
    use near_sdk::{collections::{Vector, LookupSet}, AccountId, Balance, env};
    use crate::{twab::AccountsDepositHistory, twab::AccountBalance, interfaces::{pool::ITwab, prize_distribution::PrizeDistribution}, test_utils::{mmmm, sec, mint, burn}};
    use common::{generic_ring_buffer::GenericRingBuffer, types::{U256, DrawId}};

    fn setup() -> AccountsDepositHistory{
        let mut result = AccountsDepositHistory::default();
        let acc_id = mmmm();
        let sec_id = sec();

        mint(&mut result, &acc_id, 100, 0);
        mint(&mut result, &sec_id, 30, 5);
        mint(&mut result, &acc_id, 50, 10);
        burn(&mut result, &acc_id, 100, 20);
        mint(&mut result, &sec_id, 80, 26);
        burn(&mut result, &acc_id, 20, 30);
        mint(&mut result, &acc_id, 10, 40);
        burn(&mut result, &sec_id, 50, 50);

        return result;
    }

    #[test]
    fn test_generic_buffer(){
        let mut buffer = GenericRingBuffer::<PrizeDistribution, DrawId, 5>::new();
        buffer.arr[0] =  PrizeDistribution::default();
        buffer.arr[0].tiers[1] = 20;
        println!("xx");
    }

    #[test]
    fn it_works() {
        let num:u32 = 13;

        let x:u8 = 1;
        println!("{:08b}", x);

        let aa = u64::from_le_bytes([0,1,0,0,0,0,0,0]);
        
        let one = U256::from_little_endian(&[1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]);

        let a = U256([1u64, 0, 0, 0]);
        let b = U256::one();

        assert_eq!(b, one);

        let bytes_arr = [mmmm().as_bytes(), &num.to_le_bytes()].concat();
        let hash = env::keccak256_array(&bytes_arr);
        
        let mut nums = vec![1,2,3,4];
        let mut v:Vector<u32> = Vector::new(b"x".to_vec());
        let a = v.pop();
        v.push(&1);
        v.push(&2);
        let x = v.pop();
        println!("{}", nums[3]);
        let result = 2 + 2;
        assert_eq!(result, 4);
    }

    #[test]
    fn verify_setup(){
        let setup = setup();
        assert_eq!(setup.total_supply.balance, 100);
        assert_eq!(setup.total_supply.twabs.get(0).unwrap_or_default().amount, 0);
        assert_eq!(setup.total_supply.twabs.get(1).unwrap_or_default().amount, 500);
        assert_eq!(setup.total_supply.twabs.get(2).unwrap_or_default().amount, 1150);
        assert_eq!(setup.total_supply.twabs.get(3).unwrap_or_default().amount, 2950);
        assert_eq!(setup.total_supply.twabs.get(4).unwrap_or_default().amount, 3430);

        let acc_sec : AccountBalance = setup.accounts.get(&sec()).unwrap_or_default();
        let acc_mmm : AccountBalance = setup.accounts.get(&mmmm()).unwrap_or_default();

        assert_eq!(acc_mmm.balance, 40);
        assert_eq!(acc_mmm.twabs.get(0).unwrap_or_default().amount, 0);
        assert_eq!(acc_mmm.twabs.get(1).unwrap_or_default().amount, 1000);
        assert_eq!(acc_mmm.twabs.get(2).unwrap_or_default().amount, 2500);
        assert_eq!(acc_mmm.twabs.get(3).unwrap_or_default().amount, 3000);

        assert_eq!(acc_sec.balance, 60);
        assert_eq!(acc_sec.twabs.get(0).unwrap_or_default().amount, 0);
        assert_eq!(acc_sec.twabs.get(1).unwrap_or_default().amount, 630);
        assert_eq!(acc_sec.twabs.get(2).unwrap_or_default().amount, 3270);
    }

    #[test]
    fn check_twabs(){
        let setup = setup();
        
        // test existing timestamps
        assert_eq!(setup.average_balance_between_timestamps(&mmmm(), 0, 20), 125);
        // test timestamps that are in range but doesnt exist
        assert_eq!(setup.average_balance_between_timestamps(&mmmm(), 5, 15), 125);
        assert_eq!(setup.average_balance_between_timestamps(&mmmm(), 5, 10), 100);
        assert_eq!(setup.average_balance_between_timestamps(&mmmm(), 3, 13), 115);
        assert_eq!(setup.average_balance_between_timestamps(&mmmm(), 4, 28), 104);
        // test timestamps that are not in existing range
        assert_eq!(setup.average_balance_between_timestamps(&mmmm(), 4, 45), 75);

        assert_eq!(setup.average_balance_between_timestamps(&sec(), 0, 60), 64);
    }

    #[test]
    fn check_total_supply_twabs(){
        let setup = setup();
        let bal = setup.average_balance_between_timestamps(&mmmm(), 10000, 20000);
        println!("{}", bal);
    }

    #[test]
    fn test_lookup(){
        let mut c = LookupSet::<i32>::new(b"a");
        c.insert(&1);
        c.insert(&2);
        c.insert(&3);
        c.remove(&2);
        assert_eq!(c.contains(&2), false);
        assert_eq!(c.contains(&3), true);
    }
}
