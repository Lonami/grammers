// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use std::{
    ops::{Index, IndexMut},
    slice::SliceIndex,
};

#[derive(Clone, Debug)]
pub struct RingBuffer<T: Copy + Default> {
    buffer: Vec<T>,
    head: usize,
    default_head: usize,
}

pub struct View<'a, T: Copy + Default> {
    pub view: &'a mut [T],
    pub pos: usize,
}

impl<T: Copy + Default> RingBuffer<T> {
    pub fn with_capacity(capacity: usize, default_head: usize) -> Self {
        let mut buffer = Vec::with_capacity(default_head + capacity);
        buffer.extend((0..default_head).map(|_| T::default()));
        Self {
            buffer,
            head: default_head,
            default_head,
        }
    }

    pub fn clear(&mut self) {
        self.buffer.truncate(self.default_head);
        self.buffer.fill(T::default());
        self.head = self.default_head;
    }

    pub fn shift(&mut self, amount: usize) -> View<'_, T> {
        if self.head >= amount {
            self.head -= amount
        } else {
            let shift = amount - self.head;
            self.buffer.extend((0..shift).map(|_| T::default()));
            self.buffer.rotate_right(shift);
            self.head = 0;
        }
        View {
            view: &mut self.buffer[self.head..self.head + amount],
            pos: 0,
        }
    }

    pub fn skip(&mut self, amount: usize) {
        self.head += amount;
        assert!(self.head <= self.buffer.len());
    }

    pub fn push(&mut self, value: T) {
        self.buffer.push(value)
    }

    /// Reclaim leading data, by shifting it to start at the default head.
    pub fn reclaim_leading(&mut self) {
        if self.head <= self.default_head {
            return;
        }

        let len = self.buffer.len();
        self.buffer.copy_within(self.head..len, self.default_head);
        self.buffer.truncate(self.default_head + len - self.head);
        self.head = self.default_head;
    }

    pub fn fill_remaining(&mut self) {
        let missing = self.buffer.capacity() - self.buffer.len();
        self.buffer.extend((0..missing).map(|_| T::default()));
    }

    pub fn is_empty(&self) -> bool {
        self.head == self.buffer.len()
    }

    pub fn len(&self) -> usize {
        self.buffer.len() - self.head
    }
}

impl<T: Copy + Default> AsRef<[T]> for RingBuffer<T> {
    fn as_ref(&self) -> &[T] {
        &self.buffer[self.head..]
    }
}

impl<T: Copy + Default> AsMut<[T]> for RingBuffer<T> {
    fn as_mut(&mut self) -> &mut [T] {
        &mut self.buffer[self.head..]
    }
}

impl<T: Copy + Default, I: SliceIndex<[T]>> Index<I> for RingBuffer<T> {
    type Output = I::Output;

    fn index(&self, index: I) -> &Self::Output {
        self.as_ref().index(index)
    }
}

impl<T: Copy + Default, I: SliceIndex<[T]>> IndexMut<I> for RingBuffer<T> {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        self.as_mut().index_mut(index)
    }
}

impl<T: Copy + Default> Extend<T> for RingBuffer<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.buffer.extend(iter)
    }
}

impl<'a, T: Copy + Default + 'a> Extend<&'a T> for RingBuffer<T> {
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        self.buffer.extend(iter)
    }
}

