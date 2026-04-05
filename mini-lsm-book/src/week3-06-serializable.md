<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# （部分）可串行化快照隔离

现在，我们将在事务提交时添加冲突检测算法，使引擎在一定程度上具备可串行化能力。

运行测试用例：

```
cargo x copy-test --week 3 --day 6
cargo x scheck
```

让我们通过一个例子来理解可串行化。考虑引擎中有两个事务：

```
txn1: put("key1", get("key2"))
txn2: put("key2", get("key1"))
```

数据库的初始状态为 `key1=1, key2=2`。可串行化意味着执行结果与某种串行顺序下逐一执行事务的结果相同。如果先执行 txn1 再执行 txn2，得到 `key1=2, key2=2`；如果先执行 txn2 再执行 txn1，得到 `key1=1, key2=1`。

然而，使用当前实现，如果两个事务的执行有重叠：

```
txn1: get key2 <- 2
txn2: get key1 <- 1
txn1: put key1=2, commit
txn2: put key2=1, commit
```

结果将是 `key1=2, key2=1`，这无法通过任何串行执行顺序产生。这种现象称为**写偏斜（write skew）**。

通过可串行化验证，可以确保对数据库的修改对应于某种串行执行顺序，从而让用户能够在系统上运行需要可串行化执行的关键工作负载。例如，如果用户在 Mini-LSM 上运行银行转账工作负载，他们期望任何时间点的资金总额保持不变，而没有可串行化检查就无法保证这个不变量。

一种可串行化验证技术是在系统中记录每个事务的读集（read set）和写集（write set），并在提交事务之前进行验证（乐观并发控制）。如果事务的读集与在其读时间戳之后提交的任何事务的写集有重叠，则验证失败，中止该事务。

回到上面的例子，假设 txn1 和 txn2 都在时间戳 = 1 时开始：

```
txn1: get key2 <- 2
txn2: get key1 <- 1
txn1: put key1=2, commit ts = 2
txn2: put key2=1, 开始可串行化验证
```

验证 txn2 时，需要检查所有在其读时间戳之后、预期提交时间戳之前提交的事务（在本例中，1 < ts < 3）。满足条件的只有 txn1。txn1 的写集是 `key1`，txn2 的读集也是 `key1`，两者重叠，因此应中止 txn2。

## 任务 1：在 Get 和 Write Set 中跟踪读集

本任务需要修改：

```
src/mvcc/txn.rs
src/mvcc.rs
```

调用 `get` 时，应将键添加到事务的读集中。在我们的实现中，我们存储键的哈希值，以减少内存使用并加快读集的探测速度，但这可能因两个键具有相同哈希而导致误报。可以使用 `farmhash::hash32` 为键生成哈希值。注意即使 `get` 返回键不存在，该键也应被跟踪在读集中。

在 `LsmMvccInner::new_txn` 中，如果 `serializable=true`，应为事务创建空的读/写集。

## 任务 2：在 Scan 中跟踪读集

本任务需要修改：

```
src/mvcc/txn.rs
```

在本课程中，我们只对 `get` 请求保证完全可串行化。对于扫描，仍需跟踪读集，但在某些特定情况下仍可能得到不可串行化的结果。

为了理解原因，考虑以下例子：

```
txn1: put("key1", len(scan(..)))
txn2: put("key2", len(scan(..)))
```

如果数据库的初始状态为 `a=1,b=2`，应该得到 `a=1,b=2,key1=2,key2=3` 或 `a=1,b=2,key1=3,key2=2`。然而，如果事务执行如下：

```
txn1: len(scan(..)) = 2
txn2: len(scan(..)) = 2
txn1: put key1 = 2, commit, read set = {a, b}, write set = {key1}
txn2: put key2 = 2, commit, read set = {a, b}, write set = {key2}
```

这通过了我们的可串行化验证，但不对应任何串行执行顺序！因此，完整的可串行化验证需要跟踪键范围，而使用键哈希可以在只调用 `get` 时加速可串行化检查。关于如何正确实现扫描的可串行化检查，请参阅进阶任务。

## 任务 3：引擎接口与可串行化验证

本任务需要修改：

```
src/mvcc/txn.rs
src/lsm_storage.rs
```

现在可以在提交阶段实现验证。每次处理事务提交时都应持有 `commit_lock`，确保只有一个事务进入事务验证和提交阶段。

需要遍历所有提交时间戳在 `(read_ts, expected_commit_ts)` 范围内（两端均不含）的事务，检查当前事务的读集是否与满足条件的任何事务的写集有重叠。如果可以提交事务，提交写批次，并将该事务的写集插入 `self.inner.mvcc().committed_txns`（键为提交时间戳）。

如果 `write_set` 为空，可以跳过检查。只读事务可以始终提交。

还需要修改 `LsmStorageInner` 中的 `put`、`delete` 和 `write_batch` 接口。建议定义一个辅助函数 `write_batch_inner` 来处理写批次。如果 `options.serializable = true`，则 `put`、`delete` 和用户侧的 `write_batch` 应创建事务而不是直接创建写批次。写批次辅助函数还应返回 `u64` 提交时间戳，以便 `Transaction::Commit` 能正确将提交的事务数据存储到 MVCC 结构中。

## 任务 4：垃圾回收

本任务需要修改：

```
src/mvcc/txn.rs
```

提交事务时，还可以清理已提交事务映射，删除所有低于 watermark 的事务，因为它们不会参与任何未来的可串行化验证。

## 理解检验

* 如果你有构建关系型数据库的经验，可以思考以下问题：假设我们基于 Mini-LSM 构建数据库，将关系表中的每行存储为键值对（键：主键，值：序列化行），并启用可串行化验证，这个数据库系统是否直接获得了 ANSI 可串行化隔离级别能力？为什么？
* 我们这里实现的实际上是写快照隔离（参见 [A critique of snapshot isolation](https://dl.acm.org/doi/abs/10.1145/2168836.2168853)），它保证可串行化。是否存在执行是可串行化的，但会被写快照隔离验证拒绝的情况？
* 有些数据库声称通过只跟踪 get 和 scan 中访问的键（而非键范围）来支持可串行化快照隔离。它们真的能防止幻象导致的写偏斜吗？（好吧……我说的其实是 [BadgerDB](https://dgraph.io/blog/post/badger-txn/)。）

以上问题不提供参考答案，欢迎在 Discord 社区中讨论。

## 进阶任务

* **只读事务。** 启用可串行化时，需要跟踪事务的读集。
* **精确/谓词锁。** 读集可以使用范围而非单个键来维护，这在用户扫描整个键空间时非常有用，同时也能支持扫描的可串行化验证。

{{#include copyright.md}}
