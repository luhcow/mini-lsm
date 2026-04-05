<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 第 1 周概览：Mini-LSM

![Chapter Overview](./lsm-tutorial/week1-overview.svg)

在课程第 1 周，你将为存储引擎构建必要的存储格式，实现系统的读路径和写路径，并最终得到一个可用的基于 LSM 的键值存储。本部分共有 7 章（天）。

* [第 1 天：Memtable](./week1-01-memtable.md)。你将实现系统的内存读写路径。
* [第 2 天：合并迭代器](./week1-02-merge-iterator.md)。你将扩展第 1 天的成果，为系统实现 `scan` 接口。
* [第 3 天：块编码](./week1-03-block.md)。现在开始构建磁盘结构的第一步，实现块的编解码。
* [第 4 天：SST 编码](./week1-04-sst.md)。SST 由块组成，到这一天结束时，你将拥有 LSM 磁盘结构的基本构建块。
* [第 5 天：读路径](./week1-05-read-path.md)。现在既有内存结构又有磁盘结构，可以将它们组合起来，为存储引擎构建完整可用的读路径。
* [第 6 天：写路径](./week1-06-write-path.md)。第 5 天的测试框架负责生成这些结构，第 6 天你将自己控制 SST 的刷写。你将实现刷写到 L0 SST 的功能，存储引擎也随之完整。
* [第 7 天：SST 优化](./week1-07-sst-optimizations.md)。我们将实现几项 SST 格式优化，提升系统性能。

本周结束时，你的存储引擎应该能够处理所有 get/scan/put 请求。唯一缺少的部分是将 LSM 状态持久化到磁盘，以及一种更高效的磁盘 SST 组织方式。你将拥有一个可用的 **Mini-LSM** 存储引擎。

{{#include copyright.md}}
