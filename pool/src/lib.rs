use burrow::BurrowYieldSource;
use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC,
};
use near_contract_standards::fungible_token::FungibleToken;
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LazyOption;
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, log, near_bindgen, AccountId, Balance, PanicOnDefault, PromiseOrValue, assert_one_yocto, ext_contract, PromiseError};
use interfaces::pool::{IPool, ITwab};
use interfaces::defi::IYieldSource;
use picks::AccountsPicks;
use twab::AccountsDepositHistory;
use prize::PrizeBuffer;
use common::types::{DrawId, NumPicks, WinningNumber};
use interfaces::defi::YieldSource;
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
const TOTAL_SUPPLY: u128 = 1_000;

#[derive(Deserialize, Debug, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct AssetAmount {
    pub token_id: AccountId,
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

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    token: FungibleToken,
    metadata: LazyOption<FungibleTokenMetadata>,
    // the token that is going to be used
    deposited_token_id: AccountId,
    tickets: AccountsDepositHistory,
    prizes: PrizeBuffer,
    draw_contract: AccountId,
    acc_picks: AccountsPicks,
    yield_source: YieldSource,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct TokenAmountsView{
    token: near_sdk::AccountId,
    shares: U128,
    rewards: U128
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

    fn send_to_dex(&self) {
        todo!()
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
}

#[near_bindgen]
impl Contract {
    /// Initializes the contract with the given total supply owned by the given `owner_id` with
    /// default metadata (for example purposes only).
    #[init]
    pub fn new_default_meta(owner_id: AccountId, token_for_deposit: AccountId, draw_contract: AccountId, burrow_address: AccountId) -> Self {
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
        )
    }

    /// Initializes the contract with the given total supply owned by the given `owner_id` with
    /// the given fungible token metadata.
    /// Dont provide initial supply
    /// The contract should not have initial supply at the beginning
    /// With every deposit of tokens to the contract, tokens will be minted
    #[init]
    fn new(
        owner_id: AccountId,
        deposited_token_id: AccountId,
        metadata: FungibleTokenMetadata,
        draw_contract: AccountId,
        burrow_address: AccountId,
    ) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        metadata.assert_valid();
        let mut this = Self {
            token: FungibleToken::new(b"a".to_vec()),
            metadata: LazyOption::new(b"m".to_vec(), Some(&metadata)),
            deposited_token_id: deposited_token_id,
            tickets: AccountsDepositHistory::default(),
            prizes: PrizeBuffer::new(),
            draw_contract: draw_contract,
            acc_picks: AccountsPicks::default(),
            yield_source: YieldSource::Burrow { address: burrow_address },
        };

        this.token.internal_register_account(&owner_id);
        
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
        self.assert_correct_token_is_send_to_contract(&env::predecessor_account_id());
        self
            .get_yield_source()
            .transfer(&env::predecessor_account_id(), amount.0);

        self.mint_tokens(sender_id, amount.0);

        return PromiseOrValue::Value(U128(0));
    }
}

#[cfg(test)]
mod tests {
    use near_sdk::{collections::Vector, AccountId, Balance, env};
    use crate::{twab::AccountsDepositHistory, twab::AccountBalance, interfaces::{pool::ITwab, prize_distribution::PrizeDistribution}};
    use common::{generic_ring_buffer::GenericRingBuffer, types::U256};

    fn mint(tickets: &mut AccountsDepositHistory, acc_id: &AccountId, amount: Balance, time: u64){
        tickets.increase_balance(acc_id, amount, time);
        tickets.increase_total_supply(amount, time);
    }

    fn burn(tickets: &mut AccountsDepositHistory, acc_id: &AccountId, amount: Balance, time: u64){
        tickets.decrease_balance(acc_id, amount, time);
        tickets.decrease_total_supply(amount, time);
    }

    fn mmmm() -> AccountId{
        AccountId::new_unchecked("mmmm".to_string())
    }

    fn sec() -> AccountId{
        AccountId::new_unchecked("sec".to_string())
    }

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
        let mut buffer = GenericRingBuffer::<PrizeDistribution, 5>::new();
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

    }
}
