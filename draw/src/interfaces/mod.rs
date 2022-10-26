pub mod draw {
    use common::types::WinningNumber;
    use common::types::DrawId;
    use near_sdk::{borsh::{self, BorshDeserialize, BorshSerialize}, serde::{Serialize, Deserialize}};

    #[derive(Clone, Debug, Copy, Default)]
    #[derive(BorshDeserialize, BorshSerialize)]
    #[derive(Serialize, Deserialize)]
    #[serde(crate = "near_sdk::serde")]
    pub struct Draw {
        pub winning_random_number: WinningNumber,
        pub draw_id: DrawId,
        pub started_at: u64,
        pub completed_at: u64,
    }

    pub trait DrawCreator{
        fn can_start_draw(&self) -> bool;
        fn can_complete_draw(&self) -> bool;
        fn start_draw(&mut self);
        fn complete_draw(&mut self);
    }

    pub trait DrawBuffer{
        fn get_draw(&self, draw_id: DrawId) -> Draw;
    }

    pub trait DrawRegister{
        fn get_draws(&self, from_index: usize, limit: usize) -> Vec<Draw>;
    }
}