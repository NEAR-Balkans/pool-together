use common::types::{WinningNumber, U256, NumPicks};
use near_sdk::{Balance, AccountId, env::{self, keccak256_array}};

const TIERS_NOMINAL: u128 = 100;

#[derive(Debug)]
pub (crate) struct Ratio {
    pub numerator: u128,
    pub denominator: u128,
}

impl Ratio {
    pub fn assert_valid(&self) {
        assert!(self.denominator != 0 || self.numerator == self.denominator, "Denominator can be 0, only if numerator is 0");
        assert!(
            self.numerator <= self.denominator,
            "The reward fee must be less or equal to 1"
        );
    }

    pub fn multiply(&self, value: Balance) -> Balance {
        self.assert_valid();
        if self.denominator == 0 || self.numerator == 0 {
            0
        } else {
            (U256::from(self.numerator) * U256::from(value) / U256::from(self.denominator))
                .as_u128()
        }
    }
}

pub(crate) fn to_yocto(value: &str) -> u128 {
    let vals: Vec<_> = value.split('.').collect();
    let part1 = vals[0].parse::<u128>().unwrap() * 10u128.pow(24);
    if vals.len() > 1 {
        let power = vals[1].len() as u32;
        let part2 = vals[1].parse::<u128>().unwrap() * 10u128.pow(24 - power);
        part1 + part2
    } else {
        part1
    }
}   

pub(crate) fn create_masks(bit_range_size: u8, cardinality: u8) -> Vec<WinningNumber>{
    let mut result:Vec<WinningNumber> = Vec::new();

    result.push(WinningNumber::from(2).pow(bit_range_size.into()) - 1);

    for _idx in 1..cardinality{
        result.push(result.last().unwrap() << bit_range_size);
    }

    return result;
}

pub(crate) fn get_tier_match( masks: &Vec<WinningNumber>, user_winning_number: &WinningNumber, winning_number: &WinningNumber) -> u8{
    return get_tier_match_generic(masks, user_winning_number, winning_number);
}

pub(crate) fn get_tier_match_generic<T>(
    masks: &Vec<T>, 
    user_winning_number: &T, 
    winning_number: &T
) -> u8 
where T: std::ops::BitAnd<Output = T> + PartialEq + Copy {
    let mut matched_tiers = 0u8;
    for el in masks.iter(){
        if (*el & *winning_number) == (*el & *user_winning_number){
            matched_tiers += 1;
        }else{
            break;
        }
    }

    return (masks.len() as u8) - matched_tiers;
}

pub(crate) fn number_of_prizes_for_tier( tier: u8, bit_range_size: u8) -> u64{
    if tier == 0{
        return 1;
    }else{
        return ( 1 << (bit_range_size * tier) ) - ( 1 << (bit_range_size * (tier - 1)) );
    }
}

pub(crate) fn prize_tier_fraction(tier_idx: u8, bit_range_size: u8, prize_tiers: &[u32]) -> Ratio{
    let prize_fraction = prize_tiers[(tier_idx as usize)];
    
    let number_of_prizes_for_tier = number_of_prizes_for_tier(tier_idx, bit_range_size);

    return Ratio { numerator: prize_fraction as u128, denominator: number_of_prizes_for_tier as u128 * TIERS_NOMINAL }
}

pub(crate) fn get_user_winning_number(account_id: &AccountId, pick: NumPicks) -> WinningNumber{
    let acc_keccak = env::keccak256_array(account_id.as_bytes());
    return WinningNumber::from_little_endian(&concat_pick_and_hash(&acc_keccak, pick));
}

pub(crate) fn concat_pick_and_hash(bytes: &[u8], pick: NumPicks) -> [u8; 32]{
    let arr=[bytes, &pick.to_be_bytes()].concat();
    return keccak256_array(&arr);
} 