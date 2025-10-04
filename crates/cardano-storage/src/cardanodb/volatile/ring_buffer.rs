//! Ring buffer implementation for VolatileDB
//!
//! A fixed-size circular buffer that automatically evicts the oldest element
//! when full. Provides O(1) insertion and O(1) indexed access.

/// A fixed-size ring buffer
pub struct RingBuffer<T> {
    buffer: Vec<Option<T>>,
    head: usize,
    tail: usize,
    count: usize,
    capacity: usize,
}

impl<T> RingBuffer<T> {
    /// Create a new ring buffer with the given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer.push(None);
        }

        Self {
            buffer,
            head: 0,
            tail: 0,
            count: 0,
            capacity,
        }
    }

    /// Push an element, returning the oldest if buffer is full
    pub fn push(&mut self, item: T) -> Option<T>
    where
        T: Clone,
    {
        let old = if self.count == self.capacity {
            // Buffer is full, remove oldest
            self.buffer[self.tail].take()
        } else {
            None
        };

        self.buffer[self.head] = Some(item);
        self.head = (self.head + 1) % self.capacity;

        if self.count < self.capacity {
            self.count += 1;
        } else {
            self.tail = (self.tail + 1) % self.capacity;
        }

        old
    }

    /// Get an element by index (0 = oldest, len-1 = newest)
    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.count {
            let actual_index = (self.tail + index) % self.capacity;
            self.buffer[actual_index].as_ref()
        } else {
            None
        }
    }

    /// Remove an element by index
    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index < self.count {
            let actual_index = (self.tail + index) % self.capacity;
            let removed = self.buffer[actual_index].take();

            // Shift elements to fill the gap
            // This is O(n) but acceptable since removal is rare
            for i in index..self.count.saturating_sub(1) {
                let current = (self.tail + i) % self.capacity;
                let next = (self.tail + i + 1) % self.capacity;
                self.buffer[current] = self.buffer[next].take();
            }

            self.count = self.count.saturating_sub(1);
            if self.count > 0 {
                self.head = (self.tail + self.count) % self.capacity;
            } else {
                self.head = self.tail;
            }

            removed
        } else {
            None
        }
    }

    /// Get the number of elements currently in the buffer
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get the capacity of the buffer
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Iterate over all elements (from oldest to newest)
    pub fn iter(&self) -> RingBufferIter<'_, T> {
        RingBufferIter {
            buffer: self,
            index: 0,
        }
    }

    /// Clear all elements from the buffer
    pub fn clear(&mut self) {
        for item in self.buffer.iter_mut() {
            *item = None;
        }
        self.head = 0;
        self.tail = 0;
        self.count = 0;
    }
}

impl<T: Clone> Clone for RingBuffer<T> {
    fn clone(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
            head: self.head,
            tail: self.tail,
            count: self.count,
            capacity: self.capacity,
        }
    }
}

/// Iterator over ring buffer elements
pub struct RingBufferIter<'a, T> {
    buffer: &'a RingBuffer<T>,
    index: usize,
}

impl<'a, T> Iterator for RingBufferIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.buffer.count {
            let item = self.buffer.get(self.index);
            self.index += 1;
            item
        } else {
            None
        }
    }
}

impl<'a, T> ExactSizeIterator for RingBufferIter<'a, T> {
    fn len(&self) -> usize {
        self.buffer.count.saturating_sub(self.index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_basic_operations() {
        let mut buffer = RingBuffer::with_capacity(3);

        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());

        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.get(0), Some(&1));
        assert_eq!(buffer.get(1), Some(&2));
        assert_eq!(buffer.get(2), Some(&3));
    }

    #[test]
    fn ring_buffer_eviction() {
        let mut buffer = RingBuffer::with_capacity(3);

        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        // Push 4th element - should evict 1
        let evicted = buffer.push(4);
        assert_eq!(evicted, Some(1));
        assert_eq!(buffer.len(), 3);

        // Elements should now be [2, 3, 4]
        assert_eq!(buffer.get(0), Some(&2));
        assert_eq!(buffer.get(1), Some(&3));
        assert_eq!(buffer.get(2), Some(&4));
    }

    #[test]
    fn ring_buffer_iteration() {
        let mut buffer = RingBuffer::with_capacity(5);

        for i in 0..5 {
            buffer.push(i);
        }

        let collected: Vec<_> = buffer.iter().copied().collect();
        assert_eq!(collected, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn ring_buffer_removal() {
        let mut buffer = RingBuffer::with_capacity(5);

        buffer.push(10);
        buffer.push(20);
        buffer.push(30);

        // Remove middle element
        let removed = buffer.remove(1);
        assert_eq!(removed, Some(20));
        assert_eq!(buffer.len(), 2);

        assert_eq!(buffer.get(0), Some(&10));
        assert_eq!(buffer.get(1), Some(&30));
    }

    #[test]
    fn ring_buffer_wraparound() {
        let mut buffer = RingBuffer::with_capacity(3);

        // Fill buffer
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        // Push more, causing wraparound
        buffer.push(4); // evicts 1
        buffer.push(5); // evicts 2

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.get(0), Some(&3));
        assert_eq!(buffer.get(1), Some(&4));
        assert_eq!(buffer.get(2), Some(&5));
    }

    #[test]
    fn ring_buffer_clear() {
        let mut buffer = RingBuffer::with_capacity(3);

        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        buffer.clear();

        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert_eq!(buffer.get(0), None);
    }
}
