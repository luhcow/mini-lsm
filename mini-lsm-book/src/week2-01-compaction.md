<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 压缩实现（Compaction Implementation）

![Chapter Overview](./lsm-tutorial/week2-01-full.svg)

在本章中，你将：

* 实现压缩逻辑，将一些文件合并并生成新文件。
* 实现更新 LSM 状态和管理文件系统上 SST 文件的逻辑。
* 更新 LSM 读路径以支持 LSM 层级结构。

将测试用例复制到 starter 代码并运行：

```
cargo x copy-test --week 2 --day 1
cargo x scheck
```

<div class="warning">

在阅读本章之前，建议先查看[第 2 周概览](./week2-overview.md)，以全面了解压缩的概念。

</div>

## 任务 1：压缩实现

本任务需要实现压缩的核心逻辑——将一组 SST 文件归并排序为一个有序运行（sorted run）。你需要修改：

```
src/compact.rs
```

具体来说，需要实现 `force_full_compaction` 和 `compact` 函数。`force_full_compaction` 是压缩触发器，负责决定压缩哪些文件并更新 LSM 状态；`compact` 执行实际的压缩工作，合并一些 SST 文件并返回一组新的 SST 文件。

你的压缩实现应该取出存储引擎中所有 SST，使用 `MergeIterator` 对它们进行归并，然后使用 SST builder 将结果写入新文件。如果文件太大，需要拆分 SST 文件。压缩完成后，可以更新 LSM 状态，将所有新的有序运行添加到 LSM 树的第一层，并删除 LSM 树中不再使用的文件。在你的实现中，SST 只应存储在两个地方：L0 SST 和 L1 SST。也就是说，LSM 状态中的 `levels` 结构只应有一个向量。在 `LsmStorageState` 中，我们已经初始化 LSM 在 `levels` 字段中有 L1。

压缩不应阻塞 L0 刷写，因此合并文件时不应持有状态锁。只应在压缩过程结束时更新 LSM 状态时才获取状态锁，并在修改完状态后立即释放。

你可以假设用户保证同一时间只有一个压缩在进行中。`force_full_compaction` 在任何时刻只会被一个线程调用。放入第 1 层的 SST 应按其第一个键排序，且不应有重叠的键范围。

<details>

<summary>提示：压缩伪代码</summary>

```rust,no_run
fn force_full_compaction(&self) {
    let ssts_to_compact = {
        let state = self.state.read();
        state.l0_sstables + state.levels[0]
    };
    let new_ssts = self.compact(FullCompactionTask(ssts_to_compact))?;
    {
        let state_lock = self.state_lock.lock();
        let state = self.state.write();
        state.l0_sstables.remove(/* 被压缩的那些 */);
        state.levels[0] = new_ssts; // 新 SST 添加到 L1
    };
    std::fs::remove(ssts_to_compact)?;
}
```

</details>

在你的压缩实现中，目前只需要处理 `FullCompaction`，其任务信息包含需要压缩的 SST。你还需要确保 SST 的顺序正确，使得键的最新版本会被放入新的 SST。

由于我们始终压缩所有 SST，如果发现一个键的多个版本，可以只保留最新版本。如果最新版本是删除标记，则不需要在生成的 SST 文件中保留它。这一点不适用于后续章节中的压缩策略。

需要思考以下几点：

* 你的实现如何处理压缩期间并行发生的 L0 刷写？（压缩时不持有状态锁，同时还需要考虑压缩进行中产生的新 L0 文件）
* 如果你的实现在压缩完成后立即删除原始 SST 文件，会在系统中造成问题吗？（在 macOS/Linux 上通常不会，因为只要有文件句柄持有，操作系统不会真正删除文件）

## 任务 2：ConcatIterator（拼接迭代器）

本任务需要修改：

```
src/iterators/concat_iterator.rs
```

现在系统中已经有了有序运行，可以对读路径进行一个简单的优化。对于 SST，不必总是创建合并迭代器。如果 SST 属于同一个有序运行，可以创建一个拼接迭代器，按顺序依次迭代每个 SST 中的键，因为同一有序运行中的 SST 键范围不重叠且按第一个键排序。我们不想预先创建所有 SST 迭代器（因为这会导致一次块读取），因此只在迭代器中存储 SST 对象。

## 任务 3：与读路径集成

本任务需要修改：

```
src/lsm_iterator.rs
src/lsm_storage.rs
src/compact.rs
```

现在 LSM 树有了两层结构，可以修改读路径使用新的拼接迭代器来优化读路径。

你需要修改 `LsmStorageIterator` 的内部迭代器类型。之后，可以构建一个合并 memtable 和 L0 SST 的双路合并迭代器，再构建另一个将该迭代器与 L1 拼接迭代器合并的迭代器。

你也可以修改压缩实现来利用拼接迭代器。

你需要为拼接迭代器实现 `num_active_iterators`，以便测试用例验证你的实现中是否使用了拼接迭代器，该值应始终为 1。

交互式测试你的实现：

```shell
cargo run --bin mini-lsm-cli-ref -- --compaction none # 参考答案
cargo run --bin mini-lsm-cli -- --compaction none # 你的答案
```

然后：

```
fill 1000 3000
flush
fill 1000 3000
flush
full_compaction
fill 1000 3000
flush
full_compaction
get 2333
scan 2000 2333
```

## 理解检验

* 读/写/空间放大的定义是什么？（概览章节有介绍）
* 有哪些准确计算读/写/空间放大的方法？有哪些估算方法？
* 即使用户请求删除一个键，该键也会占用一些存储空间，这种说法正确吗？
* 压缩会消耗大量写带宽和读带宽，并可能干扰前台操作，因此在写入量大时推迟压缩是个好主意。在这种情况下，停止/暂停正在进行的压缩任务甚至是有益的。你怎么看这个想法？（阅读 [SILK: Preventing Latency Spikes in Log-Structured Merge Key-Value Stores](https://www.usenix.org/conference/atc19/presentation/balmau) 论文！）
* 在压缩时使用/填充块缓存是好主意吗？还是在压缩时完全绕过块缓存更好？
* 系统中是否有必要有 `struct ConcatIterator<I: StorageIterator>`？
* 一些研究者/工程师提议将压缩卸载到远程服务器或无服务器 lambda 函数上。这样做有什么好处？远程压缩可能面临哪些挑战和性能影响？（考虑压缩完成时的那个时刻，以及下一次读请求时块缓存会发生什么……）

以上问题不提供参考答案，欢迎在 Discord 社区中讨论。

{{#include copyright.md}}
