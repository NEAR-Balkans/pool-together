use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Serialize, Deserialize};
use uint::construct_uint;

pub type DrawId = u32;
pub type NumPicks = u128;

construct_uint!{
    /// 256-bit unsigned integer
    #[derive(Serialize, Deserialize, BorshDeserialize, BorshSerialize)]
    pub struct U256(4);
}

pub type WinningNumber = U256;