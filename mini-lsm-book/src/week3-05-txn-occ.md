<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 事务与乐观并发控制（Transaction and Optimistic Concurrency Control）

在本章中，你将实现 `Transaction` 的所有接口。你的实现将在事务内部维护一个私有工作区用于修改，并批量提交它们，使事务内的所有修改在提交之前只对事务自身可见。我们只在提交时检查冲突（即可串行化冲突），这就是乐观并发控制。

运行测试用例：

```
cargo x copy-test --week 3 --day 5
cargo x scheck
```

## 任务 1：本地工作区 + Put 与 Delete

本任务需要修改：

```
src/mvcc/txn.rs
```

现在可以通过将对应的键/值插入 `local_storage` 来实现 `put` 和 `delete`，`local_storage` 是一个不带键时间戳的跳表 memtable。注意对于删除操作，仍需实现为插入空值，而不是从跳表中删除值。

## 任务 2：Get 与 Scan

本任务需要修改：

```
src/mvcc/txn.rs
```

对于 `get`，应先探测本地存储。如果找到值，根据是否是删除标记返回相应值或 `None`。对于 `scan`，需要像第 1.1 章实现无键时间戳 memtable 迭代器时那样，实现一个 `TxnLocalIterator`。需要在 `TxnIterator` 中存储 `TwoMergeIterator<TxnLocalIterator, FusedIterator<LsmIterator>>`。最后，由于 `TwoMergeIterator` 会保留子迭代器中的删除标记，需要修改 `TxnIterator` 实现以正确处理删除操作。

## 任务 3：提交

本任务需要修改：

```
src/mvcc/txn.rs
```

我们假设事务只在单个线程上使用。一旦事务进入提交阶段，应将 `self.committed` 设为 true，以防用户在事务上执行任何其他操作。你的 `put`、`delete`、`scan` 和 `get` 实现应在事务已提交时返回错误。

提交实现应从本地存储中收集所有键值对，并向存储引擎提交一个写批次。

## 任务 4：原子 WAL

本任务需要修改：

```
src/wal.rs
src/mem_table.rs
src/lsm_storage.rs
```

注意 `commit` 涉及产生写批次，而目前写批次不保证原子性。需要修改 WAL 实现，为写批次添加头部和尾部。

新的 WAL 编码格式如下：

```
|   HEADER   |                          BODY                                      |  FOOTER  |
|     u32    |   u16   | var | u64 |    u16    |  var  |           ...            |    u32   |
| batch_size | key_len | key | ts  | value_len | value | more key-value pairs ... | checksum |
```

`batch_size` 是 `BODY` 部分的大小，`checksum` 是 `BODY` 部分的校验和。

没有专门的测试用例验证你的实现。只要通过所有现有测试用例并实现上述 WAL 格式，就算完成。

你应实现 `Wal::put_batch` 和 `MemTable::put_batch`。原有的 `put` 函数应将单个键值对视为一个批次，即此时你的 `put` 函数应调用 `put_batch`。

即使一个批次超过了 memtable 的大小限制，也应在同一个 memtable 和同一个 WAL 中处理。

## 理解检验

* 到目前为止实现的所有内容，系统是否满足快照隔离？如果不满足，还需要做什么来支持快照隔离？（注意：快照隔离与下一章将讨论的可串行化快照隔离不同）
* 如果用户想批量导入数据（如 1TB）并使用事务 API 进行，你有什么建议？是否有机会针对这种情况进行优化？
* 什么是乐观并发控制？如果在 Mini-LSM 中改为实现悲观并发控制，系统会是什么样子？
* 如果系统崩溃并在磁盘上留下损坏的 WAL，如何处理这种情况？
* 提交事务时，是否有必要批量将所有内容放入 memtable，还是可以逐个键放入？为什么？

## 进阶任务

* **溢出到磁盘。** 如果事务的私有工作区变得太大，可以将部分数据刷写到磁盘。

{{#include copyright.md}}
