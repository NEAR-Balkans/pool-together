use crate::*;
use near_sdk::{borsh::{self, BorshDeserialize, BorshSerialize}};
use crate::interfaces::prize_distribution::{PrizeDistribution, PrizeDistributionActor};
use common::{generic_ring_buffer::{GenericRingBuffer, RingBuffer}, types::{WinningNumber, U256}};

const MAX_PRIZES_CAPACITY: usize = 32;
const MIN_PICK_COST: Balance = 1;
const BIT_RANGE_SIZE: u8 = 1;
const TIERS: [u32; 16]= [20,30,20,10,5,5,10,0,0,0,0,0,0,0,0,0];
const TIERS_NOMINAL:u128 = 100;
const PRIZE_DISTRIBUTION_TIME_OFFSET: u64 = 1000 * 3600 * 24 * 7;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct PrizeBuffer{
    pub buffer: GenericRingBuffer<PrizeDistribution, MAX_PRIZES_CAPACITY>,
}

impl PrizeBuffer{
    pub fn new() -> Self{
        return Self { buffer: GenericRingBuffer::<PrizeDistribution, MAX_PRIZES_CAPACITY>::new() };
    }
}

#[near_bindgen]
impl Contract{
    fn create_masks(&self, bit_range_size: u8, cardinality: u8) -> Vec<WinningNumber>{
        let mut result:Vec<WinningNumber> = Vec::new();

        result.push(WinningNumber::from(2).pow(bit_range_size.into()) - 1);

        for _idx in 1..cardinality{
            result.push(result.last().unwrap() << bit_range_size);
        }

        return result;
    }

    fn get_tier_match(&self, masks: &Vec<WinningNumber>, user_winning_number: &WinningNumber, winning_number: &WinningNumber) -> u8{
        return self.get_tier_match_generic(masks, user_winning_number, winning_number);
    }

    fn get_tier_match_generic<T>(
        &self, 
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

    fn number_of_prizes_for_tier(&self, tier: u8, bit_range_size: u8) -> u64{
        if tier == 0{
            return 1;
        }else{
            return ( 1 << (bit_range_size * tier) ) - ( 1 << (bit_range_size * (tier - 1)) );
        }
    }

    fn prize_tier_fraction(&self, tier_idx: u8, bit_range_size: u8, prize_tiers: &[u32]) -> u64{
        let prize_fraction = prize_tiers[(tier_idx as usize)];
        
        let number_of_prizes_for_tier = self.number_of_prizes_for_tier(tier_idx, bit_range_size);

        return (prize_fraction as u64) / number_of_prizes_for_tier;
    }

    #[private]
    pub fn on_get_draw_and_add_prize_distribution(&mut self, prize_awards: Balance, #[callback_result] call_result: Result<Draw, PromiseError>) {
        if call_result.is_err(){
            log!("Error when getting draw");
        }

        let draw = call_result.unwrap();
        let mut cardinality:u8 = 0;
        let tickets_supply = self.tickets.average_total_supply_between_timestamps(draw.started_at, draw.completed_at);
        let max_picks = tickets_supply / MIN_PICK_COST;
        let bit_range_sized_two = 2u8.pow(BIT_RANGE_SIZE.into());
        while u128::from(bit_range_sized_two.pow(cardinality.into())) < max_picks {
            cardinality += 1;
        }

        let number_of_picks: u64 = bit_range_sized_two.pow(cardinality.into()).into();
        let prize_distribution = PrizeDistribution {
            number_of_picks: number_of_picks, 
            draw_id: draw.draw_id,
            cardinality: cardinality,
            bit_range_size: BIT_RANGE_SIZE,
            tiers: TIERS,
            max_picks: max_picks,
            prize: prize_awards,
            start_time: draw.completed_at + PRIZE_DISTRIBUTION_TIME_OFFSET,
            end_time: draw.completed_at + 2 * PRIZE_DISTRIBUTION_TIME_OFFSET,
            winning_number: draw.winning_random_number,
        };

        self.prizes.buffer.add(&prize_distribution);
    }
}

#[near_bindgen]
impl PrizeDistributionActor for Contract{
    fn get_prize_distribution(&self, draw_id: DrawId) -> PrizeDistribution {
        for idx in 0..self.prizes.buffer.arr.len(){
            if self.prizes.buffer.arr[idx].draw_id == draw_id{
                return self.prizes.buffer.arr[idx];
            }
        }

        return PrizeDistribution::default();
    }

    fn add_prize_distribution(&mut self, draw_id: DrawId, prize_awards: Balance) {
        if self.get_prize_distribution(draw_id) != PrizeDistribution::default(){
            return;
        }
        let draw_promise = ext_draw::get_draw(draw_id, self.draw_contract.clone(), 0, gas::GET_DRAW);
        draw_promise.then(
            this_contract::on_get_draw_and_add_prize_distribution(prize_awards, env::current_account_id(), 0, gas::GET_DRAW)
        );
    }

