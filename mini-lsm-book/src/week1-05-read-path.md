<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 读路径（Read Path）

![Chapter Overview](./lsm-tutorial/week1-05-overview.svg)

在本章中，你将：

* 将 SST 集成到 LSM 读路径中。
* 实现带 SST 的 LSM 读路径 `get`。
* 实现带 SST 的 LSM 读路径 `scan`。

将测试用例复制到 starter 代码并运行：

```
cargo x copy-test --week 1 --day 5
cargo x scheck
```

## 任务 1：双路合并迭代器

本任务需要修改：

```
src/iterators/two_merge_iterator.rs
```

你已经实现了合并同类型迭代器（即 memtable 迭代器）的合并迭代器。现在我们实现了 SST 格式，既有磁盘上的 SST 结构，也有内存中的 memtable。从存储引擎扫描时，需要将 memtable 迭代器和 SST 迭代器中的数据合并为一个。为此，我们需要一个 `TwoMergeIterator<X, Y>` 来合并两种不同类型的迭代器。

你可以在 `two_merge_iterator.rs` 中实现 `TwoMergeIterator`。由于只有两个迭代器，不需要维护二叉堆，只需用一个标志来指示从哪个迭代器读取即可。与 `MergeIterator` 类似，如果两个迭代器中都找到了相同的键，第一个迭代器优先。

## 任务 2：读路径——Scan

本任务需要修改：

```
src/lsm_iterator.rs
src/lsm_storage.rs
```

实现 `TwoMergeIterator` 后，可以将 `LsmIteratorInner` 的类型改为：

```rust,no_run
type LsmIteratorInner =
    TwoMergeIterator<MergeIterator<MemTableIterator>, MergeIterator<SsTableIterator>>;
```

这样，LSM 存储引擎的内部迭代器将同时合并来自 memtable 和 SST 的数据。

目前，我们的 SST 迭代器不支持扫描的上界。为了解决这个问题，你需要在 `LsmIterator` 本身内实现这个边界检查。需要更新 `LsmIterator::new` 构造函数以接受 `end_bound` 参数：

```rust,no_run
pub(crate) fn new(iter: LsmIteratorInner, end_bound: Bound<Bytes>) -> Result<Self> {}
```

然后修改 `LsmIterator` 的迭代逻辑，确保当内部迭代器的键到达或超过指定的 `end_bound` 时停止。

测试用例会生成一些 memtable 和 `l0_sstables` 中的 SST，你需要在本任务中正确扫描出所有这些数据。下一章之前不需要刷写 SST。因此，你可以修改 `LsmStorageInner::scan` 接口，创建一个覆盖所有 memtable 和 SST 的合并迭代器，从而完成存储引擎的读路径。

由于 `SsTableIterator::create` 涉及 I/O 操作且可能较慢，不希望在 `state` 临界区内执行。因此，应先获取 `state` 读锁并克隆 LSM 状态快照的 `Arc`，然后释放锁。之后再遍历所有 L0 SST 并为每个创建迭代器，最后创建合并迭代器来检索数据。

```rust,no_run
fn scan(&self) {
    let snapshot = {
        let guard = self.state.read();
        Arc::clone(&guard)
    };
    // 创建迭代器并 seek
}
```

LSM 存储状态只在 `l0_sstables` 向量中存储 SST 的 id，需要从 `sstables` 哈希映射中获取实际的 SST 对象。

## 任务 3：读路径——Get

本任务需要修改：

```
src/lsm_storage.rs
```

对于 get 请求，先在 memtable 中查找，然后在 SST 上扫描。探测完所有 memtable 后，可以对所有 SST 创建合并迭代器，并 seek 到用户想查找的键。seek 结果有两种情况：键与用户查找的键相同，或键不同/不存在。只有当键存在且与查找的键相同时，才返回值给用户。同样需要像前一节那样减少状态锁的临界区范围。记得处理已删除的键。

## 理解检验

* 考虑这样的场景：用户有一个遍历整个存储引擎的迭代器，存储引擎大小为 1TB，扫描所有数据大约需要 1 小时。这样做会有什么问题？（这是一个很好的问题，我们会在课程的不同阶段多次提到……）
* 一些 LSM 树存储引擎提供的另一个流行接口是 multi-get（或 vectored get）。用户可以传入一组键来检索，接口返回每个键的值。例如，`multi_get(vec!["a", "b", "c", "d"]) -> a=1,b=2,c=3,d=4`。显然，最简单的实现是为每个键单独执行一次 get。你会如何实现 multi-get 接口，以及如何优化使其更高效？（提示：get 过程中的一些操作对所有键只需执行一次；此外，你可以考虑改进磁盘 I/O 接口以更好地支持 multi-get）

以上问题不提供参考答案，欢迎在 Discord 社区中讨论。

## 进阶任务

* **动态分派的代价。** 实现基于 `Box<dyn StorageIterator>` 的合并迭代器并进行基准测试，对比性能差异。
* **并行 Seek。** 创建合并迭代器时需要加载所有底层 SST 的第一个块（创建 `SSTIterator` 时）。你可以并行化创建迭代器的过程。

{{#include copyright.md}}
