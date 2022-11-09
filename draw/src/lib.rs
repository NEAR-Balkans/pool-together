use common::generic_ring_buffer::{GenericRingBuffer, RingBuffer, Identifier};
use common::types::{DrawId, U256};
use near_sdk::collections::LookupMap;
use near_sdk::{env, near_bindgen, EpochHeight, PanicOnDefault};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use interfaces::draw::{DrawCreator, Draw, DrawBuffer, DrawRegister};

mod interfaces;

const DRAW_DURATION_IN_EPOCHS: u64 = 5;
const DRAW_BUFFER_CAPACITY:usize = 3;

#[cfg(test)]
mod test_utils;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract{
    draw_buffer: GenericRingBuffer<Draw, DrawId, DRAW_BUFFER_CAPACITY>,
    last_epoch_started: EpochHeight,
    is_started: bool,
    temp_draw: Draw,
}

fn as_u256(arr: &[u8; 32]) -> U256{
    let mut result:U256 = U256::zero();
    let mut shift:u16 = 0;

    for idx in 0..arr.len(){
        result += U256::from(arr[idx]) << shift;
        shift += 8;
    }

    return result;
}

fn random_u256() -> U256{
    let random_seed = env::random_seed(); // len 32
    println!("Random seed is {:?}", random_seed.to_vec());
    // using first 16 bytes (doesn't affect randomness)
    return as_u256(random_seed.as_slice().try_into().expect("random seed of incorrect length"));
}

impl Identifier<DrawId> for Draw{
    fn id(&self) -> DrawId {
        self.draw_id
    }
}

#[near_bindgen]
impl Contract{
    #[init]
    pub fn new() -> Self{
        Self { 
            draw_buffer: GenericRingBuffer::<Draw, DrawId, DRAW_BUFFER_CAPACITY>::new(), 
            last_epoch_started: 0, 
            is_started: false, 
            temp_draw: Draw::default(),
        }
    }
}

#[near_bindgen]
impl DrawRegister for Contract{
    fn get_draws(&self, from_index: usize, limit: usize) -> Vec<Draw>{
        return self.draw_buffer.arr
            .iter()
            .cloned()
            .skip(from_index)
            .take(limit)
            .collect::<Vec<Draw>>();
    }
}

#[near_bindgen]
impl DrawBuffer for Contract{
    fn get_draw(&self, id: DrawId) -> Draw{
        return self.draw_buffer.get_by_identifier(id);
    }
}

#[near_bindgen]
impl DrawCreator for Contract{
    fn can_start_draw(&self) -> bool{
        return !self.is_started;
    }

    fn can_complete_draw(&self) -> bool {
        return self.is_started && env::epoch_height() >= self.last_epoch_started + DRAW_DURATION_IN_EPOCHS;
    }

    fn start_draw(&mut self) {
        if !self.can_start_draw(){
            return;
        }

        self.is_started = true;
        self.last_epoch_started = env::epoch_height();
        self.temp_draw.started_at = env::block_timestamp_ms();
        self.temp_draw.draw_id = self.temp_draw.draw_id + 1;
    }

    fn complete_draw(&mut self) {
        if !self.can_complete_draw() {
            return;
        }

        self.is_started = false;
        self.last_epoch_started = 0;
        self.temp_draw.winning_random_number = random_u256();
        self.temp_draw.completed_at = env::block_timestamp_ms();

        self.draw_buffer.add(&self.temp_draw);
    }
}

#[cfg(test)]
pub mod tests {
    use crate::interfaces::draw::{Draw};
    use common::{generic_ring_buffer::{GenericRingBuffer, RingBuffer}};

    use rand::Rng;
    use super::*;
    use crate::test_utils::tests::*;

    impl PartialEq for Draw{
        fn eq(&self, other: &Self) -> bool {
            self.completed_at == other.completed_at
                && self.draw_id == other.draw_id
                && self.started_at == other.started_at
                && self.winning_random_number == other.winning_random_number
        }
    }

    fn generate_random_seed() -> [u8; 32]{
        return rand::thread_rng().gen::<[u8; 32]>();
    }

    fn test_arr(arr: &[u8; 32]){
        for idx in 0..arr.len(){
            println!("{} {}", idx, arr[idx]);
        }
    }

    #[test]
    fn test_conversion(){
        let seed = generate_random_seed();
        let _res256 = as_u256(seed.as_slice().try_into().expect("msg"));
    }

    #[test]
    fn test_ring_buffer_space_management(){
        let mut buffer = GenericRingBuffer::<Draw, DrawId, 3>::new();
        let mut current_draw = Draw::default();

        current_draw.winning_random_number = U256::from(1);
        current_draw.draw_id = 12;
        buffer.add(&current_draw);
        
        current_draw.winning_random_number = U256::from(2);
        current_draw.draw_id = 21;
        buffer.add(&current_draw);
        current_draw.winning_random_number = U256::from(3);
        current_draw.draw_id = 300;
        buffer.add(&current_draw);
        current_draw.winning_random_number = U256::from(4);
        current_draw.draw_id = 100;
        buffer.add(&current_draw);
        assert_eq!(buffer.get_by_identifier(12), Draw::default())
    }

    #[test]
    fn test_ring_buffer() {
        let mut buffer = GenericRingBuffer::<Draw, DrawId, 3>::new();
        let mut current_draw = Draw::default();
        
        current_draw.winning_random_number = U256::from(1);
        buffer.add(&current_draw);
        current_draw.winning_random_number = U256::from(2);
        buffer.add(&current_draw);
        current_draw.winning_random_number = U256::from(3);
        buffer.add(&current_draw);
        current_draw.winning_random_number = U256::from(4);
        buffer.add(&current_draw);
        
        assert!(buffer.get_by_index(0).winning_random_number == U256::from(4));
        current_draw.winning_random_number = U256::from(5);
        buffer.add(&current_draw);
        assert!(buffer.get_by_index(1).winning_random_number == U256::from(5));
    }

    #[test]
    fn test_if_can_start_draw(){
        let mut emulator = Emulator::new();
        
        assert_eq!(emulator.contract.can_start_draw(), true);
        assert_eq!(emulator.contract.can_complete_draw(), false);

        emulator.contract.start_draw();
        emulator.skip_epochs(1, generate_random_seed());
        assert_eq!(emulator.contract.can_start_draw(), false);
        assert_eq!(emulator.contract.can_complete_draw(), false);

        emulator.skip_epochs(4, generate_random_seed());
        assert_eq!(emulator.contract.can_start_draw(), false);
        assert_eq!(emulator.contract.can_complete_draw(), true);

        emulator.contract.complete_draw();
        let mut current_draw_id = emulator.contract.temp_draw.draw_id;

        for _ in 0..4{
            assert_eq!(emulator.contract.can_start_draw(), true);
            emulator.contract.start_draw();
            emulator.skip_epochs(5, generate_random_seed());
            assert_eq!(emulator.contract.can_complete_draw(), true);
            emulator.contract.complete_draw();
            assert_eq!(emulator.contract.temp_draw.draw_id, current_draw_id + 1);
            current_draw_id+=1;
        }
    }
}
