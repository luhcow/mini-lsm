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

use anyhow::{Result, bail};

use crate::{
    iterators::{StorageIterator, merge_iterator::MergeIterator},
    mem_table::MemTableIterator,
};

/// Represents the internal type for an LSM iterator. This type will be changed across the course for multiple times.
type LsmIteratorInner = MergeIterator<MemTableIterator>;

pub struct LsmIterator {
    inner: LsmIteratorInner,
}

impl LsmIterator {
    pub(crate) fn new(iter: LsmIteratorInner) -> Result<Self> {
        let mut iter = Self { inner: iter };
        while iter.is_valid() && iter.value().is_empty() {
            iter.next()?;
        }
        Ok(iter)
    }
}

impl StorageIterator for LsmIterator {
    type KeyType<'a> = &'a [u8];

    fn is_valid(&self) -> bool {
        let valid = self.inner.is_valid();
        eprintln!("[LsmIterator] is_valid: {}", valid);
        valid
    }

    fn key(&self) -> &[u8] {
        let key = self.inner.key().raw_ref();
        eprintln!("[LsmIterator] key: {:?}", String::from_utf8_lossy(key));
        key
    }

    fn value(&self) -> &[u8] {
        let value = self.inner.value();
        eprintln!("[LsmIterator] value: {} bytes", value.len());
        value
    }

    fn next(&mut self) -> Result<()> {
        eprintln!("[LsmIterator] next() called");
        self.inner.next()?;
        eprintln!("[LsmIterator] inner.next() completed");
        let mut skip_count = 0;
        while self.is_valid() && self.value().is_empty() {
            eprintln!(
                "[LsmIterator] value is empty, skipping (count: {})",
                skip_count
            );
            self.inner.next()?;
            skip_count += 1;
        }
        if skip_count > 0 {
            eprintln!("[LsmIterator] skipped {} empty values", skip_count);
        }
        eprintln!(
            "[LsmIterator] next() finished, current key: {:?}",
            String::from_utf8_lossy(self.key())
        );
        Ok(())
    }
}

/// A wrapper around existing iterator, will prevent users from calling `next` when the iterator is
/// invalid. If an iterator is already invalid, `next` does not do anything. If `next` returns an error,
/// `is_valid` should return false, and `next` should always return an error.
pub struct FusedIterator<I: StorageIterator> {
    iter: I,
    has_errored: bool,
}

impl<I: StorageIterator> FusedIterator<I> {
    pub fn new(iter: I) -> Self {
        eprintln!("[FusedIterator] Creating new FusedIterator");
        Self {
            iter,
            has_errored: false,
        }
    }
}

impl<I: StorageIterator> StorageIterator for FusedIterator<I> {
    type KeyType<'a>
        = I::KeyType<'a>
    where
        Self: 'a;

    fn is_valid(&self) -> bool {
        let valid = !self.has_errored && self.iter.is_valid();
        eprintln!(
            "[FusedIterator] is_valid: {} (errored: {})",
            valid, self.has_errored
        );
        valid
    }

    fn key(&self) -> Self::KeyType<'_> {
        eprintln!("[FusedIterator] key access");
        self.iter.key()
    }

    fn value(&self) -> &[u8] {
        eprintln!("[FusedIterator] value access");
        self.iter.value()
    }

    fn next(&mut self) -> Result<()> {
        eprintln!("[FusedIterator] next() called");
        // only move when the iterator is valid and not errored
        if self.has_errored {
            eprintln!("[FusedIterator] Iterator is tainted, returning error");
            bail!("the iterator is tainted");
        }
        if self.iter.is_valid() {
            eprintln!("[FusedIterator] Iterator is valid, advancing");
            if let Err(e) = self.iter.next() {
                eprintln!("[FusedIterator] Error occurred during next(), marking as tainted");
                self.has_errored = true;
                return Err(e);
            }
            eprintln!("[FusedIterator] Successfully advanced");
        } else {
            eprintln!("[FusedIterator] Iterator is invalid, skipping next()");
        }
        Ok(())
    }
}
