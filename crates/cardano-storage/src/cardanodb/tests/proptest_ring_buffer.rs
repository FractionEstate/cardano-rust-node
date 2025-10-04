//! Property-based tests for the ring buffer implementation
//!
//! These tests verify that the ring buffer maintains its invariants
//! under arbitrary sequences of operations.

use crate::cardanodb::volatile::ring_buffer::RingBuffer;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum RingBufferOp {
    Push(u64),
    Get(usize),
    Remove(usize),
    Clear,
}

// Strategy for generating ring buffer operations
fn ring_buffer_op_strategy() -> impl Strategy<Value = RingBufferOp> {
    prop_oneof![
        any::<u64>().prop_map(RingBufferOp::Push),
        (0usize..100).prop_map(RingBufferOp::Get),
        (0usize..100).prop_map(RingBufferOp::Remove),
        Just(RingBufferOp::Clear),
    ]
}

proptest! {
    #[test]
    fn ring_buffer_len_never_exceeds_capacity(
        capacity in 1usize..100,
        values in prop::collection::vec(any::<u64>(), 0..200)
    ) {
        let mut buffer = RingBuffer::with_capacity(capacity);

        for value in values {
            buffer.push(value);
            prop_assert!(buffer.len() <= capacity);
        }
    }

    #[test]
    fn ring_buffer_empty_after_clear(
        capacity in 1usize..100,
        values in prop::collection::vec(any::<u64>(), 0..200)
    ) {
        let mut buffer = RingBuffer::with_capacity(capacity);

        for value in values {
            buffer.push(value);
        }

        buffer.clear();
        prop_assert_eq!(buffer.len(), 0);
        prop_assert!(buffer.is_empty());
    }

    #[test]
    fn ring_buffer_get_returns_pushed_values(
        capacity in 10usize..100,
        values in prop::collection::vec(any::<u64>(), 1..10)
    ) {
        // Use capacity larger than values to avoid eviction
        let mut buffer = RingBuffer::with_capacity(capacity);

        for &value in &values {
            buffer.push(value);
        }

        // Values should be retrievable in order
        for (i, &expected) in values.iter().enumerate() {
            let actual = buffer.get(i);
            prop_assert_eq!(actual, Some(&expected));
        }
    }

    #[test]
    fn ring_buffer_fifo_eviction(
        capacity in 5usize..20,
        values in prop::collection::vec(any::<u64>(), 50..100)
    ) {
        let mut buffer = RingBuffer::with_capacity(capacity);

        for value in &values {
            buffer.push(*value);
        }

        // Should contain the last `capacity` elements
        let expected_start = values.len() - capacity;
        for (i, &expected) in values[expected_start..].iter().enumerate() {
            prop_assert_eq!(buffer.get(i), Some(&expected));
        }
    }

    #[test]
    fn ring_buffer_remove_decreases_len(
        capacity in 5usize..20,
        values in prop::collection::vec(any::<u64>(), 1..10),
        remove_idx in 0usize..9
    ) {
        prop_assume!(remove_idx < values.len());
        prop_assume!(values.len() <= capacity); // Ensure no eviction

        let mut buffer = RingBuffer::with_capacity(capacity);
        for &value in &values {
            buffer.push(value);
        }

        let len_before = buffer.len();
        if remove_idx < len_before {
            buffer.remove(remove_idx);
            let len_after = buffer.len();
            prop_assert_eq!(len_after, len_before - 1);
        }
    }    #[test]
    fn ring_buffer_iter_yields_all_elements(
        capacity in 10usize..50,
        values in prop::collection::vec(any::<u64>(), 1..10)
    ) {
        let mut buffer = RingBuffer::with_capacity(capacity);

        for &value in &values {
            buffer.push(value);
        }

        let collected: Vec<u64> = buffer.iter().copied().collect();
        prop_assert_eq!(collected, values);
    }

    #[test]
    fn ring_buffer_operations_maintain_invariants(
        capacity in 5usize..50,
        ops in prop::collection::vec(ring_buffer_op_strategy(), 0..100)
    ) {
        let mut buffer = RingBuffer::with_capacity(capacity);

        for op in ops {
            match op {
                RingBufferOp::Push(value) => {
                    buffer.push(value);
                }
                RingBufferOp::Get(idx) => {
                    if idx < buffer.len() {
                        let _ = buffer.get(idx);
                    }
                }
                RingBufferOp::Remove(idx) => {
                    if idx < buffer.len() {
                        buffer.remove(idx);
                    }
                }
                RingBufferOp::Clear => {
                    buffer.clear();
                }
            }

            // Invariants that should always hold
            prop_assert!(buffer.len() <= capacity);
            prop_assert_eq!(buffer.is_empty(), buffer.len() == 0);

            // All indexed elements should be accessible
            for i in 0..buffer.len() {
                prop_assert!(buffer.get(i).is_some());
            }

            // Out of bounds access should return None
            prop_assert!(buffer.get(buffer.len()).is_none());
        }
    }

    #[test]
    fn ring_buffer_wraparound_preserves_order(
        capacity in 5usize..20,
        iterations in 5usize..50
    ) {
        let mut buffer = RingBuffer::with_capacity(capacity);

        // Push more elements than capacity to force wraparound
        for i in 0..(capacity * iterations) {
            buffer.push(i as u64);
        }

        // Buffer should contain last `capacity` elements in order
        let start = (capacity * iterations) - capacity;
        for i in 0..capacity {
            let expected = (start + i) as u64;
            prop_assert_eq!(buffer.get(i), Some(&expected));
        }
    }

    #[test]
    fn ring_buffer_remove_preserves_remaining_order(
        capacity in 10usize..30,
        remove_idx in 0usize..4
    ) {
        // Use unique sequential values to test ordering
        let values: Vec<u64> = (0..5).collect();
        prop_assume!(remove_idx < values.len() && values.len() <= capacity);

        let mut buffer = RingBuffer::with_capacity(capacity);
        for &value in &values {
            buffer.push(value);
        }

        buffer.remove(remove_idx);

        // Remaining elements should maintain relative order
        let remaining: Vec<u64> = buffer.iter().copied().collect();

        // Create expected by removing the element at remove_idx
        let mut expected = values.clone();
        expected.remove(remove_idx);

        prop_assert_eq!(remaining, expected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_test_ring_buffer_operations() {
        // Verify the strategy produces valid operations
        proptest!(|(ops in prop::collection::vec(ring_buffer_op_strategy(), 0..10))| {
            prop_assert!(ops.len() <= 10);
        });
    }
}
