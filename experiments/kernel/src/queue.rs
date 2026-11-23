//! A fixed-size ring buffer: no allocation, `Copy` items.

/// A FIFO queue holding at most `N` items.
#[derive(Clone, Copy, Debug)]
pub struct Queue<T: Copy + Default, const N: usize> {
    buf: [T; N],
    head: usize,
    len: usize,
}

impl<T: Copy + Default, const N: usize> Default for Queue<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy + Default, const N: usize> Queue<T, N> {
    /// An empty queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buf: [T::default(); N],
            head: 0,
            len: 0,
        }
    }

    /// Number of items waiting.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether nothing is waiting.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the queue is full.
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len == N
    }

    /// Adds `item` at the back; gives it back when the queue is full.
    ///
    /// # Errors
    /// Returns `Err(item)` when there is no room.
    pub fn push(&mut self, item: T) -> Result<(), T> {
        if self.is_full() {
            return Err(item);
        }
        self.buf[(self.head + self.len) % N] = item;
        self.len += 1;
        Ok(())
    }

    /// Removes the oldest item.
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let item = self.buf[self.head];
        self.head = (self.head + 1) % N;
        self.len -= 1;
        Some(item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_order_and_wraparound() {
        let mut q = Queue::<u8, 3>::new();
        for round in 0..5u8 {
            assert_eq!(q.push(round), Ok(()));
            assert_eq!(q.push(round + 100), Ok(()));
            assert_eq!(q.pop(), Some(round));
            assert_eq!(q.pop(), Some(round + 100));
        }
        assert!(q.is_empty());
    }

    #[test]
    fn full_queue_returns_the_item() {
        let mut q = Queue::<u8, 2>::new();
        assert_eq!(q.push(1), Ok(()));
        assert_eq!(q.push(2), Ok(()));
        assert!(q.is_full());
        assert_eq!(q.push(3), Err(3));
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn zero_capacity_is_always_full() {
        let mut q = Queue::<u8, 0>::new();
        assert_eq!(q.push(1), Err(1));
        assert_eq!(q.pop(), None);
    }
}
