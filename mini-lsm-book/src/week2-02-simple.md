<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 简单分层压缩策略（Simple Compaction Strategy）

![Chapter Overview](./lsm-tutorial/week2-02-simple.svg)

在本章中，你将：

* 实现简单分层压缩策略，并在压缩模拟器上进行仿真。
* 将压缩作为后台任务启动，并在系统中实现压缩触发器。

将测试用例复制到 starter 代码并运行：

```
cargo x copy-test --week 2 --day 2
cargo x scheck
```

<div class="warning">

在阅读本章之前，建议先查看[第 2 周概览](./week2-overview.md)，以全面了解压缩的概念。

</div>

## 任务 1：简单分层压缩

本章我们来实现第一个压缩策略——简单分层压缩（simple leveled compaction）。本任务需要修改：

```
src/compact/simple_leveled.rs
```

简单分层压缩与 LSM 原始论文的压缩策略类似。它为 LSM 树维护若干层级。当某一层（>= L1）过大时，会将该层所有 SST 与下一层合并。该压缩策略由 `SimpleLeveledCompactionOptions` 中定义的 3 个参数控制：

* `size_ratio_percent`：下层文件数 / 上层文件数的比率。实际应计算文件的实际大小，但为了便于仿真，我们简化为使用文件数量。当比率过低（上层文件过多）时，触发压缩。
* `level0_file_num_compaction_trigger`：L0 中 SST 数量大于或等于该值时，触发 L0 和 L1 的压缩。
* `max_levels`：LSM 树中的层数（不含 L0）。

假设 size_ratio_percent=200（下层文件数应是上层的 2 倍）、max_levels=3、level0_file_num_compaction_trigger=2，看下面的例子：

假设引擎刷写了两个 L0 SST，达到 `level0_file_num_compaction_trigger`，你的控制器应触发 L0->L1 压缩：

```
--- 刷写后 ---
L0 (2): [1, 2]
L1 (0): []
L2 (0): []
L3 (0): []
--- 压缩后 ---
L0 (0): []
L1 (2): [3, 4]
L2 (0): []
L3 (0): []
```

现在 L2 为空，L1 有两个文件。L1 和 L2 的比率为 `(L2/L1) * 100 = (0/2) * 100 = 0 < size_ratio_percent (200)`。因此触发 L1+L2 压缩，将数据下推到 L2。同样适用于 L2，这两个 SST 经过 2 次压缩后会被放到最底层：

```
--- 压缩后 ---
L0 (0): []
L1 (0): []
L2 (2): [5, 6]
L3 (0): []
--- 压缩后 ---
L0 (0): []
L1 (0): []
L2 (0): []
L3 (2): [7, 8]
```

继续刷写 SST，最终会发现：

```
L0 (0): []
L1 (0): []
L2 (2): [13, 14]
L3 (2): [7, 8]
```

此时 `L3/L2 = (1/1) * 100 = 100 < size_ratio_percent (200)`，需要触发 L2 和 L3 之间的压缩：

```
--- 压缩后 ---
L0 (0): []
L1 (0): []
L2 (0): []
L3 (4): [15, 16, 17, 18]
```

随着继续刷写 SST，可能最终达到如下状态：

```
--- 刷写后 ---
L0 (2): [19, 20]
L1 (0): []
L2 (0): []
L3 (4): [15, 16, 17, 18]
--- 压缩后 ---
L0 (0): []
L1 (0): []
L2 (2): [23, 24]
L3 (4): [15, 16, 17, 18]
```

由于 `L3/L2 = (4/2) * 100 = 200 >= size_ratio_percent (200)`，不需要合并 L2 和 L3，最终保持上述状态。简单分层压缩策略总是压缩整个层，并在各层之间保持扇出比，使下层始终是上层的某个倍数大小。

我们已经将 LSM 状态初始化为有 `max_level` 层。你应该首先实现 `generate_compaction_task`，根据上述 3 个条件生成压缩任务。然后实现 `apply_compaction_result`。建议先实现 L0 触发，运行压缩仿真，再实现比率触发，再运行压缩仿真。运行压缩仿真：

```shell
cargo run --bin compaction-simulator-ref simple # 参考答案
cargo run --bin compaction-simulator simple # 你的答案
```

仿真器会向 LSM 状态刷写一个 L0 SST，运行你的压缩控制器生成压缩任务，然后应用压缩结果。每次刷写新 SST 后，都会重复调用控制器直到不需要再调度压缩，因此你应确保压缩任务生成器会收敛。

在你的压缩实现中，应尽量减少活跃迭代器的数量（即使用拼接迭代器）。同时记住，合并顺序很重要，当一个键的多个版本出现时，你需要确保创建的迭代器能以正确顺序产生键值对。

另外，注意实现中某些参数是从 0 开始索引，某些是从 1 开始索引。在将 `level` 用作向量索引时要小心。

**注意：本部分没有细粒度的单元测试。你可以运行压缩仿真器并与参考答案的输出进行对比，以验证你的实现是否正确。**

## 任务 2：压缩线程

本任务需要修改：

```
src/compact.rs
```

现在你已经实现了压缩策略，需要在后台线程中运行它，以便在后台压缩文件。在 `compact.rs` 中，`trigger_compaction` 每 50ms 调用一次，你需要：

1. 生成压缩任务，如果不需要调度任务，返回 ok。
2. 运行压缩并获取新 SST 的列表。
3. 与上一章实现的 `force_full_compaction` 类似，更新 LSM 状态。

## 任务 3：与读路径集成

本任务需要修改：

```
src/lsm_storage.rs
```

现在有了多层 SST，可以修改读路径以包含新层的 SST。你需要更新 scan/get 函数以包含 L1 以下的所有层，可能还需要再次修改 `LsmStorageIterator` 的内部类型。

交互式测试你的实现：

```shell
cargo run --bin mini-lsm-cli-ref -- --compaction simple # 参考答案
cargo run --bin mini-lsm-cli -- --compaction simple # 你的答案
```

然后：

```
fill 1000 3000
flush
fill 1000 3000
flush
fill 1000 3000
flush
get 2333
scan 2000 2333
```

可以在压缩器触发压缩时打印一些信息，例如压缩任务信息。

## 理解检验

* 分层压缩的估计写放大是多少？
* 分层压缩的估计读放大是多少？
* 只有当用户请求删除某个键且该键已被压缩到最底层时，该键才会从 LSM 树中被清除，这种说法正确吗？
* 定期对 LSM 树进行全量压缩是好策略吗？为什么？
* 主动选择一些旧文件/层进行压缩（即使它们没有违反层放大因子）是否是好的选择？（看看 [Lethe](https://disc-projects.bu.edu/lethe/) 论文！）
* 如果存储设备能达到 1GB/s 的持续写吞吐量，LSM 树的写放大为 10x，用户从 LSM 键值接口能获得多少吞吐量？
* 如果 L2 中有 SST 文件，可以直接合并 L1 和 L3 吗？这样还能产生正确结果吗？
* 目前，我们假设 SST 文件使用单调递增的 id 作为文件名。使用 `<level>_<begin_key>_<end_key>.sst` 作为 SST 文件名可以吗？这样做有什么潜在问题？（第 3 周可以再思考这个问题……）
* 你们城市你最喜欢的珍珠奶茶店是哪家？（如果你在第 1 周第 3 天回答了"是"……）

以上问题不提供参考答案，欢迎在 Discord 社区中讨论。

{{#include copyright.md}}
