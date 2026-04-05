<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 快照读（第二部分）——引擎读路径与事务 API

在本章中，你将：

* 完成基于上一章的读路径以支持快照读。
* 实现事务 API 以支持快照读。
* 实现引擎恢复过程以正确恢复提交时间戳。

本章结束时，你的引擎将能够向用户提供存储键空间的一致视图。

重构过程中，可能需要根据需要将某些函数的签名从 `&self` 改为 `self: &Arc<Self>`。

运行测试用例：

```
cargo x copy-test --week 3 --day 3
cargo x scheck
```

**注意：完成本章后，你还需要通过 2.5 和 2.6 的测试用例。**

## 任务 1：带读时间戳的 LSM 迭代器

本章的目标是实现如下功能：

```rust,no_run
let snapshot1 = engine.new_txn();
// 向引擎写入一些数据
let snapshot2 = engine.new_txn();
// 向引擎写入一些数据
snapshot1.get(/* ... */); // 可以检索引擎之前某个状态的一致快照
```

为此，可以在创建事务时记录读时间戳（即最新的已提交时间戳）。当对事务进行读操作时，只读取时间戳小于或等于读时间戳的所有键版本。

本任务需要修改：

```
src/lsm_iterator.rs
```

为此，需要在 `LsmIterator` 中记录读时间戳：

```rust,no_run
impl LsmIterator {
    pub(crate) fn new(
        iter: LsmIteratorInner,
        end_bound: Bound<Bytes>,
        read_ts: u64,
    ) -> Result<Self> {
        // ...
    }
}
```

需要修改 LSM 迭代器的 `next` 逻辑以找到正确的键。

## 任务 2：多版本 Scan 与 Get

本任务需要修改：

```
src/mvcc.rs
src/mvcc/txn.rs
src/lsm_storage.rs
```

现在 LSM 迭代器中有了 `read_ts`，可以在事务结构上实现 `scan` 和 `get`，以便在存储引擎的给定时间点读取数据。

建议在 `LsmStorageInner` 结构中创建辅助函数，如 `scan_with_ts(/* 原始参数 */, read_ts: u64)` 和 `get_with_ts`（如果需要）。存储引擎上原始的 get/scan 应该通过创建事务（快照）并在该事务上进行 get/scan 来实现。调用路径如下：

```
LsmStorageInner::scan -> new_txn 和 Transaction::scan -> LsmStorageInner::scan_with_ts
```

在 `LsmStorageInner::scan` 中创建事务，需要向事务构造函数提供 `Arc<LsmStorageInner>`。因此，可以将 `scan` 的签名改为接受 `self: &Arc<Self>` 而非简单的 `&self`，这样就可以用 `let txn = self.mvcc().new_txn(self.clone(), /* ... */)` 创建事务。

还需要将 `scan` 函数的返回值改为 `TxnIterator`。必须确保在用户遍历引擎时快照保持活跃，因此 `TxnIterator` 存储了快照对象。目前，`TxnIterator` 内部可以存储 `FusedIterator<LsmIterator>`，之后实现 OCC 时会再次修改它。

目前不需要实现 `Transaction::put/delete`，所有修改仍通过引擎进行。

## 任务 3：在 SST 中存储最大时间戳

本任务需要修改：

```
src/table.rs
src/table/builder.rs
```

在 SST 编码中，应在块元数据之后存储最大时间戳，并在加载 SST 时恢复它。这有助于系统在恢复时确定最新的提交时间戳。

## 任务 4：恢复提交时间戳

现在 SST 中有了最大时间戳信息，WAL 中也有了时间戳信息，可以获取引擎启动之前已提交的最大时间戳，并将其作为创建 `mvcc` 对象时的最新提交时间戳。

如果未启用 WAL，可以简单地通过找到所有 SST 中最大时间戳来计算最新提交时间戳。如果启用了 WAL，还应遍历所有已恢复的 memtable 并找到最大时间戳。

本任务需要修改：

```
src/lsm_storage.rs
```

本节没有专门的测试用例。完成本节后，你应通过之前所有章节的持久化测试（包括 2.5 和 2.6）。

## 理解检验

* 我们假设 SST 文件使用单调递增的 id 作为文件名。使用 `<level>_<begin_key>_<end_key>_<max_ts>.sst` 作为 SST 文件名是否可以？可能面临哪些潜在问题？
* 考虑事务/快照的另一种实现方式。在我们的实现中，迭代器和事务上下文中都有 `read_ts`，用户始终可以根据时间戳访问数据库某个版本的一致视图。是否可以在事务上下文中直接存储当前 LSM 状态（即所有 SST id、层信息以及所有 memtable + ts）来获得一致的快照？这样做有什么优缺点？如果引擎没有 memtable 会怎样？如果引擎运行在 S3 等分布式存储系统上会怎样？
* 假设你正在为 MVCC Mini-LSM 引擎实现备份工具。仅仅复制所有 SST 文件而不备份 LSM 状态是否足够？为什么？

以上问题不提供参考答案，欢迎在 Discord 社区中讨论。

{{#include copyright.md}}