impl<'a, T: Copy + Default> Extend<T> for View<'a, T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        iter.into_iter().for_each(|value| {
            self.view[self.pos] = value;
            self.pos += 1;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pretty-print a ring buffer, including values, capacity, and head position.
    fn repr(ring: &RingBuffer<u8>) -> String {
        use std::fmt::Write as _;

        let mut repr = String::new();
        repr.push('[');
        ring.buffer.iter().enumerate().for_each(|(i, c)| {
            repr.push(if ring.head == i { '|' } else { ' ' });
            write!(repr, "{}", c).unwrap();
        });
        repr.push(if ring.head == ring.buffer.len() {
            '|'
        } else {
            ' '
        });
        (ring.buffer.len()..ring.buffer.capacity()).for_each(|_| {
            repr.push('?');
            repr.push(' ');
        });
        repr.push(']');
        repr
    }

    fn sanity_checks(ring: &RingBuffer<u8>) {
        assert_eq!(ring.as_ref().len(), ring.len());
        assert_eq!(ring.is_empty(), ring.len().eq(&0));
    }

    #[test]
    fn initialization() {
        let buffer = RingBuffer::<u8>::with_capacity(0, 0);
        sanity_checks(&buffer);
        assert!(buffer.is_empty());
        assert_eq!(repr(&buffer), "[|]");

        let buffer = RingBuffer::<u8>::with_capacity(0, 4);
        sanity_checks(&buffer);
        assert!(buffer.is_empty());
        assert_eq!(repr(&buffer), "[ 0 0 0 0|]");

        let buffer = RingBuffer::<u8>::with_capacity(4, 0);
        sanity_checks(&buffer);
        assert!(buffer.is_empty());
        assert_eq!(repr(&buffer), "[|? ? ? ? ]");

        let buffer = RingBuffer::<u8>::with_capacity(2, 4);
        sanity_checks(&buffer);
        assert!(buffer.is_empty());
        assert_eq!(repr(&buffer), "[ 0 0 0 0|? ? ]");
    }

    #[test]
    fn shift_extends_if_needed() {
        let mut buffer = RingBuffer::<u8>::with_capacity(2, 4);

        assert_eq!(buffer.shift(3).view, vec![0; 3]);
        sanity_checks(&buffer);
        assert_eq!(repr(&buffer), "[ 0|0 0 0 ? ? ]");

        assert_eq!(buffer.shift(1).view, vec![0; 1]);
        sanity_checks(&buffer);
        assert_eq!(repr(&buffer), "[|0 0 0 0 ? ? ]");

        assert_eq!(buffer.shift(2).view, vec![0; 2]);
        sanity_checks(&buffer);
        assert_eq!(repr(&buffer), "[|0 0 0 0 0 0 ]");

        let mut buffer = RingBuffer::<u8>::with_capacity(2, 4);

        assert_eq!(buffer.shift(5).view, vec![0; 5]);
        sanity_checks(&buffer);
        assert_eq!(repr(&buffer), "[|0 0 0 0 0 ? ]");

        assert_eq!(buffer.shift(2).view, vec![0; 2]);
        sanity_checks(&buffer);
        assert!(repr(&buffer).starts_with("[|0 0 0 0 0 0 0 ?")); // don't assume Vec's growth
    }

    #[test]
    fn mutates_as_expected() {
        let mut buffer = RingBuffer::<u8>::with_capacity(6, 4);

        buffer.extend(1..=2);
        sanity_checks(&buffer);
        assert_eq!(repr(&buffer), "[ 0 0 0 0|1 2 ? ? ? ? ]");

        buffer.push(3);
        sanity_checks(&buffer);
        assert_eq!(repr(&buffer), "[ 0 0 0 0|1 2 3 ? ? ? ]");

        let mut head = buffer.shift(3);
        head.extend([4, 5, 6]);
        sanity_checks(&buffer);
        assert_eq!(repr(&buffer), "[ 0|4 5 6 1 2 3 ? ? ? ]");

        buffer.clear();
        assert_eq!(repr(&buffer), "[ 0 0 0 0|? ? ? ? ? ? ]");
    }

    #[test]
    fn reclaims_as_expected() {
        let mut buffer = RingBuffer::<u8>::with_capacity(6, 4);
        buffer.extend([1, 2, 3, 4, 5, 6]);
        assert_eq!(repr(&buffer), "[ 0 0 0 0|1 2 3 4 5 6 ]");
        buffer.reclaim_leading();
        assert_eq!(repr(&buffer), "[ 0 0 0 0|1 2 3 4 5 6 ]");

        buffer.skip(2);
        assert_eq!(repr(&buffer), "[ 0 0 0 0 1 2|3 4 5 6 ]");
        buffer.reclaim_leading();
        assert_eq!(repr(&buffer), "[ 0 0 0 0|3 4 5 6 ? ? ]");
        buffer.reclaim_leading();

        buffer.shift(2);
        assert_eq!(repr(&buffer), "[ 0 0|0 0 3 4 5 6 ? ? ]");
        buffer.reclaim_leading();
        assert_eq!(repr(&buffer), "[ 0 0|0 0 3 4 5 6 ? ? ]");

        buffer.clear();
        buffer.shift(2);
        buffer.extend([1, 2, 3, 4, 5, 6]);
        assert_eq!(repr(&buffer), "[ 0 0|0 0 1 2 3 4 5 6 ]");
        buffer.reclaim_leading();

        buffer.skip(4);
        assert_eq!(repr(&buffer), "[ 0 0 0 0 1 2|3 4 5 6 ]");
        buffer.reclaim_leading();
        assert_eq!(repr(&buffer), "[ 0 0 0 0|3 4 5 6 ? ? ]");
        buffer.fill_remaining();
        assert_eq!(repr(&buffer), "[ 0 0 0 0|3 4 5 6 0 0 ]");
    }
}
