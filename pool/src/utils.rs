use crate::Contract;
use crate::env;

pub mod storage_keys{
    use near_sdk::{BorshStorageKey, CryptoHash};
    use near_sdk::borsh::{self, BorshSerialize};

    #[derive(BorshStorageKey, BorshSerialize)]
    pub enum StorageKeys {
        AccountsDepositHistory,
        AccountBalance,
        SubAccountBalance {account_hash: CryptoHash},
        TotalSupplyAccountBalance,
        AccountPicks,
        AccountDrawPicks {account_hash: CryptoHash},
        Token,
        TokenMetadata,
        UserNearDeposit,
        AccountClaimedPicks{account_hash: CryptoHash},
        PauserUser,
    }
}

pub mod utils{
    use common::types::{NumPicks, WinningNumber};
    use near_sdk::{AccountId, CryptoHash};
    use near_sdk::env::{self, keccak256_array};

    pub(crate) fn get_hash(account_id: &AccountId) -> CryptoHash {
        env::sha256_array(account_id.as_bytes())
    }

    pub(crate) fn get_user_winning_number(account_id: &AccountId, pick: NumPicks) -> WinningNumber{
        let acc_keccak = env::keccak256_array(account_id.as_bytes());
        return WinningNumber::from_little_endian(&concat_pick_and_hash(&acc_keccak, pick));
    }

    pub(crate) fn concat_pick_and_hash(bytes: &[u8], pick: NumPicks) -> [u8; 32]{
        let arr=[bytes, &pick.to_be_bytes()].concat();
        return keccak256_array(&arr);
    } 
}

impl Contract{
    pub(crate) fn assert_owner(&self){
        assert_eq!(self.owner_id, env::signer_account_id());
    }

    pub(crate) fn assert_pauser_user(&self){
        assert!(self.pauser_users.contains(&env::signer_account_id()));
    }
}

pub mod gas{
    use near_sdk::{Gas};

    pub const ON_GET_DRAW: Gas = Gas(15_000_000_000_000);
    pub const GET_DRAW: Gas = Gas(1_000_000_000_000);
    pub const GET_BALANCE_FROM_DEFI: Gas = Gas(20_000_000_000_000);
    pub const CLAIM_REWARDS_CALLBACK: Gas = Gas(50_000_000_000_000);
    pub const CLAIM_REWARDS_EXTERNAL_DEFI: Gas = Gas(50_000_000_000_000);
    pub const WITHDRAW_TOKENS_FROM_DEFI_CALLBACK: Gas = Gas(50_000_000_000_000);
    pub const WITHDRAW_TOKENS_EXTERNAL_DEFI: Gas = Gas(50_000_000_000_000);
    
    pub const GAS_FOR_FT_TRANSFER: Gas = Gas(Gas::ONE_TERA.0 * 10);
    pub const GAS_FOR_TRANSFER_TO_DEFI:Gas = Gas(Gas::ONE_TERA.0 * 100);
}