<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 快照读（第一部分）——Memtable 与时间戳

在本章中，你将：

* 重构 memtable/WAL 以存储键的多个版本。
* 实现新的引擎写路径，为每个键分配时间戳。
* 让压缩过程感知多版本键。
* 实现新的引擎读路径，返回键的最新版本。

重构过程中，可能需要根据需要将某些函数的签名从 `&self` 改为 `self: &Arc<Self>`。

运行测试用例：

```
cargo x copy-test --week 3 --day 2
cargo x scheck
```

**注意：完成本章后，你还需要通过 2.4 及之前所有测试用例。**

## 任务 1：MemTable、预写日志与读路径

本任务需要修改：

```
src/wal.rs
src/mem_table.rs
src/lsm_storage.rs
```

我们已经将引擎中大多数键改为 `KeySlice`（包含字节键和时间戳），但系统的某些部分仍未考虑时间戳。在第一个任务中，你需要修改 memtable 和 WAL 实现以考虑时间戳。

首先需要修改 memtable 中 `SkipMap` 的类型：

```rust,no_run
pub struct MemTable {
    // map: Arc<SkipMap<Bytes, Bytes>>,
    map: Arc<SkipMap<KeyBytes, Bytes>>, // Bytes -> KeyBytes
    // ...
}
```

之后，修复所有编译错误以完成本任务。

**MemTable::get**

保留 get 接口是为了让测试用例仍能在 memtable 中探测特定版本的键。完成本任务后，读路径中不应再使用此接口。鉴于跳表中存储的是 `KeyBytes`（即 `(Bytes, u64)`），而用户探测的是 `KeySlice`（即 `(&[u8], u64)`），我们需要找到一种方式将后者转换为前者的引用，以便在跳表中检索数据。

为此，可以使用 unsafe 代码强制将 `&[u8]` 转换为 static，并使用 `Bytes::from_static` 从 static 切片创建 bytes 对象。这是合理的，因为 `Bytes` 会假定切片是 static 的，不会尝试释放切片的内存。

<details>

<summary>提示：将 u8 切片转换为 Bytes</summary>

```rust,no_run
Bytes::from_static(unsafe { std::mem::transmute(key.key_ref()) })
```

</details>

之前没有这个问题是因为我们使用 `Bytes` 和 `&[u8]`，而 `Bytes` 实现了 `Borrow<[u8]>`。

**MemTable::put**

签名应改为 `fn put(&self, key: KeySlice, value: &[u8])`，你需要在实现中将键切片转换为 `KeyBytes`。

**MemTable::scan**

签名应改为 `fn scan(&self, lower: Bound<KeySlice>, upper: Bound<KeySlice>) -> MemTableIterator`。你需要将 `KeySlice` 转换为 `KeyBytes`，并将它们用作 `SkipMap::range` 的参数。

**MemTable::flush**

现在应使用键的时间戳而不是默认时间戳来刷写 memtable 到 SST。

**MemTableIterator**

现在应存储 `(KeyBytes, Bytes)`，返回键类型应为 `KeySlice`。

**Wal::recover 和 Wal::put**

预写日志现在应接受键切片而非用户键切片。序列化和反序列化 WAL 记录时，应将时间戳放入 WAL 文件，并对时间戳和之前所有字段一起计算校验和。

WAL 格式如下：

```
| key_len（不含 ts 长度）(u16) | key | ts (u64) | value_len (u16) | value | checksum (u32) |
```

**LsmStorageInner::get**

之前 `get` 的实现是先探测 memtable，然后扫描 SST。现在 memtable 改用了新的 key-ts API，需要重新实现 `get` 接口。最简单的方式是对所有内容（memtable、不可变 memtable、L0 SST 以及其他层 SST）创建合并迭代器，就像 `scan` 中做的那样，只是对 SST 进行了布隆过滤器过滤。

**LsmStorageInner::scan**

需要使用新的 memtable API，扫描范围设为 `(user_key_begin, TS_RANGE_BEGIN)` 和 `(user_key_end, TS_RANGE_END)`。注意处理排除边界时，需要将迭代器正确定位到下一个键（而不是相同时间戳的当前键）。

## 任务 2：写路径

本任务需要修改：

```
src/lsm_storage.rs
```

`LsmStorageInner` 中有一个 `mvcc` 字段，包含本周 MVCC 需要使用的所有数据结构。打开目录并初始化存储引擎时，需要创建该结构。

在 `write_batch` 实现中，需要为写批次中的所有键获取提交时间戳。可以在逻辑开始时使用 `self.mvcc().latest_commit_ts() + 1` 获取时间戳，在逻辑结束时使用 `self.mvcc().update_commit_ts(ts)` 递增下一个提交时间戳。为确保所有写批次有不同的时间戳且新键位于旧键之上，需要在函数开始时持有写锁 `self.mvcc().write_lock.lock()`，以确保同一时间只有一个线程能写入存储引擎。

## 任务 3：MVCC 压缩

本任务需要修改：

```
src/compact.rs
```

之前的做法是：只保留键的最新版本，当键被压缩到最底层且被删除时，移除该键。有了 MVCC，键现在关联了时间戳，不能再使用相同的压缩逻辑。

本章中，可以简单地移除删除键的逻辑，暂时忽略 `compact_to_bottom_level`，在压缩时保留**所有版本**的键。

此外，需要以这样一种方式实现压缩算法：即使超过 SST 大小限制，相同键的不同时间戳版本也应放在同一个 SST 文件中。这确保如果某层的一个 SST 中找到了某个键，该键就不会出现在该层的其他 SST 文件中，从而简化系统许多部分的实现。

## 任务 4：LSM 迭代器

本任务需要修改：

```
src/lsm_iterator.rs
```

上一章中，LSM 迭代器被实现为将同一键的不同时间戳视为不同键。现在需要重构 LSM 迭代器，使其在从子迭代器获取到多个版本时只返回键的最新版本。

需要在迭代器中记录 `prev_key`。如果已经向用户返回了键的最新版本，可以跳过所有旧版本并继续到下一个键。

此时，你应该通过之前所有章节的测试用例（持久化测试 2.5 和 2.6 除外）。

## 理解检验

* MVCC 引擎中的 `get` 与你在第 2 周构建的引擎中的 `get` 有什么区别？
* 在第 2 周，`get` 时在第一个找到键的 memtable/层就停止。在 MVCC 版本中可以这样做吗？
* 如何将 `KeySlice` 转换为 `&KeyBytes`？这是安全/合理的操作吗？
* 为什么需要在写路径中持有写锁？

以上问题不提供参考答案，欢迎在 Discord 社区中讨论。

## 进阶任务

* **Memtable Get 提前终止。** 不必对所有 memtable 和 SST 创建合并迭代器，可以这样实现 `get`：如果在 memtable 中找到键的某个版本，就停止查找。对 SST 同样适用。

{{#include copyright.md}}
