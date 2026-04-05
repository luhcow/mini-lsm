<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# Manifest（清单文件）

![Chapter Overview](./lsm-tutorial/week2-05-overview.svg)

在本章中，你将：

* 实现 manifest 文件的编解码。
* 在系统重启时从 manifest 恢复状态。

将测试用例复制到 starter 代码并运行：

```
cargo x copy-test --week 2 --day 5
cargo x scheck
```

## 任务 1：Manifest 编码

系统使用 manifest 文件记录引擎中发生的所有操作。目前只有两种类型：压缩和 SST 刷写。引擎重启时会读取 manifest 文件，重建状态，并加载磁盘上的 SST 文件。

存储 LSM 状态有很多方法。最简单的方式是将完整状态存储到 JSON 文件中。每次进行压缩或刷写新 SST 时，将整个 LSM 状态序列化到文件中。这种方式的问题是，当数据库非常大时（如 10k 个 SST），写 manifest 到磁盘会非常慢。因此，我们将 manifest 设计为仅追加文件。

本任务需要修改：

```
src/manifest.rs
```

我们使用 JSON 对 manifest 记录进行编码。可以使用 `serde_json::to_vec` 将 manifest 记录编码为 JSON，写入 manifest 文件，然后执行 fsync。读取 manifest 文件时，可以使用 `serde_json::Deserializer::from_slice`，它会返回一个记录流，不需要存储每条记录的长度，`serde_json` 能自动找到记录的分界。

Manifest 格式如下：

```
| JSON record | JSON record | JSON record | JSON record |
```

注意，我们不记录每条记录的字节数。

引擎运行数小时后，manifest 文件可能会变得很大。这时可以定期压缩 manifest 文件，只存储当前快照并截断日志。这可以作为进阶任务实现。

## 任务 2：写入 Manifest

现在可以修改 LSM 引擎，在必要时写入 manifest。本任务需要修改：

```
src/lsm_storage.rs
src/compact.rs
```

目前只使用两种类型的 manifest 记录：SST 刷写和压缩。SST 刷写记录存储被刷写到磁盘的 SST id。压缩记录存储压缩任务和生成的 SST id。每次向磁盘写入新文件时，先同步文件和存储目录，然后写入 manifest 并同步 manifest。Manifest 文件应写入 `<path>/MANIFEST`。

同步目录可以通过实现 `sync_dir` 函数完成，使用 `File::open(dir).sync_all()?`。在 Linux 上，目录也是文件，包含目录中的文件列表。对目录执行 fsync 可以确保在断电的情况下新写入（或删除）的文件对用户可见。

记得为后台压缩触发器（分层/简单/通用）和用户请求的强制压缩都写入压缩 manifest 记录。

## 任务 3：关闭时刷写

本任务需要修改：

```
src/lsm_storage.rs
```

你需要实现 `close` 函数。如果 `self.options.enable_wal = false`（下一章介绍 WAL），在停止存储引擎之前应将所有 memtable 刷写到磁盘，以确保所有用户更改都被持久化。

## 任务 4：从状态恢复

本任务需要修改：

```
src/lsm_storage.rs
```

现在可以修改 `open` 函数，从 manifest 文件恢复引擎状态。恢复时需要首先生成需要加载的 SST 列表，可以通过调用 `apply_compaction_result` 并恢复 LSM 状态中的 SST id 来实现。之后，可以遍历状态并加载所有 SST（更新 sstables 哈希映射）。在此过程中，需要计算最大 SST id 并更新 `next_sst_id` 字段。之后，可以使用该 id 创建新的 memtable，并将 id 加 1。

如果你实现了分层压缩，每次应用压缩结果时可能都对 SST 进行了排序。但是在 manifest 恢复时，你的排序逻辑会出问题，因为恢复过程中无法知道每个 SST 的起始键和结束键。解决方法是读取 `apply_compaction_result` 函数的 `in_recovery` 标志。在恢复过程中，不应尝试获取 SST 的第一个键。LSM 状态恢复完毕且所有 SST 打开后，可以在恢复过程结束时进行一次排序。

或者，可以在 manifest 中包含每个 SST 的起始键和结束键。RocksDB/BadgerDB 使用了这种策略，这样在压缩应用过程中就无需区分恢复模式和正常模式。

可以使用 mini-lsm-cli 测试你的实现：

```
cargo run --bin mini-lsm-cli
fill 1000 2000
close
cargo run --bin mini-lsm-cli
get 1500
```

## 理解检验

* 什么时候需要调用 `fsync`？为什么需要对目录执行 fsync？
* 在哪些地方需要写入 manifest？
* 考虑一种不使用 manifest 文件的 LSM 引擎替代实现：将层/tier 信息记录在每个文件的头部，每次重启时扫描存储目录，仅从目录中存在的文件恢复 LSM 状态。这种实现能正确维护 LSM 状态吗？可能面临哪些问题/挑战？
* 目前，我们在创建合并迭代器之前先创建所有 SST/拼接迭代器，这意味着在开始扫描过程之前必须将所有层中第一个 SST 的第一个块加载到内存。manifest 中有起始/结束键信息，是否可以利用这些信息延迟数据块的加载，使返回第一个键值对的时间更快？
* 是否可以不在 manifest 中存储 tier/层信息？即只在 manifest 中存储 SST 列表，通过键范围和时间戳信息（SST 元数据）重建 tier/层？

## 进阶任务

* **Manifest 压缩。** 当 manifest 文件中的日志数量过多时，可以重写 manifest 文件，只存储当前快照，并将新日志追加到该文件。
* **并行打开。** 收集到需要打开的 SST 列表后，可以并行打开和解码它们，而不是逐个处理，从而加速恢复过程。

{{#include copyright.md}}
