// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
#![cfg(any(feature = "markdown", feature = "html"))]

use std::cmp::Ordering;
use std::fmt::{self, Write as _};

pub const MENTION_URL_PREFIX: &str = "tg://user?id=";

/// The length of a string, according to Telegram.
///
/// Telegram considers the length of the string with surrogate pairs.
pub fn telegram_string_len(string: &str) -> i32 {
    // https://en.wikipedia.org/wiki/Plane_(Unicode)#Overview
    string.encode_utf16().count() as i32
}

/// Updates the length of the latest `MessageEntity` inside the specified vector.
///
/// # Examples
///
/// ```notrust
/// let mut vec = Vec::new();
/// push_entity!(MessageEntityBold(1) => vec);
/// update_entity_len!(MessageEntityBold(2) => vec);
/// ```
#[macro_export]
macro_rules! update_entity_len {
    ( $ty:ident($end_offset:expr) in $vector:expr ) => {
        let mut remove = false;
        let end_offset = $end_offset;
        let pos = $vector.iter_mut().rposition(|e| match e {
            tl::enums::MessageEntity::$ty(e) => {
                e.length = end_offset - e.offset;
                remove = e.length == 0;
                true
            }
            _ => false,
        });

        if remove {
            $vector.remove(pos.unwrap());
        }
    };
}

/// Represents the edge before or after a position.
/// Note that `After` sorts first, because it must be applied before the next `Before`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Edge {
    After,
    Before,
}

/// Represents an insertion position that can be accurately sorted.
#[derive(Debug, PartialEq, Eq)]
pub struct Position {
    offset: i32,
    edge: Edge,
    index: usize,
    part: u8,
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> Ordering {
        self.offset
            .cmp(&other.offset)
            .then_with(|| self.edge.cmp(&other.edge))
            .then_with(|| match self.edge {
                Edge::Before => self.index.cmp(&other.index),
                Edge::After => other.index.cmp(&self.index),
            })
            .then_with(|| self.part.cmp(&other.part))
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Represents a borrowed or owned segment of text to insert.
#[derive(Debug)]
pub enum Segment<'a> {
    Fixed(&'static str),
    String(&'a str),
    Number(i64),
}

impl<'a> Segment<'a> {
    fn len(&self) -> usize {
        match self {
            Self::Fixed(s) => s.len(),
            Self::String(s) => s.len(),
            Self::Number(n) => {
                let minus_sign = if *n < 0 { 1 } else { 0 };
                let digits = n.abs().ilog10() as usize + 1;
                minus_sign + digits
            }
        }
    }
}

impl<'a> fmt::Display for Segment<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fixed(s) => f.write_str(s),
            Self::String(s) => f.write_str(s),
            Self::Number(s) => write!(f, "{}", s),
        }
    }
}

/// Shorthand to create a position with `Edge::Before`.
#[inline(always)]
pub fn before(index: usize, part: u8, offset: i32) -> Position {
    Position {
        index,
        offset,
        edge: Edge::Before,
        part,
    }
}

/// Shorthand to create a position with `Edge::After`.
#[inline(always)]
pub fn after(index: usize, part: u8, offset: i32) -> Position {
    Position {
        index,
        offset,
        edge: Edge::After,
        part,
    }
}

/// Inject multiple text segments into a message.
///
/// The insertions do not need to be sorted before-hand, as this method takes care of that.
pub fn inject_into_message(message: &str, mut insertions: Vec<(Position, Segment)>) -> String {
    // Allocate exactly as much as needed, then walk through the UTF-16-encoded message,
    // applying insertions at the exact position they occur.
    let mut result = String::with_capacity(
        message.len() + insertions.iter().map(|(_, what)| what.len()).sum::<usize>(),
    );

    insertions.sort_unstable_by(|(a, _), (b, _)| b.cmp(a)); // sort in reverse so we can pop

    let mut char_buffer = [0u8; 4]; // temporary storage to re-encode chars as utf-8
    let mut prev_point = None; // temporary storage for utf-16 surrogate pairs

    for (index, point) in message.encode_utf16().enumerate() {
        loop {
            match insertions.last() {
                Some((at, what)) if at.offset as usize == index => {
                    write!(result, "{}", what).unwrap();
                    insertions.pop();
                }
                _ => break,
            }
        }

        let c = if let Some(previous) = prev_point.take() {
            char::decode_utf16([previous, point])
                .next()
                .unwrap()
                .unwrap()
        } else {
            match char::decode_utf16([point]).next().unwrap() {
                Ok(c) => c,
                Err(unpaired) => {
                    prev_point = Some(unpaired.unpaired_surrogate());
                    continue;
                }
            }
        };

        result.push_str(c.encode_utf8(&mut char_buffer));
    }

    while let Some((_, what)) = insertions.pop() {
        write!(result, "{}", what).unwrap();
    }

    result
}
