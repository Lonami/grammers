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

/// A growable buffer with the properties of a deque.
///
/// Unlike the standard [`VecDeque`](std::collections::VecDeque),
/// this buffer is designed to not need explicit calls to `make_contiguous`
/// and minimize the amount of memory moves.
#[derive(Clone, Debug)]
pub struct DequeBuffer<T: Copy + Default> {
    buffer: Vec<T>,
    head: usize,
    default_head: usize,
}

impl<T: Copy + Default> DequeBuffer<T> {
    /// Creates an empty deque buffer with space for at least `back_capacity` elements in the back,
    /// and exactly `front_capacity` elements in the front.
    pub fn with_capacity(back_capacity: usize, front_capacity: usize) -> Self {
        let mut buffer = Vec::with_capacity(front_capacity + back_capacity);
        buffer.extend((0..front_capacity).map(|_| T::default()));
        Self {
            buffer,
            head: front_capacity,
            default_head: front_capacity,
        }
    }

    /// Clears the buffer, removing all values.
    pub fn clear(&mut self) {
        self.buffer.truncate(self.default_head);
        self.buffer.fill(T::default());
        self.head = self.default_head;
    }

    /// Extend the front by copying the elements from `slice`.
    pub fn extend_front(&mut self, slice: &[T]) {
        if self.head >= slice.len() {
            self.head -= slice.len()
        } else {
            let shift = slice.len() - self.head;
            self.buffer.extend((0..shift).map(|_| T::default()));
            self.buffer.rotate_right(shift);
            self.head = 0;
        }
        self.buffer[self.head..self.head + slice.len()].copy_from_slice(slice);
    }

    /// Appends an element to the back of the buffer.
    pub fn push(&mut self, value: T) {
        self.buffer.push(value)
    }

    /// Returns `true` if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.head == self.buffer.len()
    }

    /// Returns the number of elements in the buffer.
    pub fn len(&self) -> usize {
        self.buffer.len() - self.head
    }
}

impl<T: Copy + Default> AsRef<[T]> for DequeBuffer<T> {
    fn as_ref(&self) -> &[T] {
        &self.buffer[self.head..]
    }
}

impl<T: Copy + Default> AsMut<[T]> for DequeBuffer<T> {
    fn as_mut(&mut self) -> &mut [T] {
        &mut self.buffer[self.head..]
    }
}

impl<T: Copy + Default, I: SliceIndex<[T]>> Index<I> for DequeBuffer<T> {
    type Output = I::Output;

    fn index(&self, index: I) -> &Self::Output {
        self.as_ref().index(index)
    }
}

impl<T: Copy + Default, I: SliceIndex<[T]>> IndexMut<I> for DequeBuffer<T> {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        self.as_mut().index_mut(index)
    }
}

impl<T: Copy + Default> Extend<T> for DequeBuffer<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.buffer.extend(iter)
    }
}

impl<'a, T: Copy + Default + 'a> Extend<&'a T> for DequeBuffer<T> {
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        self.buffer.extend(iter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pretty-print a ring buffer, including values, capacity, and head position.
    fn repr(ring: &DequeBuffer<u8>) -> String {
        use std::fmt::Write as _;

        let mut repr = String::new();
        repr.push('[');
        ring.buffer.iter().enumerate().for_each(|(i, c)| {
            repr.push(if ring.head == i { '|' } else { ' ' });
            write!(repr, "{c}").unwrap();
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

    #[allow(clippy::len_zero)]
    fn sanity_checks(ring: &DequeBuffer<u8>) {
        assert_eq!(ring.as_ref().len(), ring.len());
        assert_eq!(ring.is_empty(), ring.len() == 0);
    }

    #[test]
    fn initialization() {
        let buffer = DequeBuffer::<u8>::with_capacity(0, 0);
        sanity_checks(&buffer);
        assert!(buffer.is_empty());
        assert_eq!(repr(&buffer), "[|]");

        let buffer = DequeBuffer::<u8>::with_capacity(0, 4);
        sanity_checks(&buffer);
        assert!(buffer.is_empty());
        assert_eq!(repr(&buffer), "[ 0 0 0 0|]");

        let buffer = DequeBuffer::<u8>::with_capacity(4, 0);
        sanity_checks(&buffer);
        assert!(buffer.is_empty());
        assert_eq!(repr(&buffer), "[|? ? ? ? ]");

        let buffer = DequeBuffer::<u8>::with_capacity(2, 4);
        sanity_checks(&buffer);
        assert!(buffer.is_empty());
        assert_eq!(repr(&buffer), "[ 0 0 0 0|? ? ]");
    }

    #[test]
    fn shift_extends_if_needed() {
        let mut buffer = DequeBuffer::<u8>::with_capacity(2, 4);

        buffer.extend_front(&[3, 3, 3]);
        sanity_checks(&buffer);
        assert_eq!(repr(&buffer), "[ 0|3 3 3 ? ? ]");

        buffer.extend_front(&[1]);
        sanity_checks(&buffer);
        assert_eq!(repr(&buffer), "[|1 3 3 3 ? ? ]");

        buffer.extend_front(&[2, 2]);
        sanity_checks(&buffer);
        assert_eq!(repr(&buffer), "[|2 2 1 3 3 3 ]");

        let mut buffer = DequeBuffer::<u8>::with_capacity(2, 4);

        buffer.extend_front(&[5, 5, 5, 5, 5]);
        sanity_checks(&buffer);
        assert_eq!(repr(&buffer), "[|5 5 5 5 5 ? ]");

        buffer.extend_front(&[2, 2]);
        sanity_checks(&buffer);
        assert!(repr(&buffer).starts_with("[|2 2 5 5 5 5 5 ?")); // don't assume Vec's growth
    }

    #[test]
    fn mutates_as_expected() {
        let mut buffer = DequeBuffer::<u8>::with_capacity(6, 4);

        buffer.extend(1..=2);
        sanity_checks(&buffer);
        assert_eq!(repr(&buffer), "[ 0 0 0 0|1 2 ? ? ? ? ]");

        buffer.push(3);
        sanity_checks(&buffer);
        assert_eq!(repr(&buffer), "[ 0 0 0 0|1 2 3 ? ? ? ]");

        buffer.extend_front(&[4, 5, 6]);
        sanity_checks(&buffer);
        assert_eq!(repr(&buffer), "[ 0|4 5 6 1 2 3 ? ? ? ]");

        buffer.clear();
        assert_eq!(repr(&buffer), "[ 0 0 0 0|? ? ? ? ? ? ]");
    }
}
