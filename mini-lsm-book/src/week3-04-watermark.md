<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 水位线与垃圾回收（Watermark and Garbage Collection）

在本章中，你将实现跟踪用户正在使用的最低读时间戳的必要结构，并在压缩时从 SST 中清理未使用的版本。

运行测试用例：

```
cargo x copy-test --week 3 --day 4
cargo x scheck
```

## 任务 1：实现水位线（Watermark）

本任务需要修改：

```
src/mvcc/watermark.rs
```

Watermark 是跟踪系统中最低 `read_ts` 的结构。创建新事务时，应调用 `add_reader` 添加其读时间戳以进行跟踪。事务中止或提交时，应从 watermark 中移除自身。调用 `watermark()` 时，watermark 结构返回系统中最低的 `read_ts`。如果没有正在进行的事务，简单地返回 `None`。

可以使用 `BTreeMap` 实现 watermark。它为每个 `read_ts` 维护一个计数器，记录有多少快照正在使用该读时间戳。b-tree map 中不应存在计数为 0 的条目。

## 任务 2：在事务中维护水位线

本任务需要修改：

```
src/mvcc/txn.rs
src/mvcc.rs
```

需要在事务开始时将 `read_ts` 添加到 watermark，并在事务的 `drop` 被调用时移除它。

## 任务 3：压缩中的垃圾回收

本任务需要修改：

```
src/compact.rs
```

现在系统中有了 watermark，可以在压缩过程中清理未使用的版本。

* 如果键的某个版本高于 watermark，保留它。
* 对于 watermark 以下（含）的所有键版本，只保留最新版本。

例如，如果 watermark=3，有如下数据：

```
a@4=del <- 高于 watermark
a@3=3   <- watermark 以下（含）的最新版本
a@2=2   <- 可以删除，不会被任何人读取
a@1=1   <- 可以删除，不会被任何人读取
b@1=1   <- watermark 以下（含）的最新版本
c@4=4   <- 高于 watermark
d@3=del <- 如果压缩到最底层可以删除
d@2=2   <- 可以删除
```

对这些键进行压缩后，将得到：

```
a@4=del
a@3=3
b@1=1
c@4=4
d@3=del（如果压缩到最底层可以删除）
```

假设这是引擎中的所有键。在 ts=3 时进行扫描，压缩前后都会得到 `a=3,b=1,c=4`。在 ts=4 时进行扫描，压缩前后都会得到 `b=1,c=4`。压缩**不会**也**不应该**影响读时间戳 >= watermark 的事务。

## 理解检验

* 在我们的实现中，我们通过 `Transaction` 的生命周期自己管理水位线（称为非托管模式）。如果用户想自己管理键的时间戳和水位线（即他们有自己的时间戳生成器），在 write_batch/get/scan API 中需要做什么来验证他们的请求？我们有哪些可能难以在这种情况下维护的架构假设？
* 为什么需要在事务迭代器中存储 `Arc<Transaction>`？
* 从 SST 文件中完全删除一个键的条件是什么？
* 目前，只有在压缩到最底层时才删除键。是否还有其他更早的时机可以删除键？（提示：你知道所有层中每个 SST 的起始/结束键。）
* 考虑用户创建了一个长时间运行的事务，导致无法进行垃圾回收。用户不断更新同一个键，最终一个 SST 文件中可能会有该键的数千个版本。这会如何影响性能？你会如何处理这个问题？

## 进阶任务

* **O(1) 水位线。** 可以使用哈希映射或循环队列实现均摊 O(1) 的水位线结构。

{{#include copyright.md}}
