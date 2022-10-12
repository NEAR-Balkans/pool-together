pub mod generic_ring_buffer{

    use borsh::{BorshDeserialize, BorshSerialize};
    pub trait RingBuffer<T : Copy>{
        fn next_index(&self) -> usize;
        fn add(&mut self, element: &T);
        fn get(&self, idx: usize) -> T;
    }

    #[derive(BorshSerialize, BorshDeserialize)]
    pub struct GenericRingBuffer<T, const CAPACITY: usize>{
        pub arr: [T; CAPACITY],
        current_index: usize,
    }

    impl<T: Default + Copy, const CAPACITY: usize> Default for GenericRingBuffer<T, CAPACITY>{
        fn default() -> Self {
            assert_ne!(CAPACITY, 0, "capacity cannot be lower than 1");
    
            let arr = [T::default(); CAPACITY];
    
            return Self { arr: arr, current_index: 0 };
        }
    }
    
    impl<T: Default + Copy, const CAPACITY: usize> GenericRingBuffer<T, CAPACITY>{
        pub fn new() -> Self{
            Self::default()
        }
    }

    impl<T: Copy, const CAPACITY: usize> RingBuffer<T> for GenericRingBuffer<T, CAPACITY>{
        fn next_index(&self) -> usize{
            return (self.current_index + 1) % self.arr.len();
        }

        fn add(&mut self, element: &T){
            let next_idx = self.next_index();
            
            self.arr[self.current_index] = *element;
            self.current_index = next_idx;
        }

        fn get(&self, idx: usize) -> T{
            self.arr[idx]
        }
    }
}

pub mod types;

#[cfg(test)]
mod tests {
    use crate::generic_ring_buffer::{GenericRingBuffer, RingBuffer};

    #[test]
    fn test_default_elements_in_buffer() {
        let result = 2 + 2;
        assert_eq!(result, 4);
        let x = GenericRingBuffer::<u32, 5>::new();
        for el in 0..x.arr.len(){
            assert_eq!(x.arr[el], u32::default());
        }
    }

    #[test]
    fn test_ring_buffer() {
        let mut buffer = GenericRingBuffer::<u32, 3>::new();
        let mut current_element = 1;
        buffer.add(&current_element);
        current_element = 2;
        buffer.add(&current_element);
        current_element = 3;
        buffer.add(&current_element);
        current_element = 4;
        buffer.add(&current_element);
        
        assert!(buffer.get(0) == 4);
        current_element = 5;
        buffer.add(&current_element);
        assert!(buffer.get(1) == 5);
    }
}
