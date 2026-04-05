<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 分级压缩策略（Tiered Compaction Strategy）

![Chapter Overview](./lsm-tutorial/week2-00-tiered.svg)

在本章中，你将：

* 实现分级压缩策略，并在压缩模拟器上进行仿真。
* 将分级压缩策略集成到系统中。

本章所讨论的分级压缩（tiered compaction）与 RocksDB 的通用压缩（universal compaction）相同，下文将互换使用这两个术语。

将测试用例复制到 starter 代码并运行：

```
cargo x copy-test --week 2 --day 3
cargo x scheck
```

<div class="warning">

在阅读本章之前，建议先查看[第 2 周概览](./week2-overview.md)，以全面了解压缩的概念。

</div>

## 任务 1：通用压缩（Universal Compaction）

本章你将实现 RocksDB 的通用压缩，它属于分级压缩策略。与简单分层压缩策略类似，本策略也只使用文件数量作为指标，触发压缩任务时总是包含完整的有序运行（tier）。

### 任务 1.0：前置条件

本任务需要修改：

```
src/compact/tiered.rs
```

在通用压缩中，不使用 LSM 状态中的 L0 SST。而是将新 SST 直接刷写到单个有序运行（称为 tier）中。在 LSM 状态中，`levels` 现在包含所有 tier，**索引最小的是最新刷写的 SST**。`levels` 向量中的每个元素存储一个元组：层 ID（用作 tier ID）和该层的 SST。每次刷写 L0 SST 时，应将 SST 刷写到向量头部的新 tier。压缩模拟器基于第一个 SST 的 id 生成 tier id，你的实现也应如此。

通用压缩只有在 tier（有序运行）数量达到 `num_tiers` 时才会触发任务，否则不触发任何压缩。

### 任务 1.1：由空间放大比率触发

通用压缩的第一个触发条件是空间放大比率。如概览章节所述，空间放大可以估算为 `engine_size / last_level_size`。在我们的实现中，计算方式为 `除最后一层外所有层的大小 / 最后一层大小`，使得比率范围为 `[0, +inf)` 而非 `[1, +inf]`，这与 RocksDB 的实现一致。

这样计算的原因是：我们将引擎建模为存储固定数量的用户数据（比如 100GB），用户不断通过写入来更新值。因此，最终所有键都下沉到最底层，最底层大小应等于数据量（100GB），上层包含尚未压缩到最底层的更新。

当 `除最后一层外所有层的大小 / 最后一层大小` >= `max_size_amplification_percent * 1%` 时，需要触发全量压缩。例如，若 LSM 状态如下：

```
Tier 3: 1
Tier 2: 1 ; 除最后一层外所有层大小 = 2
Tier 1: 1 ; 最后一层大小 = 1，2/1=2
```

假设 `max_size_amplification_percent` = 200，此时应触发全量压缩。

实现此触发条件后，可以运行压缩模拟器：

```shell
cargo run --bin compaction-simulator tiered --iterations 10
```

```
=== Iteration 2 ===
--- After Flush ---
L3 (1): [3]
L2 (1): [2]
L1 (1): [1]
--- Compaction Task ---
compaction triggered by space amplification ratio: 200
L3 [3] L2 [2] L1 [1] -> [4, 5, 6]
--- After Compaction ---
L4 (3): [3, 2, 1]
```

使用此触发条件，只有在达到空间放大比率时才会触发全量压缩。仿真结束时你会看到：

```bash
cargo run --bin compaction-simulator tiered
```

```
=== Iteration 7 ===
--- After Flush ---
...
--- Compaction Task ---
compaction triggered by space amplification ratio: 700
L8 [8] L7 [7] ... L1 [1] -> [9, 10, 11, 12, 13, 14, 15, 16]
--- After Compaction ---
L9 (8): [8, 7, 6, 5, 4, 3, 2, 1]
--- Statistics ---
Write Amplification: 16/8=2.000x
Maximum Space Usage: 16/8=2.000x
Read Amplification: 1x

=== Iteration 49 ===
...
no compaction triggered
--- Statistics ---
Write Amplification: 82/50=1.640x
Maximum Space Usage: 50/50=1.000x
Read Amplification: 27x
```

压缩模拟器中 `num_tiers` 设为 8，但 LSM 状态中的 tier 数远超 8，导致读放大很高。当前触发条件只能降低空间放大，我们还需要新的触发条件来降低读放大。

### 任务 1.2：由大小比率触发

下一个触发条件是大小比率触发。从第一个 tier 开始，计算 `当前 tier 大小 / 所有之前 tier 的大小之和`。对于第一个满足该值 `> (100 + size_ratio) * 1%` 的 tier，将该 tier 之前的所有 tier（不含当前 tier）进行合并。只有当待合并的 tier 数量超过 `min_merge_width` 时才执行此压缩。

以下示例中，假设 `size_ratio` = 1，`min_merge_width` = 2，触发条件为比率值 > 101%：

