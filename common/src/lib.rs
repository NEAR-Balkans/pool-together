pub mod generic_ring_buffer{

    use near_sdk::collections::LookupMap;
    use borsh::{BorshDeserialize, BorshSerialize};
    pub trait Identifier<T>{
        fn id(&self) -> T;
    }
    pub trait RingBuffer<TID: BorshDeserialize + BorshSerialize, T : Copy + Identifier<TID>>{
        /// calculate next index
        fn next_index(&self) -> usize;
        /// adds element to the buffer
        fn add(&mut self, element: &T);
        /// return the element at index
        fn get_by_index(&self, idx: usize) -> T;
        fn get_by_identifier(&self, id: TID) -> T;
    }

    #[derive(BorshSerialize, BorshDeserialize)]
    pub struct GenericRingBuffer<T, TID, const CAPACITY: usize>{
        pub arr: [T; CAPACITY],
        current_index: usize,
        id_idx_map: LookupMap<TID, usize>,
    }

    impl<TID, T: Default + Copy + Identifier<TID>, const CAPACITY: usize> Default for GenericRingBuffer<T, TID, CAPACITY>{
        fn default() -> Self {
            assert_ne!(CAPACITY, 0, "capacity cannot be lower than 1");
    
            let arr = [T::default(); CAPACITY];
    
            return Self { arr: arr, current_index: 0, id_idx_map: LookupMap::new(b"iim".to_vec()) };
        }
    }
    
    impl<TID: BorshDeserialize + BorshSerialize, T: Default + Copy + Identifier<TID>, const CAPACITY: usize> GenericRingBuffer<T, TID, CAPACITY>{
        pub fn new() -> Self{
            Self::default()
        }
    }

    impl<TID: BorshDeserialize + BorshSerialize, T: Copy + Default + Identifier<TID>, const CAPACITY: usize> RingBuffer<TID, T> for GenericRingBuffer<T, TID, CAPACITY>{
        fn next_index(&self) -> usize{
            return (self.current_index + 1) % self.arr.len();
        }

        fn add(&mut self, element: &T){
            let next_idx = self.next_index();
            let replaced_element = self.arr[self.current_index];
            self.arr[self.current_index] = *element;
            self.id_idx_map.insert(&element.id(), &self.current_index);
            
            if self.id_idx_map.contains_key(&replaced_element.id()){
                self.id_idx_map.remove(&replaced_element.id());
            }

            self.current_index = next_idx;
        }

        fn get_by_index(&self, idx: usize) -> T{
            self.arr[idx]
        }

        fn get_by_identifier(&self, id: TID) -> T {
            return self.id_idx_map
                        .get(&id)
                        .map(|el| self.get_by_index(el))
                        .unwrap_or_default();
        }
    }
}

pub mod types;

pub mod utils{
    use near_sdk::env;

    use crate::types::U256;
    fn as_u256(arr: &[u8; 32]) -> U256{
        let mut result:U256 = U256::zero();
        let mut shift:u16 = 0;
    
        for idx in 0..arr.len(){
            result += U256::from(arr[idx]) << shift;
            shift += 8;
        }
    
        return result;
    }

    pub fn random_u256() -> U256{
        let random_seed = env::random_seed(); // len 32
        println!("Random seed is {:?}", random_seed.to_vec());
        // using first 16 bytes (doesn't affect randomness)
        return as_u256(random_seed.as_slice().try_into().expect("random seed of incorrect length"));
    }
    
}

#[cfg(test)]
mod tests {
    use crate::generic_ring_buffer::{GenericRingBuffer, RingBuffer, Identifier};

    impl Identifier<u32> for u32{
        fn id(&self) -> u32 {
            return *self;
    }
    }

    #[test]
    fn test_default_elements_in_buffer() {
        let result = 2 + 2;
        assert_eq!(result, 4);
        let x = GenericRingBuffer::<u32, u32, 5>::new();
        for el in 0..x.arr.len(){
            assert_eq!(x.arr[el], u32::default());
        }
    }

    #[test]
    fn test_ring_buffer() {
        let mut buffer = GenericRingBuffer::<u32, u32, 3>::new();
        let mut current_element = 1;
        buffer.add(&current_element);
        current_element = 2;
        buffer.add(&current_element);
        current_element = 3;
        buffer.add(&current_element);
        current_element = 4;
        buffer.add(&current_element);
        
        assert!(buffer.get_by_index(0) == 4);
        current_element = 5;
        buffer.add(&current_element);
        assert!(buffer.get_by_index(1) == 5);
    }
}
