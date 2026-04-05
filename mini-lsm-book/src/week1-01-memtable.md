<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# Memtable（内存表）

![Chapter Overview](./lsm-tutorial/week1-01-overview.svg)

在本章中，你将：

* 基于跳表实现 memtable。
* 实现冻结 memtable 的逻辑。
* 实现 memtable 的 LSM 读路径 `get`。

将测试用例复制到 starter 代码并运行：

```
cargo x copy-test --week 1 --day 1
cargo x scheck
```

## 任务 1：基于 SkipList 的 Memtable

本任务需要修改：

```
src/mem_table.rs
```

首先，我们来实现 LSM 存储引擎的内存结构——memtable。我们选用 [crossbeam 的跳表实现](https://docs.rs/crossbeam-skiplist/latest/crossbeam_skiplist/) 作为 memtable 的底层数据结构，因为它支持无锁并发读写。本课程不深入讲解跳表的工作原理，简而言之，它是一种有序键值映射，天然支持并发读写。

crossbeam-skiplist 提供了与 Rust 标准库 `BTreeMap` 类似的接口：insert、get 和 iter。唯一的区别是修改接口（即 `insert`）只需要对跳表的不可变引用，而不需要可变引用。因此，在你的实现中，实现 memtable 结构时不需要加任何互斥锁。

你还会注意到 `MemTable` 结构没有 `delete` 接口。在 mini-lsm 实现中，删除操作用对应键的空值来表示。

在本任务中，你需要实现 `MemTable::get` 和 `MemTable::put` 以支持对 memtable 的修改。注意 `put` 应该总是在键已存在时覆盖它。单个 memtable 中同一个键不会有多个条目。

我们使用 `bytes` crate 来存储 memtable 中的数据。`bytes::Bytes` 类似于 `Arc<[u8]>`。克隆 `Bytes` 或获取 `Bytes` 的切片时，底层数据不会被复制，因此克隆操作是廉价的。它只是创建了一个新的指向存储区域的引用，当没有引用指向该区域时，存储区域才会被释放。

## 任务 2：引擎中的单个 Memtable

本任务需要修改：

```
src/lsm_storage.rs
```

现在，我们将第一个数据结构 memtable 加入 LSM 状态。在 `LsmStorageState::create` 中，你会看到创建 LSM 结构时会初始化一个 id 为 0 的 memtable。这就是初始状态下的**可写 memtable**。在任意时刻，引擎只有一个可写 memtable。memtable 通常有大小限制（如 256MB），达到上限时会被冻结为不可变 memtable。

查看 `lsm_storage.rs`，你会发现有两个结构表示存储引擎：`MiniLSM` 和 `LsmStorageInner`。`MiniLSM` 是 `LsmStorageInner` 的薄封装。在第 2 周的压缩实现之前，你将主要在 `LsmStorageInner` 中实现大多数功能。

`LsmStorageState` 存储 LSM 存储引擎的当前结构。目前我们只使用 `memtable` 字段，它存储当前可写 memtable。在本任务中，你需要实现 `LsmStorageInner::get`、`LsmStorageInner::put` 和 `LsmStorageInner::delete`，它们都应直接将请求分发给当前 memtable。

![one memtable LSM](./lsm-tutorial/week1-01-single.svg)

`delete` 实现应该直接为该键存入一个空切片，我们称之为*删除墓碑（delete tombstone）*。`get` 实现需要相应地处理这种情况。

要访问 memtable，需要获取 `state` 锁。由于 memtable 实现的 `put` 只需要不可变引用，因此修改 memtable 时**只需要对 `state` 加读锁**，从而允许多个线程并发访问 memtable。

## 任务 3：写路径——冻结 Memtable

本任务需要修改：

```
src/lsm_storage.rs
src/mem_table.rs
```

![one memtable LSM](./lsm-tutorial/week1-01-frozen.svg)

memtable 不能无限增长，当它达到大小限制时，需要将其冻结（之后再刷写到磁盘）。memtable 的大小限制在 `LsmStorageOptions` 中可以找到，**它等于 SST 大小限制**（不是 `num_memtables_limit`）。这不是一个硬性限制，你应尽力在达到限制时冻结 memtable。

在本任务中，你需要在 memtable 的 put/delete 操作时估算 memtable 的近似大小。估算方法很简单：每次调用 `put` 时将键和值的字节数累加即可。如果一个键被 put 两次，虽然跳表中只保留最新值，但你可以在近似大小中累计两次。一旦 memtable 达到大小限制，应调用 `force_freeze_memtable` 来冻结 memtable 并创建新的 memtable。

`LsmStorageInner` 中的 `state: Arc<RwLock<Arc<LsmStorageState>>>` 字段采用这种结构，是为了通过写时复制（CoW）策略来安全地并发管理 LSM 树的整体状态：

1. 内层 `Arc<LsmStorageState>`：存储实际 `LsmStorageState`（包含 memtable 列表、SST 引用等）的**不可变快照**。克隆这个 `Arc` 非常廉价（只是原子引用计数加一），任何读者都能获得一个在操作期间保持一致、不会改变的状态视图。

2. `RwLock<Arc<LsmStorageState>>`：这个读写锁保护指向当前 `Arc<LsmStorageState>`（活跃快照）的*指针*。
    * **读者**获取读锁，克隆 `Arc<LsmStorageState>`（获得当前快照的自有引用），然后快速释放读锁。之后可以使用自己的快照无需进一步加锁。
    * **写者**（修改状态时，例如冻结 memtable）将：
        * 创建一个*新的* `LsmStorageState` 实例，通常是从当前快照克隆数据后再应用修改。
        * 将新状态包装在新的 `Arc<LsmStorageState>` 中。
        * 获取 `RwLock` 的写锁。
        * 用新的 `Arc<LsmStorageState>` 替换旧的。
        * 释放写锁。

3. 外层 `Arc<RwLock<...>>`：允许 `RwLock` 本身（以及访问和更新状态的机制）在多个线程或应用程序的多个部分之间安全共享。

这种 CoW 方式确保读者始终看到有效、一致的状态快照，且阻塞最少。写者通过替换整个状态快照来原子性地更新状态，减少了持有关键锁的时间，从而提高并发性。

由于可能有多个线程同时向存储引擎写入数据，`force_freeze_memtable` 可能会被多个线程并发调用。你需要思考如何避免这种情况下的竞态条件。

有多个地方需要修改 LSM 状态：冻结可写 memtable、将 memtable 刷写到 SST、GC/压缩。在所有这些修改过程中都可能有 I/O 操作。直觉上的加锁策略是：

```rust,no_run
fn freeze_memtable(&self) {
    let state = self.state.write();
    state.immutable_memtable.push(/* something */);
    state.memtable = MemTable::create();
}
```

……即在 LSM 状态的写锁内修改所有内容。

这种方式目前可以正常工作。但考虑需要为每个新创建的 memtable 创建预写日志文件的情况：

```rust,no_run
fn freeze_memtable(&self) {
    let state = self.state.write();
    state.immutable_memtable.push(/* something */);
    state.memtable = MemTable::create_with_wal()?; // <- 可能需要几毫秒
}
```

现在冻结 memtable 时，其他线程将有几毫秒无法访问 LSM 状态，这会造成延迟尖峰。

解决方法是将 I/O 操作移到加锁区域之外：

```rust,no_run
fn freeze_memtable(&self) {
    let memtable = MemTable::create_with_wal()?; // <- 可能需要几毫秒
    {
        let state = self.state.write();
        state.immutable_memtable.push(/* something */);
        state.memtable = memtable;
    }
}
```

这样，状态写锁内就没有耗时操作了。但考虑另一个问题：memtable 快要达到容量限制，两个线程都成功向 memtable 写入了键，都在写入后发现 memtable 达到容量限制，它们都会进行大小检查并决定冻结 memtable。在这种情况下，可能会创建一个空的 memtable 然后立即被冻结。

解决这个问题需要通过 state lock 序列化所有状态修改：

```rust,no_run
fn put(&self, key: &[u8], value: &[u8]) {
    // 将数据写入 memtable，检查容量，然后释放 LSM 状态的读锁
    if memtable_reaches_capacity_on_put {
        let state_lock = self.state_lock.lock();
        if /* 再次检查当前 memtable 是否达到容量 */ {
            self.freeze_memtable(&state_lock)?;
        }
    }
}
```

你在后续章节中会经常看到这种模式。例如 L0 刷写：

```rust,no_run
fn force_flush_next_imm_memtable(&self) {
    let state_lock = self.state_lock.lock();
    // 获取最旧的 memtable 并释放 LSM 状态的读锁
    // 将内容写入磁盘
    // 获取 LSM 状态的写锁并更新状态
}
```

这确保只有一个线程能修改 LSM 状态，同时仍允许并发访问 LSM 存储。

在本任务中，你需要修改 `put` 和 `delete` 以遵守 memtable 的软容量限制。达到限制时调用 `force_freeze_memtable` 冻结 memtable。注意我们没有针对并发场景的测试用例，你需要自行考虑所有可能的竞态条件。同时，记得检查加锁区域，确保临界区最小化。

你可以简单地将 `self.next_sst_id()` 作为下一个 memtable 的 id。注意 `imm_memtables` 按从最新到最旧的顺序存储 memtable，即 `imm_memtables.first()` 应该是最后一个被冻结的 memtable。

## 任务 4：读路径——Get

本任务需要修改：

```
src/lsm_storage.rs
```

现在你有了多个 memtable，可以修改读路径的 `get` 函数来获取键的最新版本。确保从最新的 memtable 向最旧的 memtable 顺序查找。

## 理解检验

* 为什么 memtable 不提供 `delete` API？
* memtable 存储所有写操作（而不只是键的最新版本）是否有意义？例如，用户对同一个 memtable 执行 a->1、a->2、a->3。
* 是否可以在 LSM 中使用其他数据结构作为 memtable？使用跳表的优缺点是什么？
* 为什么我们需要同时使用 `state` 和 `state_lock`？能否只用 `state.read()` 和 `state.write()`？
* 为什么探测 memtable 的顺序很重要？如果一个键出现在多个 memtable 中，应该向用户返回哪个版本？
* memtable 的内存布局是否高效、是否有良好的数据局部性？（想想 `Bytes` 的实现方式以及它在跳表中的存储方式……）有哪些可能的优化手段？
* 本课程使用 `parking_lot` 锁。它的读写锁是否公平？如果有一个写者在等待现有读者释放锁，正在尝试获取锁的读者会发生什么？
* 冻结 memtable 后，是否可能有某些线程仍持有旧的 LSM 状态并向这些不可变 memtable 写入数据？你的方案如何防止这种情况发生？
* 有几处你可能先获取状态的读锁，然后释放它再获取写锁（这两个操作可能在不同函数中，但由于函数调用而顺序发生）。这与直接将读锁升级为写锁有什么区别？是否有必要升级而不是先释放再获取？升级的代价是什么？

以上问题不提供参考答案，欢迎在 Discord 社区中讨论。

## 进阶任务

* **更多 Memtable 格式。** 你可以实现其他 memtable 格式，例如 BTree memtable、vector memtable 和 ART memtable。

{{#include copyright.md}}
