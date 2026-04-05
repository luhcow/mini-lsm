<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 时间戳键编码与重构（Timestamp Key Encoding + Refactor）

在本章中，你将：

* 将实现重构为使用 key+ts 的表示方式。
* 让代码在新的键表示下能够编译通过。

运行测试用例：

```
cargo x copy-test --week 3 --day 1
cargo x scheck
```

**注意：MVCC 子系统在第 3 周第 2 天之前尚未完全实现。本天结束时，你只需通过第 3 周第 1 天的测试以及所有第 1 周的测试。由于涉及压缩，第 2 周的测试暂时无法通过。**

## 任务 0：使用 MVCC 键编码

你需要将键编码模块替换为 MVCC 版本。我们从原始键模块中删除了一些接口，并为键实现了新的比较器。如果你在之前章节中按照指引没有对键使用 `into_inner`，在完成第 3 天的所有重构后应该能通过所有测试。否则，你需要仔细检查那些只比较键而不查看时间戳的地方。

具体来说，键类型定义从：

```rust,no_run
pub struct Key<T: AsRef<[u8]>>(T);
```

变更为：

```rust,no_run
pub struct Key<T: AsRef<[u8]>>(T /* user key */, u64 /* timestamp */);
```

……键现在关联了一个时间戳。我们只在系统内部使用这种键表示，在用户接口侧，不要求用户提供时间戳，因此一些结构在引擎中仍使用 `&[u8]` 而非 `KeySlice`。我们稍后会介绍需要修改函数签名的地方。现在只需运行：

```
cp mini-lsm-mvcc/src/key.rs mini-lsm-starter/src/
```

还有其他存储时间戳的方式。例如，仍可以使用 `pub struct Key<T: AsRef<[u8]>>(T);` 表示，但假设键的最后 8 字节是时间戳。你也可以将此作为进阶任务实现。

```plaintext
替代键表示：| user_key (varlen) | ts (8 bytes) | 在单个切片中
我们的键表示：| user_key slice | ts (u64) |
```

在 key+ts 编码中，用户键最小且时间戳最大的键排在最前面。例如：

```
("a", 233) < ("a", 0) < ("b", 233) < ("b", 0)
```

## 任务 1：在块中编码时间戳

替换键模块后，你首先会注意到代码可能无法编译。在本章中，你需要做的就是让它能编译。本任务需要修改：

```
src/block.rs
src/block/builder.rs
src/block/iterator.rs
```

你会注意到键 API 中删除了 `raw_ref()` 和 `len()`。取而代之的是 `key_ref` 获取用户键的切片，以及 `key_len` 获取用户键的长度。你需要重构块 builder 和解码实现以使用新的 API。同时，需要修改块编码以编码时间戳。在 `BlockBuilder::add` 中执行此操作。新的块条目记录格式如下：

```
key_overlap_len (u16) | remaining_key_len (u16) | key (remaining_key_len) | timestamp (u64)
```

可以使用 `raw_len` 估算键所需的空间，并在用户键之后存储时间戳。

修改块编码后，需要相应地修改 `block.rs` 和 `iterator.rs` 中的解码逻辑。

## 任务 2：在 SST 中编码时间戳

然后，修改表格式：

```
src/table.rs
src/table/builder.rs
src/table/iterator.rs
```

具体来说，需要修改块元数据编码以包含键的时间戳，其他代码保持不变。由于所有函数签名（如 seek、add）都使用 `KeySlice`，新的键比较器应自动按用户键和时间戳对键进行排序。

在表 builder 中，可以直接使用 `key_ref()` 构建布隆过滤器，这自然为 SST 创建了前缀布隆过滤器。

## 任务 3：LSM 迭代器

由于我们使用关联泛型类型（GAT）使大多数迭代器能支持不同的键类型（如 `&[u8]` 和 `KeySlice<'_>`），如果实现正确，无需修改合并迭代器和拼接迭代器。`LsmIterator` 是从内部键表示中去除时间戳并向用户返回最新版本键的地方。本任务需要修改：

```
src/lsm_iterator.rs
```

目前，我们不修改 `LsmIterator` 只保留键最新版本的逻辑。我们只是通过在向内部迭代器传递键时追加时间戳，并在向用户返回时去除时间戳，使其能编译通过。目前 LSM 迭代器的行为是向用户返回同一键的多个版本。

## 任务 4：Memtable

目前保持 memtable 的逻辑不变。我们向用户返回键切片，并使用 `TS_DEFAULT` 刷写 SST。下一章将把 memtable 改为支持 MVCC。本任务需要修改：

```
src/mem_table.rs
```

## 任务 5：引擎读路径

本任务需要修改：

```
src/lsm_storage.rs
```

现在键中有了时间戳，创建迭代器时需要使用带时间戳的键而不只是用户键进行 seek。可以使用 `TS_RANGE_BEGIN`（即最大时间戳）创建键切片。

检查某个用户键是否在某个表中时，只需比较用户键，无需比较时间戳。

此时，你应该能构建实现并通过所有第 1 周测试用例。系统中存储的所有键都使用 `TS_DEFAULT`（即时间戳 0）。我们将在接下来两章中让引擎完全支持多版本并通过所有测试用例。

{{#include copyright.md}}