```
Tier 3: 1
Tier 2: 1 ; 1/1 = 1
Tier 1: 1 ; 1/(1+1) = 0.5，不触发压缩
```

示例 2：

```
Tier 3: 1
Tier 2: 1 ; 1/1 = 1
Tier 1: 3 ; 3/(1+1) = 1.5，合并 Tier 2+3
```

```
Tier 4: 2
Tier 1: 3
```

示例 3：

```
Tier 3: 1
Tier 2: 2 ; 2/1 = 2，但只合并一个 tier 没有意义；注意 min_merge_width=2
Tier 1: 4 ; 4/3 = 1.33，合并 Tier 2+3
```

```
Tier 4: 3
Tier 1: 4
```

启用此触发条件后，压缩模拟器中会看到：

```bash
cargo run --bin compaction-simulator tiered
```

```
=== Iteration 49 ===
...
no compaction triggered
--- Statistics ---
Write Amplification: 119/50=2.380x
Maximum Space Usage: 52/50=1.040x
Read Amplification: 7x
```

```bash
cargo run --bin compaction-simulator tiered --iterations 200 --size-only
```

```
=== Iteration 199 ===
--- After Flush ---
Levels: 0 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 2 3 4 5 6 10 15 21 28 78
no compaction triggered
--- Statistics ---
Write Amplification: 537/200=2.685x
Maximum Space Usage: 200/200=1.000x
Read Amplification: 38x
```

1-SST tier 会减少，压缩算法会维护从小到大按大小比率排列的 tier。但当 LSM 状态中有更多 SST 时，仍可能出现超过 `num_tiers` 个 tier 的情况。为限制 tier 数量，还需要另一个触发条件。

### 任务 1.3：减少有序运行数量

如果前面的触发条件都没有产生压缩任务，则执行主要压缩：将前 `max_merge_tiers` 个 tier 的 SST 文件合并为一个 tier，以减少 tier 数量。

启用此压缩触发条件后：

```bash
cargo run --bin compaction-simulator-ref tiered --iterations 200 --size-only
```

```
=== Iteration 199 ===
--- After Flush ---
Levels: 0 1 1 4 5 21 28 140
no compaction triggered
--- Statistics ---
Write Amplification: 742/200=3.710x
Maximum Space Usage: 280/200=1.400x
Read Amplification: 7x
```

也可以用更多 tier 测试：

```bash
cargo run --bin compaction-simulator tiered --iterations 200 --size-only --num-tiers 16
```

```
=== Iteration 199 ===
--- After Flush ---
Levels: 0 1 1 1 1 1 1 1 1 1 1 15 175
no compaction triggered
--- Statistics ---
Write Amplification: 607/200=3.035x
Maximum Space Usage: 350/200=1.750x
Read Amplification: 12x
```

**注意：本部分没有细粒度的单元测试。你可以运行压缩模拟器并与参考答案的输出进行对比，以验证你的实现是否正确。**

## 任务 2：与读路径集成

本任务需要修改：

```
src/compact.rs
src/lsm_storage.rs
```

由于分级压缩不使用 LSM 状态的 L0 层，应直接将 memtable 刷写到新 tier，而不是作为 L0 SST。你可以使用 `self.compaction_controller.flush_to_l0()` 来判断是否应刷写到 L0。可以使用第一个输出 SST 的 id 作为新有序运行的层/tier id。你还需要修改压缩过程，为分级压缩任务构建合并迭代器。

## 扩展阅读

[Universal Compaction - RocksDB Wiki](https://github.com/facebook/rocksdb/wiki/Universal-Compaction)

## 理解检验

* 通用压缩（不含最后一个"减少有序运行"触发条件）的估计写放大是多少？（这个很难估算……）
* 通用压缩的估计读放大是多少？
* 与简单分层/分级压缩相比，通用压缩的优缺点是什么？
* 运行通用压缩需要多少存储空间（相对于用户数据大小）？
* 是否可以合并 LSM 状态中不相邻的两个 tier？
* 如果对于分级压缩，压缩速度跟不上 SST 刷写速度，会发生什么？
* 如果系统并行调度多个压缩任务，需要考虑哪些问题？
* SSD 也会写自己的日志（本质上也是日志结构存储）。如果 SSD 的写放大为 2x，整个系统的端到端写放大是多少？相关：[ZNS: Avoiding the Block Interface Tax for Flash-based SSDs](https://www.usenix.org/conference/atc21/presentation/bjorling)。
* 考虑用户选择为分级压缩保留大量有序运行（如 300 个）的情况。为了加快读路径，是否值得保留一些数据结构，将在每层中查找 SST 的时间复杂度降至 `O(log n)`？注意，通常需要在每个有序运行中进行二分查找来找到需要读取的键范围。（参考 Neon 的 [layer map](https://neon.tech/blog/persistent-structures-in-neons-wal-indexing) 实现！）

以上问题不提供参考答案，欢迎在 Discord 社区中讨论。

{{#include copyright.md}}
