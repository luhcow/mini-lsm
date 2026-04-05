<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 预写日志（Write-Ahead Log，WAL）

![Chapter Overview](./lsm-tutorial/week2-06-overview.svg)

在本章中，你将：

* 实现预写日志文件的编解码。
* 在系统重启时从 WAL 恢复 memtable。

将测试用例复制到 starter 代码并运行：

```
cargo x copy-test --week 2 --day 6
cargo x scheck
```

## 任务 1：WAL 编码

本任务需要修改：

```
src/wal.rs
```

上一章我们实现了 manifest 文件，使 LSM 状态可以持久化，并实现了 `close` 函数，在停止引擎前将所有 memtable 刷写到 SST。但如果系统崩溃（如断电）怎么办？我们可以将 memtable 的修改记录到 WAL（预写日志），并在重启数据库时从 WAL 恢复。WAL 只在 `self.options.enable_wal = true` 时启用。

WAL 编码就是一系列键值对：

```
| key_len | key | value_len | value |
```

你还需要实现 `recover` 函数，读取 WAL 并恢复 memtable 的状态。

注意，我们使用 `BufWriter` 来写 WAL。使用 `BufWriter` 可以减少系统调用次数，降低写路径延迟。当用户修改键时，数据不保证立即写入磁盘，引擎只保证在调用 `sync` 时数据已持久化。要正确将数据持久化到磁盘，需要先调用 `flush()` 将 buffer writer 中的数据刷新到文件对象，然后使用 `get_mut().sync_all()` 对文件执行 fsync。注意，*只有*在引擎的 `sync` 被调用时才需要 fsync，写入数据时*不需要*每次都执行 fsync。

## 任务 2：集成 WAL

本任务需要修改：

```
src/mem_table.rs
src/wal.rs
src/lsm_storage.rs
```

`MemTable` 有一个 WAL 字段。如果 `wal` 字段设为 `Some(wal)`，更新 memtable 时需要追加到 WAL。在 LSM 引擎中，如果 `enable_wal = true`，需要创建 WAL。使用 `ManifestRecord::NewMemtable` 记录在创建新 memtable 时更新 manifest。

可以使用 `create_with_wal` 函数创建带 WAL 的 memtable。WAL 应写入存储目录中的 `<memtable_id>.wal`。如果这个 memtable 被刷写为 L0 SST，memtable id 应与 SST id 相同。

## 任务 3：从 WAL 恢复

本任务需要修改：

```
src/lsm_storage.rs
```

如果启用了 WAL，加载数据库时需要基于 WAL 恢复 memtable。还需要实现数据库的 `sync` 函数。`sync` 的基本保证是：引擎确保数据已持久化到磁盘（重启后可以恢复）。为此，只需同步当前 memtable 对应的 WAL 即可。

```
cargo run --bin mini-lsm-cli -- --enable-wal
```

记得从状态中恢复正确的 `next_sst_id`，它应该是 `max{memtable id, sst id}` + 1。在 `close` 函数中，如果设置了 `enable_wal`，不应将 memtable 刷写到 SST，因为 WAL 本身已提供持久性。在关闭数据库前，应等待所有压缩和刷写线程退出。

## 理解检验

* 在引擎中什么时候应该调用 `fsync`？如果每次 put 请求都调用 `fsync` 会怎样（即调用过于频繁）？
* `fsync` 操作在 SSD（固态硬盘）上的代价通常是多少？
* 什么时候可以告诉用户他们的修改（put/delete）已被持久化？
* 如何处理 WAL 中的损坏数据？
* 是否可以设计一个没有 WAL 的 LSM 引擎（即用 L0 作为 WAL）？这种设计有哪些影响？

以上问题不提供参考答案，欢迎在 Discord 社区中讨论。

{{#include copyright.md}}