    #[payable]
    fn claim(&mut self, draw_id: U128, pick: U128) -> u128{
        assert_one_yocto();
        
        let prize_distribution = self.get_prize_distribution(draw_id.0);
        let caller = env::signer_account_id();
        let picks_for_draw = self.acc_picks.get_picks_for_draw(&caller, &draw_id.0);
        
        if picks_for_draw == NumPicks::default(){
            panic!("There are no generated picks for this draw for client");
        }

        if pick.0 >= picks_for_draw {
            panic!("Invalid pick");
        }

        let user_winning_number = utils::utils::get_user_winning_number(&caller, pick.0);
        let masks = self.create_masks(prize_distribution.bit_range_size, prize_distribution.cardinality);
        // get tier match
        let tier_match = self.get_tier_match(&masks, &user_winning_number, &prize_distribution.winning_number);
        // get prize tier fraction
        let prize_tier_fraction = self.prize_tier_fraction(tier_match, prize_distribution.bit_range_size, &prize_distribution.tiers);
        let prize_to_take = u128::from(prize_tier_fraction) * prize_distribution.prize / TIERS_NOMINAL;

        log!("Prize to claim is {} {}", prize_to_take, self.deposited_token_id);
        self.get_yield_source().claim(&caller, &self.deposited_token_id, prize_to_take);
        
        return prize_to_take;
    }
}

#[cfg(test)]
mod tests{
    use crate::*;
    use crate::test_utils::{get_contract};
    use common::types::U256;

    use super::TIERS;

    #[test]
    fn test_masks(){
        let contract = get_contract();

        let cardinality = 8;
        let mut masks = contract.create_masks(1, 8);

        assert_eq!(masks.len(), cardinality as usize);
        assert_eq!(masks[0], U256::from(1));
        assert_eq!(masks[1], U256::from(2));
        assert_eq!(masks[2], U256::from(4));
        assert_eq!(masks[7], U256::from(128));

        masks = contract.create_masks(4, cardinality);
        assert_eq!(masks.len(), cardinality as usize);
        assert_eq!(masks[0], U256::from(15));
        assert_eq!(masks[1], U256::from(240));
    }

    #[test]
    fn test_tier_match(){
        let contract = get_contract();

        let masks= contract
            .create_masks(1, 8)
            .iter()
            .map(|x| x.as_u32())
            .collect::<Vec<u32>>();

        let winning_number = 153u32;
        let mut guess_number = 25u32;
        let tier_match = contract.get_tier_match_generic(&masks, &guess_number, &winning_number);
        assert_eq!(tier_match, 1);

        guess_number = 153;
        let tier_match = contract.get_tier_match_generic(&masks, &guess_number, &winning_number);
        assert_eq!(tier_match, 0);

        guess_number = 152;
        let tier_match = contract.get_tier_match_generic(&masks, &guess_number, &winning_number);
        assert_eq!(tier_match, 8);

        guess_number = 25;
        let tier_match = contract.get_tier_match(&masks.iter().map(|x| U256::from(*x)).collect(), &U256::from(guess_number), &U256::from(winning_number));
        assert_eq!(U256::from(tier_match), U256::one());

    }

    #[test]
    fn test_number_of_prizes(){
        let contract = get_contract();
        let prizes_number = contract.number_of_prizes_for_tier(0, 8);
        assert_eq!(prizes_number, 1);
        let prizes_number = contract.number_of_prizes_for_tier(0, 10);
        assert_eq!(prizes_number, 1);
        let prizes_number = contract.number_of_prizes_for_tier(1, 4);
        assert_eq!(prizes_number, 15);
        let prizes_number = contract.number_of_prizes_for_tier(1, 1);
        assert_eq!(prizes_number, 1);
    }

    #[test]
    fn test_prize_tier_fraction(){
        let contract = get_contract();

        let prize_fraction = contract.prize_tier_fraction(0, 4, &TIERS);
        assert_eq!(prize_fraction, TIERS[0] as u64);

        let prize_fraction = contract.prize_tier_fraction(1, 1, &TIERS);
        assert_eq!(prize_fraction, TIERS[1] as u64);

        let prize_fraction = contract.prize_tier_fraction(1, 4, &TIERS);
        assert_eq!(prize_fraction, 2);
        
        let prize_fraction = contract.prize_tier_fraction(1, 2, &TIERS);
        assert_eq!(prize_fraction, 10);

        let prize_fraction = contract.prize_tier_fraction(2, 2, &TIERS);
        assert_eq!(prize_fraction, 1);
    }
}