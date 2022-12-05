use crate::{*};
use near_sdk::{borsh::{self, BorshDeserialize, BorshSerialize}};
use crate::interfaces::prize_distribution::{PrizeDistribution, PrizeDistributionActor};
use common::{generic_ring_buffer::{GenericRingBuffer, RingBuffer, Identifier}, types::{WinningNumber, U256}};

const MAX_PRIZES_CAPACITY: usize = 32;
const TIERS: [u32; 16]= [20,30,20,10,5,5,10,0,0,0,0,0,0,0,0,0];
const TIERS_NOMINAL:u128 = 100;
const PRIZE_DISTRIBUTION_TIME_OFFSET: u64 = 1000 * 3600 * 24 * 7;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct PrizeBuffer{
    pub buffer: GenericRingBuffer<PrizeDistribution, DrawId, MAX_PRIZES_CAPACITY>,
}

impl Identifier<DrawId> for PrizeDistribution{
    fn id(&self) -> DrawId {
        self.draw_id
    }
}

impl PrizeBuffer{
    pub fn new() -> Self{
        return Self { buffer: GenericRingBuffer::<PrizeDistribution, DrawId, MAX_PRIZES_CAPACITY>::new() };
    }
}

pub struct Ratio {
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

    fn prize_tier_fraction(&self, tier_idx: u8, bit_range_size: u8, prize_tiers: &[u32]) -> Ratio{
        let prize_fraction = prize_tiers[(tier_idx as usize)];
        
        let number_of_prizes_for_tier = self.number_of_prizes_for_tier(tier_idx, bit_range_size);

        return Ratio { numerator: prize_fraction as u128, denominator: number_of_prizes_for_tier as u128 * TIERS_NOMINAL }
    }

    #[private]
    pub fn on_get_draw_and_add_prize_distribution(&mut self, prize_awards: U128, cardinality: u8, bit_range_size: u8, #[callback_result] call_result: Result<Draw, PromiseError>) {
        if call_result.is_err(){
            log!("Error when getting draw");
        }

        let draw = call_result.unwrap();
        
        let tickets_supply = self.tickets.average_total_supply_between_timestamps(draw.started_at, draw.completed_at);
        let max_picks = tickets_supply / self.min_pick_cost;
        let prize_distribution = PrizeDistribution {
            draw_id: draw.draw_id,
            cardinality: cardinality,
            bit_range_size: bit_range_size,
            tiers: TIERS,
            max_picks: max_picks,
            prize: prize_awards.0,
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
        return self.prizes.buffer.get_by_identifier(draw_id);
    }

    fn add_prize_distribution(&mut self, draw_id: DrawId, prize_awards: U128, cardinality: u8, bit_range_size: u8) {
        if self.paused{
            return;
        }

        self.assert_owner();
        assert!((cardinality * bit_range_size) as u16 <= 256 && (cardinality * bit_range_size) as u16 > 0);
        assert!(usize::from(cardinality) <= TIERS.len());
        assert!(prize_awards.0 > 0);

        if self.get_prize_distribution(draw_id) != PrizeDistribution::default(){
            return;
        }
        let draw_promise = ext_draw::get_draw(draw_id, self.draw_contract.clone(), 0, gas::GET_DRAW);
        draw_promise.then(
            this_contract::on_get_draw_and_add_prize_distribution(prize_awards, cardinality, bit_range_size, env::current_account_id(), 0, env::prepaid_gas() - env::used_gas() - near_sdk::Gas(100000000000000))
        );
    }

    #[payable]
    fn claim(&mut self, draw_id: DrawId, pick: U128) -> U128{
        if self.paused{
            return U128(0);
        }

        let prize_distribution = self.get_prize_distribution(draw_id);
        let caller = env::signer_account_id();
        // check if everything is okay with the pick and add it as claimed pick
        self.acc_picks.check_and_add_picks_for_draw(&caller, &draw_id, pick.0);

        let (deposit, gas) = self.get_yield_source().get_action_required_deposit_and_gas(YieldSourceAction::Claim);
        if env::attached_deposit() < deposit{
            self.assert_sender_has_deposited_enough(&caller, deposit);
            self.decrement_user_near_deposit(&caller, Some(deposit));
        }
        assert!(env::prepaid_gas() >= gas);

        let user_winning_number = utils::utils::get_user_winning_number(&caller, pick.0);
        let masks = self.create_masks(prize_distribution.bit_range_size, prize_distribution.cardinality);
        // get tier match
        let tier_match = self.get_tier_match(&masks, &user_winning_number, &prize_distribution.winning_number);
        // get prize tier fraction
        let prize_tier_fraction = self.prize_tier_fraction(tier_match, prize_distribution.bit_range_size, &prize_distribution.tiers);
        let prize_to_take = prize_tier_fraction.multiply(prize_distribution.prize);

        log!("Prize to claim is {} {}", prize_to_take, self.deposited_token_id);
        self.get_yield_source().claim(&caller, &self.deposited_token_id, prize_to_take, draw_id, pick.0);
        
        return prize_to_take.into();
    }
}

#[cfg(test)]
mod tests{
    use crate::*;
    use crate::test_utils::{get_contract, mmmm, sec, mint};
    use common::types::U256;
    use common::utils;
    use crate::utils::utils::get_user_winning_number;
    use super::TIERS_NOMINAL;

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
        assert_eq!(prize_fraction.multiply(TIERS_NOMINAL), TIERS[0] as u128);

        let prize_fraction = contract.prize_tier_fraction(1, 1, &TIERS);
        assert_eq!(prize_fraction.multiply(TIERS_NOMINAL), TIERS[1] as u128);

        let prize_fraction = contract.prize_tier_fraction(1, 4, &TIERS);
        assert_eq!(prize_fraction.multiply(TIERS_NOMINAL), 2);
        
        let prize_fraction = contract.prize_tier_fraction(1, 2, &TIERS);
        assert_eq!(prize_fraction.multiply(TIERS_NOMINAL), 10);

        let prize_fraction = contract.prize_tier_fraction(2, 2, &TIERS);
        assert_eq!(prize_fraction.multiply(TIERS_NOMINAL), 1);
    }

    #[test]
    fn test_on_get_draw_and_add_prize_distribution(){
        let mut contract = get_contract();
        let acc_id = mmmm();
        let sec_id = sec();
        
        let draw = Draw {draw_id: 1, started_at:10, completed_at: 100, winning_random_number: utils::random_u256()};
        mint(&mut contract.tickets, &acc_id, 20000000000000000000000000, 5);
        mint(&mut contract.tickets, &sec_id, 10000000000000000000000000, 8);

        contract.on_get_draw_and_add_prize_distribution(U128(52439282415939890845657), 8, 4, Result::Ok(draw));
    }

    #[test]
    fn test_claim(){
        assert_eq!("18327974331163228590", "18327974331163228590");
        let contract = get_contract();
        let winning_num = U256::from_dec_str("110764760720936366222033825268428769436781304912252756802424649970273906838888").unwrap();

        let caller=AccountId::new_unchecked("test1.test.near".to_string());
        let cardinality = 4;
        let masks = contract.create_masks(1, cardinality);
        for x in 1..4{
            let user_winning_number = get_user_winning_number(&caller, x);
            let tier_match = contract.get_tier_match(&masks, &user_winning_number, &winning_num);
            if tier_match < 8{
                println!("{} {}", tier_match, x);
                let fraction = contract.prize_tier_fraction(tier_match, 1, &TIERS);
                
            }
        }

        
        // get tier match
        
    }
}