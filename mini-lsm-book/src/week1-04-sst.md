<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# Sorted String Table（SST，有序字符串表）

![Chapter Overview](./lsm-tutorial/week1-04-overview.svg)

在本章中，你将：

* 实现 SST 编码与元数据编码。
* 实现 SST 解码与迭代器。

将测试用例复制到 starter 代码并运行：

```
cargo x copy-test --week 1 --day 4
cargo x scheck
```

## 任务 1：SST Builder（SST 构建器）

本任务需要修改：

```
src/table/builder.rs
src/table.rs
```

SST 由存储在磁盘上的数据块（data block）和索引块（index block）组成。通常，数据块是懒加载的——只有用户请求时才会加载到内存中。索引块也可以按需加载，但本课程简化处理，假设所有 SST 的索引块（元数据块）都能装入内存（实际上我们没有单独的索引块实现）。一般来说，SST 文件大小为 256MB。

SST builder 与 block builder 类似——用户调用 builder 的 `add` 方法。你需要在 SST builder 内部维护一个 `BlockBuilder`，并在必要时切分块。你还需要维护块元数据 `BlockMeta`，包括每个块的第一个/最后一个键以及各块的偏移量。`build` 函数将对 SST 进行编码，使用 `FileObject::create` 将所有内容写入磁盘，并返回一个 `SsTable` 对象。

SST 的编码格式如下：

```plaintext
-------------------------------------------------------------------------------------------
|         Block Section（块区）         |      Meta Section（元数据区）      |      Extra      |
-------------------------------------------------------------------------------------------
| data block | ... | data block |            metadata           | meta block offset (u32) |
-------------------------------------------------------------------------------------------
```

你还需要实现 `SsTableBuilder` 的 `estimated_size` 函数，以便调用方知道何时可以开始写入新的 SST。该函数不需要非常精确。考虑到数据块包含的数据远多于元数据块，`estimated_size` 可以简单地返回数据块的大小。

除了 SST builder，你还需要完成块元数据的编解码，以便 `SsTableBuilder::build` 能生成合法的 SST 文件。

## 任务 2：SST 迭代器

本任务需要修改：

```
src/table/iterator.rs
src/table.rs
```

与 `BlockIterator` 类似，你需要在 SST 上实现一个迭代器。注意数据应按需加载。例如，如果迭代器当前在第 1 块，在移动到下一块之前不应在内存中持有任何其他块的内容。

`SsTableIterator` 应实现 `StorageIterator` trait，以便将来与其他迭代器组合使用。

有一点需要注意：`seek_to_key` 函数。基本上，你需要对块元数据进行二分查找，以确定哪个块可能包含该键。由于键可能不存在于 LSM 树中，块迭代器在 seek 后可能立即失效。例如：

```plaintext
--------------------------------------
| block 1 | block 2 |   block meta   |
--------------------------------------
| a, b, c | e, f, g | 1: a/c, 2: e/g |
--------------------------------------
```

我们建议只使用每个块的第一个键来做二分查找，以降低实现复杂度。如果在这个 SST 上执行 `seek(b)`，很简单——通过二分查找，可以知道块 1 包含 `a <= keys < e` 范围内的键。因此，加载块 1 并将块迭代器定位到对应位置。

但如果执行 `seek(d)`，使用第一个键作为二分查找条件的话会定位到块 1，但在块 1 中 seek `d` 会到达块的末尾。因此，seek 后应检查迭代器是否无效，并在必要时切换到下一块。你也可以利用最后一个键的元数据直接定位到正确的块，具体由你决定。

## 任务 3：块缓存

本任务需要修改：

```
src/table/iterator.rs
src/table.rs
```

你可以在 `SsTable` 上实现一个新的 `read_block_cached` 函数。

我们使用 [`moka-rs`](https://docs.rs/moka/latest/moka/) 作为块缓存实现。块以 `(sst_id, block_id)` 作为缓存键进行缓存。你可以使用 `try_get_with` 在缓存命中时从缓存获取块，或在缓存未命中时填充缓存。如果多个请求读取同一块且缓存未命中，`try_get_with` 只会向磁盘发出一次读请求并将结果广播给所有请求。

此时，你可以将表迭代器改为使用 `read_block_cached` 而非 `read_block`，以充分利用块缓存。

## 理解检验

* 在 SST 中查找一个键的时间复杂度是多少？
* 在你的实现中，seek 一个不存在的键时，游标会停在哪里？
* 是否可能（或有必要）对 SST 文件进行就地更新？
* SST 通常很大（如 256MB）。在这种情况下，复制/扩展 `Vec` 的代价会很高。你的实现是否提前为 SST builder 分配了足够的空间？你是怎么实现的？
* 查看 `moka` 的块缓存，为什么它返回 `Arc<Error>` 而不是原始的 `Error`？
* 使用块缓存是否保证内存中最多有固定数量的块？例如，如果你有一个 4GB 的 `moka` 块缓存，块大小为 4KB，内存中同时存在的块数量会超过 4GB/4KB 吗？
* 是否可以在 LSM 引擎中存储列式数据（即包含 100 个整数列的表）？当前的 SST 格式是否仍然是好的选择？
* 考虑将 LSM 引擎构建在对象存储服务（如 S3）之上的场景。你会如何优化或修改 SST 格式/参数以及块缓存，使其更适合此类服务？
* 目前，我们将所有 SST 的索引都加载到内存中。假设为索引预留了 16GB 内存，你能估计你的 LSM 系统最多能支持多大的数据库吗？（这就是为什么你需要索引缓存！）

以上问题不提供参考答案，欢迎在 Discord 社区中讨论。

## 进阶任务

* **探索不同的 SST 编码与布局。** 例如，在 [Lethe: Enabling Efficient Deletes in LSMs](https://disc-projects.bu.edu/lethe/) 论文中，作者为 SST 添加了二级键支持。
  * 或者可以用 B+ 树替代有序块作为 SST 格式。
* **索引块。** 将块索引和块元数据拆分到独立的索引块中，并按需加载。
* **索引缓存。** 使用独立于数据块缓存的索引缓存。
* **I/O 优化。** 将块对齐到 4KB 边界，并使用直接 I/O 绕过系统页缓存。

{{#include copyright.md}}
