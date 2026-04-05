// Copyright (c) 2022-2025 Alex Chi Z
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(unused_variables)] // TODO(you): remove this lint after implementing this mod
#![allow(dead_code)] // TODO(you): remove this lint after implementing this mod

use std::cmp::{self};
use std::collections::BinaryHeap;
use std::collections::binary_heap::PeekMut;

use anyhow::Result;

use crate::key::KeySlice;

use super::StorageIterator;

struct HeapWrapper<I: StorageIterator>(pub usize, pub Box<I>);

impl<I: StorageIterator> PartialEq for HeapWrapper<I> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == cmp::Ordering::Equal
    }
}

impl<I: StorageIterator> Eq for HeapWrapper<I> {}

impl<I: StorageIterator> PartialOrd for HeapWrapper<I> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<I: StorageIterator> Ord for HeapWrapper<I> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.1
            .key()
            .cmp(&other.1.key())
            .then(self.0.cmp(&other.0))
            .reverse()
    }
}

/// Merge multiple iterators of the same type. If the same key occurs multiple times in some
/// iterators, prefer the one with smaller index.
pub struct MergeIterator<I: StorageIterator> {
    iters: BinaryHeap<HeapWrapper<I>>,
    current: Option<HeapWrapper<I>>,
}

impl<I: StorageIterator> MergeIterator<I> {
    pub fn create(iters: Vec<Box<I>>) -> Self {
        eprintln!(
            "[create] Creating MergeIterator with {} iterators",
            iters.len()
        );
        if iters.is_empty() {
            eprintln!("[create] No iterators provided, returning empty MergeIterator");
            return Self {
                iters: BinaryHeap::new(),
                current: None,
            };
        }

        let mut heap = BinaryHeap::new();
        if iters.iter().all(|x| !x.is_valid()) {
            eprintln!("[create] All iterators are invalid, using first as current");
            let mut iters = iters;
            return Self {
                iters: heap,
                current: Some(HeapWrapper(0, iters.pop().unwrap())),
            };
        }

        eprintln!("[create] Building heap with valid iterators");
        for (i, it) in iters.into_iter().enumerate() {
            if it.is_valid() {
                eprintln!("[create]   Iterator {} is valid, adding to heap", i);
                heap.push(HeapWrapper(i, it));
            } else {
                eprintln!("[create]   Iterator {} is invalid, skipping", i);
            }
        }

        let current = heap.pop().unwrap();
        eprintln!("[create] Popped initial current iterator from heap");

        Self {
            iters: heap,
            current: Some(current),
        }
    }
}

impl<I: 'static + for<'a> StorageIterator<KeyType<'a> = KeySlice<'a>>> StorageIterator
    for MergeIterator<I>
{
    type KeyType<'a> = KeySlice<'a>;

    fn key(&self) -> KeySlice {
        let key = self.current.as_ref().unwrap().1.key();
        eprintln!("[key] Accessing key: {:?}", key);
        key
    }

    fn value(&self) -> &[u8] {
        let value = self.current.as_ref().unwrap().1.value();
        eprintln!("[value] Accessing value: {} bytes", value.len());
        value
    }

    fn is_valid(&self) -> bool {
        let valid = self
            .current
            .as_ref()
            .map(|x| x.1.is_valid())
            .unwrap_or(false);
        eprintln!("[is_valid] Checking validity: {}", valid);
        valid
    }

    fn next(&mut self) -> Result<()> {
        eprintln!("[next] Starting next() operation");
        let current = self.current.as_mut().unwrap();
        eprintln!("[next] Current key: {:?}", current.1.key());

        eprintln!("[next] Step 1: Checking and skipping duplicates in heap");
        while let Some(mut inner_iter) = self.iters.peek_mut() {
            debug_assert!(
                inner_iter.1.key() >= current.1.key(),
                "heap invariant violated"
            );
            eprintln!("[next]   Heap top key: {:?}", inner_iter.1.key());
            if inner_iter.1.key() == current.1.key() {
                eprintln!("[next]   Found duplicate key, advancing heap iterator");
                // Case 1: an error occurred when calling `next`.
                if let e @ Err(_) = inner_iter.1.next() {
                    eprintln!("[next]   Error occurred, removing invalid iterator");
                    PeekMut::pop(inner_iter);
                    return e;
                }

                // Case 2: iter is no longer valid.
                if !inner_iter.1.is_valid() {
                    eprintln!("[next]   Iterator no longer valid, removing from heap");
                    PeekMut::pop(inner_iter);
                }
            } else {
                eprintln!("[next]   Key mismatch, breaking duplicate loop");
                break;
            }
        }

        eprintln!("[next] Step 2: Advancing current iterator");
        current.1.next()?;
        eprintln!("[next]   Current advanced, valid={}", current.1.is_valid());

        if !current.1.is_valid() {
            eprintln!("[next] Step 3: Current iterator exhausted, replacing from heap");
            if let Some(iter) = self.iters.pop() {
                eprintln!("[next]   Popped new iterator from heap");
                *current = iter;
            } else {
                eprintln!("[next]   Heap empty, no more iterators");
            }

            return Ok(());
        }

        eprintln!("[next] Step 4: Current still valid, comparing with heap top");
        // Otherwise, compare with heap top and swap if necessary.
        if let Some(mut inner_iter) = self.iters.peek_mut() {
            eprintln!(
                "[next]   Current key: {:?}, heap top key: {:?}",
                current.1.key(),
                inner_iter.1.key()
            );
            if *current < *inner_iter {
                eprintln!("[next]   Swapping current with heap top");
                std::mem::swap(&mut *inner_iter, current);
            } else {
                eprintln!("[next]   No swap needed, current >= heap top");
            }
        } else {
            eprintln!("[next]   Heap is empty");
        }

        eprintln!("[next] Finished next()");
        Ok(())
    }
}
