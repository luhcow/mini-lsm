<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 写路径（Write Path）

![Chapter Overview](./lsm-tutorial/week1-05-overview.svg)

在本章中，你将：

* 实现带 L0 刷写的 LSM 写路径。
* 实现正确更新 LSM 状态的逻辑。

将测试用例复制到 starter 代码并运行：

```
cargo x copy-test --week 1 --day 6
cargo x scheck
```

## 任务 1：将 Memtable 刷写到 SST

至此，所有内存结构和磁盘文件都已准备好，存储引擎能够从所有这些结构中读取并合并数据。现在我们来实现将内存数据移动到磁盘的逻辑（即刷写，flush），完成 Mini-LSM 第 1 周的课程。

本任务需要修改：

```
src/lsm_storage.rs
src/mem_table.rs
```

你需要修改 `LSMStorageInner::force_flush_next_imm_memtable` 和 `MemTable::flush`。在 `LSMStorageInner::open` 中，如果 LSM 数据库目录不存在，需要创建它。将 memtable 刷写到磁盘需要做三件事：

* 选择一个 memtable 进行刷写。
* 创建对应 memtable 的 SST 文件。
* 从不可变 memtable 列表中移除该 memtable，并将 SST 文件添加到 L0 SST 中。

目前还没有解释 L0（level-0）SST 是什么。一般来说，L0 SST 是直接由 memtable 刷写产生的 SST 文件集合。在本课程第 1 周，磁盘上只有 L0 SST。第 2 周会深入研究如何用分层（leveled）或分级（tiered）结构高效组织它们。

注意，创建 SST 文件是计算密集型且耗时的操作。同样，我们不希望长时间持有 `state` 读/写锁，因为这会阻塞其他操作并在 LSM 操作中产生巨大的延迟尖峰。我们还使用 `state_lock` 互斥锁来序列化 LSM 树中的状态修改操作。在本任务中，你需要仔细考虑如何使用这些锁，在最小化临界区的同时使 LSM 状态修改不存在竞态条件。

我们没有并发测试用例，你需要仔细考虑你的实现。另外，记住不可变 memtable 列表中最后一个 memtable 是最旧的，也是你应该刷写的那个。

<details>

<summary>提示：刷写 L0 的伪代码</summary>

```rust,no_run
fn flush_l0(&self) {
    let _state_lock = self.state_lock.lock();

    let memtable_to_flush;
    let snapshot = {
        let guard = self.state.read();
        memtable_to_flush = guard.imm_memtables.last();
    };

    let sst = memtable_to_flush.flush()?;

    {
        let guard = self.state.write();
        guard.imm_memtables.pop();
        guard.l0_sstables.insert(0, sst);
    };

}
```

</details>

## 任务 2：刷写触发器

本任务需要修改：

```
src/lsm_storage.rs
src/compact.rs
```

当内存中的 memtable 数量（不可变 + 可变）超过 LSM 存储选项中的 `num_memtable_limit` 时，应将最旧的 memtable 刷写到磁盘。这由后台的刷写线程完成。刷写线程随 `MiniLSM` 结构一起启动，我们已经实现了启动和正确停止线程所需的代码。

在本任务中，你需要实现 `compact.rs` 中的 `LsmStorageInner::trigger_flush`，以及 `lsm_storage.rs` 中的 `MiniLsm::close`。`trigger_flush` 每 50 毫秒执行一次。如果 memtable 数量超过限制，应调用 `force_flush_next_imm_memtable` 来刷写一个 memtable。用户调用 `close` 函数时，应等待刷写线程（以及第 2 周的压缩线程）完成。

## 任务 3：过滤 SST

现在你有了一个完整可用的存储引擎，可以使用 mini-lsm-cli 与它交互：

```shell
cargo run --bin mini-lsm-cli -- --compaction none
```

然后：

```
fill 1000 3000
get 2333
flush
fill 1000 3000
get 2333
flush
get 2333
scan 2000 2333
```

如果写入更多数据，你可以看到刷写线程自动刷写 L0 SST，而无需使用 `flush` 命令。

最后，在结束本周之前，我们来实现一个简单的优化：过滤 SST。根据用户提供的键范围，我们可以轻松过滤掉不包含该键范围的 SST，从而无需在合并迭代器中读取它们。

本任务需要修改：

```
src/lsm_storage.rs
src/iterators/*
src/lsm_iterator.rs
```

你需要修改读路径函数，跳过不可能包含该键/键范围的 SST。你需要为迭代器实现 `num_active_iterators`，以便测试用例可以检查你的实现是否正确。对于 `MergeIterator` 和 `TwoMergeIterator`，它是所有子迭代器的 `num_active_iterators` 之和。注意，如果你没有修改 starter 代码中 `MergeIterator` 的字段，记得也将 `MergeIterator::current` 计入其中。对于 `LsmIterator` 和 `FusedIterator`，直接返回内部迭代器的活跃迭代器数量即可。

你可以实现 `range_overlap` 和 `key_within` 等辅助函数来简化代码。

## 理解检验

* 如果用户请求删除一个键两次，会发生什么？
* 初始化迭代器时，同时会有多少内存（或多少块）被加载到内存中？
* 一些追求极致的用户想要*分叉*他们的 LSM 树。他们希望启动引擎写入一些数据，然后分叉它，从而获得两个相同的数据集并分别操作。一个简单但低效的实现方式是直接将所有 SST 和内存结构复制到新目录并启动引擎。然而，由于我们从不修改磁盘上的文件，实际上可以复用父引擎的 SST 文件。你认为如何高效实现这个分叉功能而无需复制数据？（参考 [Neon Branching](https://neon.tech/docs/introduction/branching)）
* 假设你在构建一个多租户 LSM 系统，在单台 128GB 内存的机器上托管 10k 个数据库，memtable 大小限制设为 256MB。这种配置需要多少内存用于 memtable？
  * 显然，内存不够用。假设每个用户仍有自己的 memtable，如何设计 memtable 刷写策略使其可行？让所有用户共享同一个 memtable（通过在键前缀中编码租户 ID）是否有意义？

以上问题不提供参考答案，欢迎在 Discord 社区中讨论。

## 进阶任务

* **实现写/L0 限速。** 当 memtable 数量超过上限太多时，可以阻止用户向存储引擎写入。第 2 周实现压缩后，也可以为 L0 表实现写限速。
* **前缀扫描。** 通过实现前缀扫描接口并利用前缀信息，可以过滤更多 SST。

{{#include copyright.md}}
