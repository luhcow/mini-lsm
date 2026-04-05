# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 协作约定

- **始终用中文回复**，包括代码注释之外的所有解释、分析和建议。
- 这是一个**教学项目**，用户正在学习 LSM 存储引擎的实现原理。解释问题时优先说明"为什么"，而不只是"怎么做"。遇到 bug 时引导用户理解根因，而不是直接给出答案。

## Commands

```bash
# Build
cargo build -p mini-lsm-starter

# Run all tests for the starter crate
cargo test -p mini-lsm-starter

# Run tests for a specific week/day
cargo test -p mini-lsm-starter week1_day2

# Run a single test by name
cargo test -p mini-lsm-starter week1_day2::test_task4_integration

# Lint
cargo clippy -p mini-lsm-starter

# Format
cargo fmt -p mini-lsm-starter

# Copy test cases for a specific week/day into starter
cargo x copy-test --week 1 --day 2

# Full CI check (check, test, clippy, fmt, build book)
cargo x ci
```

Tests are organized under `mini-lsm-starter/src/tests/week{N}_day{N}.rs`. Each day's tests are named `test_task{N}_*`.

## Architecture

This is an educational LSM (Log-Structured Merge-tree) storage engine course. The student implementation lives in **`mini-lsm-starter/`**, with reference implementations in `mini-lsm/` (weeks 1–2) and `mini-lsm-mvcc/` (week 3).

### Layer Stack (bottom-up)

**Block** (`block.rs`) → **SST** (`table.rs`) → **Storage State** (`lsm_storage.rs`)

1. **Block** — smallest unit of disk I/O. A `BlockBuilder` encodes sorted key-value pairs; a `BlockIterator` reads them back. Blocks are cached via `BlockCache` (Moka LRU keyed by `(sst_id, block_idx)`).

2. **SST (Sorted String Table)** (`table.rs`) — immutable on-disk file containing multiple blocks. `BlockMeta` stores each block's byte offset, first key, and last key. Built with `SsTableBuilder`, read with `SsTableIterator`. Optional Bloom filter and prefix key compression live in `table/bloom.rs`.

3. **MemTable** (`mem_table.rs`) — in-memory sorted store backed by a crossbeam `SkipMap`. Deletes are tombstones (empty value). Has an optional WAL (`wal.rs`) for durability. Flushed to an L0 SST when it exceeds the size threshold.

4. **LSM Storage State** (`lsm_storage.rs`) — the authoritative snapshot:
   - `memtable` — active write target
   - `imm_memtables` — frozen, awaiting flush; index 0 is the most recently frozen (highest priority for reads)
   - `l0_sstables` — recently flushed, uncompacted SSTs
   - `levels` — compacted SSTs organized by level/tier
   - `sstables: HashMap<usize, Arc<SsTable>>` — all open SST objects

   `LsmStorageState` is wrapped in `Arc<RwLock<...>>` and replaced atomically on every mutation.

5. **Iterators** (`iterators/`) — unified via the `StorageIterator` trait:
   - `MergeIterator` — k-way merge with a BinaryHeap (lower index = higher priority for same key)
   - `TwoMergeIterator` — optimized two-way version
   - `ConcatIterator` — chains non-overlapping SST iterators within a level
   - `LsmIterator` — top-level iterator; wraps a `MergeIterator<MemTableIterator>` and **skips tombstones** (empty values). Must skip tombstones both on construction and in `next()`.
   - `FusedIterator` — guards against calling `next()` on an exhausted or errored iterator

6. **Compaction** (`compact.rs`) — three pluggable strategies: `SimpleLeveled`, `Tiered`, `Leveled`. A background thread calls `force_compaction()` which selects a `CompactionTask`, merges SSTs via `ConcatIterator`/`MergeIterator`, then applies the result atomically to storage state.

7. **Manifest** (`manifest.rs`) — append-only log recording flush and compaction events for crash recovery.

8. **MVCC** (`mvcc/`) — week 3 only. Keys gain a `u64` timestamp. `Transaction` implements optimistic concurrency control (OCC). `Watermark` tracks the oldest active read timestamp for GC.

### Read Path

`scan()` / `get()` merges iterators in priority order:
1. Current `memtable`
2. `imm_memtables` (index 0 first)
3. L0 SSTs (newest first, each via `SsTableIterator`)
4. L1+ levels (each via `ConcatIterator`)

### Write Path

`put()` / `delete()` → `memtable.put()` (+ optional WAL) → when size exceeds threshold, freeze into `imm_memtables` → background flush to L0 SST → background compaction into deeper levels.

### Key Encoding

`Key<T>` in `key.rs` is a generic wrapper. In weeks 1–2, it's just raw bytes. In week 3, it gains a timestamp suffix. `KeySlice` / `KeyVec` / `KeyBytes` are type aliases. `map_bound()` in `mem_table.rs` converts `Bound<&[u8]>` to `Bound<Bytes>` for SkipMap range queries.
