<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 第 3 周概览：多版本并发控制（MVCC）

在本部分中，你将在前两周构建的 LSM 引擎之上实现 MVCC。我们将在键中添加时间戳编码以维护键的多个版本，并修改引擎的某些部分，确保旧数据根据是否有用户在读取旧版本来决定保留或垃圾回收。

本课程 MVCC 部分的总体方法受 [BadgerDB](https://github.com/dgraph-io/badger) 的启发，并部分基于它。

MVCC 的关键是在存储引擎中存储和访问键的多个版本。因此，需要将键格式改为 `user_key + timestamp (u64)`。在用户接口侧，需要提供新的 API 帮助用户访问历史版本。总的来说，我们将为键添加一个单调递增的时间戳。

在之前的部分中，我们假设较新的键在 LSM 树的上层，较旧的键在下层。在压缩过程中，如果在多个层中找到同一键的多个版本，只保留最新版本，压缩过程通过只合并相邻层/tier 来确保较新的键保留在上层。在 MVCC 实现中，时间戳较大的键是最新的键。在压缩过程中，只有当没有用户在访问数据库的旧版本时才能删除键。尽管不在上层保留键的最新版本在 MVCC LSM 实现中仍能得到正确结果，但在本课程中我们选择保持这个不变量：如果一个键有多个版本，较新的版本总是出现在上层。

一般来说，有两种使用支持 MVCC 的存储引擎的方式。如果用户将引擎作为独立组件使用，不想手动分配键的时间戳，他们会使用事务 API 存储和检索数据，时间戳对用户透明。另一种方式是将存储引擎集成到系统中，用户自己管理时间戳。用 BadgerDB 的术语来描述这两种方式：隐藏时间戳的方式称为*非托管模式（un-managed mode）*，给用户完全控制的方式称为*托管模式（managed mode）*。

**托管模式 API**
```
get(key, read_timestamp) -> (value, write_timestamp)
scan(key_range, read_timestamp) -> iterator<key, value, write_timestamp>
put/delete/write_batch(key, timestamp)
set_watermark(timestamp) # 我们很快会讲到水位线！
```

**非托管/普通模式 API**
```
get(key) -> value
scan(key_range) -> iterator<key, value>
start_transaction() -> txn
txn.put/delete/write_batch(key, timestamp)
```

如你所见，托管模式 API 要求用户在操作时提供时间戳，时间戳可能来自某个集中式时间戳系统，或来自其他系统的日志（如 Postgres 逻辑复制日志）。用户需要指定水位线，即引擎可以删除该版本以下内容的阈值。

而对于非托管 API，与之前实现的接口相同，只是用户需要通过创建事务来写入和读取数据。当用户创建事务时，可以获得数据库的一致状态（即快照）。即使其他线程/事务向数据库写入数据，这些数据对进行中的事务也是不可见的。存储引擎在内部管理时间戳，不向用户暴露。

本周，我们将先用 3 天时间对表格式和 memtable 进行重构，将键格式改为键切片加时间戳。之后，我们将实现必要的 API 来提供一致的快照和事务。

本部分共有 7 章（天）：

* [第 1 天：时间戳键重构](./week3-01-ts-key-refactor.md)。你将把 `key` 模块替换为 MVCC 版本，并重构系统以使用带时间戳的键。
* [第 2 天：快照读——Memtable 与时间戳](./week3-02-snapshot-read-part-1.md)。你将重构 memtable 和写路径以支持多版本读写。
* [第 3 天：快照读——事务 API](./week3-03-snapshot-read-part-2.md)。你将实现事务 API 并完成读写路径的其余部分，以支持快照读。
* [第 4 天：水位线与垃圾回收](./week3-04-watermark.md)。你将实现水位线计算算法，并在压缩时实现垃圾回收以删除旧版本。
* [第 5 天：事务与乐观并发控制](./week3-05-txn-occ.md)。你将为所有事务创建私有工作区，并批量提交它们，使事务的修改对其他事务不可见，直到提交。
* [第 6 天：可串行化快照隔离](./week3-06-serializable.md)。你将实现 OCC 可串行化检查，确保对数据库的修改是可串行化的，并中止违反可串行化的事务。
* [第 7 天：压缩过滤器](./week3-07-compaction-filter.md)。本周结束时，我们将压缩时的垃圾回收逻辑泛化为压缩过滤器，根据用户需求在压缩时删除数据。

{{#include copyright.md}}
