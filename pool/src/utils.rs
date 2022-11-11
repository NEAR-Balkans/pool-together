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

pub mod gas{
    use near_sdk::{Gas, Balance};

    pub const GET_DRAW: Gas = Gas(20_000_000_000_000);
    pub const ONE_YOCTO: Balance = 1;
    pub const GET_BALANCE_FROM_DEFI: Gas = Gas(20_000_000_000_000);
    
    pub const GAS_FOR_FT_TRANSFER: Gas = Gas(Gas::ONE_TERA.0 * 10);
    pub const GAS_FOR_FT_TRANSFER_CALL: Gas = Gas(300_000_000_000_000);
    pub const GAS_FOR_AFTER_FT_TRANSFER: Gas = Gas(Gas::ONE_TERA.0 * 20);
    pub const GAS_FOR_TRANSFER_TO_DEFI:Gas = Gas(Gas::ONE_TERA.0 * 100);
    
    pub const MAX_GAS: Gas = Gas(300_000_000_000_000);
}