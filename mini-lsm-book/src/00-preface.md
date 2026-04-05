<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 前言

![横幅](./mini-lsm-logo.png)

本课程教您如何在 Rust 中构建一个简单的 LSM-Tree 存储引擎。

## 什么是 LSM，为什么是 LSM？

日志结构化合并树是维护键值对的数据结构。这种数据结构在分布式数据库系统中广泛使用，如 [TiDB](https://www.pingcap.com) 和 [CockroachDB](https://www.cockroachlabs.com) 作为其底层存储引擎。[RocksDB](http://rocksdb.org) 基于 [LevelDB](https://github.com/google/leveldb)，是 LSM-Tree 存储引擎的实现。它提供了许多键值访问功能，并在许多生产系统中使用。

一般来说，LSM Tree 是一种追加友好的数据结构。将 LSM 与其他键值数据结构如 RB-Tree 和 B-Tree 进行比较更直观。对于 RB-Tree 和 B-Tree，所有数据操作都是就地进行的。也就是说，当您想要更新键对应的值时，引擎将用新值覆盖其原始内存或磁盘空间。但在 LSM Tree 中，所有写入操作，即插入、更新、删除，都是懒惰地应用于存储。引擎将这些操作批处理到 SST（排序字符串表）文件中并写入磁盘。一旦写入磁盘，引擎将不会直接修改它们。在一个称为压缩的特定后台任务中，引擎将合并这些文件以应用更新和删除。

这种架构设计使 LSM 树易于使用。

1. 数据在持久存储上是不可变的。并发控制更简单。将后台任务（压缩）卸载到远程服务器是可能的。从云原生存储系统如 S3 直接存储和服务数据也是可行的。
2. 改变压缩算法允许存储引擎在读取、写入和空间放大之间平衡。该数据结构是通用的，通过调整压缩参数，我们可以针对不同工作负载优化 LSM 结构。

本课程将教您如何在 Rust 编程语言中构建基于 LSM-Tree 的存储引擎。

## 先决条件

* 您应该了解 Rust 编程语言的基础知识。阅读 [Rust 书](https://doc.rust-lang.org/book/) 就足够了。
* 您应该了解键值存储引擎的基本概念，即为什么我们需要复杂的设计来实现持久性。如果您之前没有数据库系统和存储系统的经验，您可以在 [PingCAP Talent Plan](https://github.com/pingcap/talent-plan/tree/master/courses/rust/projects/project-2) 中实现 Bitcask。
* 了解 LSM 树的基础知识不是必需的，但我们建议您阅读一些相关内容，例如 LevelDB 的整体理念。事先了解它们将使您熟悉可变和不可变内存表、SST、压缩、WAL 等概念。

## 您应该从本课程中期待什么

完成本课程后，您应该深入理解基于 LSM 的存储系统如何工作，获得设计此类系统的实践经验，并将学到的知识应用于您的学习和职业中。您将理解此类存储系统中的设计权衡，并找到设计基于 LSM 的存储系统的优化方式，以满足您的工作负载要求/目标。本课程非常深入，涵盖了现代存储系统（即 RocksDB）的所有基本实现细节和设计选择，基于作者在几个类似 LSM 的存储系统中的经验，您将能够直接在工业界和学术界应用学到的知识。

### 结构

本课程是一个广泛的课程，分为几个部分（周）。每周有七章；您可以在 2 到 3 小时内完成每一章。每部分的头六章将指导您构建一个工作系统，每周的最后一章将是 *小吃时间* 章，在前六天构建的基础上实现一些简单的事情。每章将有必需任务、*检查您的理解* 问题和奖励任务。

### 测试

我们提供完整的测试套件和一些 CLI 工具，用于验证您的解决方案是否正确。请注意，测试套件不是详尽的，您的解决方案在通过所有测试用例后可能不是 100% 正确。当实现系统的后期部分时，您可能需要修复早期错误。我们建议您彻底思考您的实现，特别是当有多线程操作和竞争条件时。

### 解决方案

我们在 mini-lsm 主仓库中有一个实现课程中所有所需功能的解决方案。同时，我们还有一个 mini-lsm 解决方案检查点仓库，其中每个提交对应课程中的一章。

保持这样的检查点仓库与 mini-lsm 课程同步更新是具有挑战性的，因为每个错误修复或新功能必须通过所有提交（或检查点）。因此，此仓库可能不使用最新的启动代码或纳入 mini-lsm 课程的最新功能。

**TL;DR：我们不保证解决方案检查点仓库包含正确的解决方案、通过所有测试或具有正确的文档注释。** 对于正确的实现和实现一切后的解决方案，请查看主仓库中的解决方案。[https://github.com/skyzh/mini-lsm/tree/main/mini-lsm](https://github.com/skyzh/mini-lsm/tree/main/mini-lsm)。

如果您在课程的某个部分卡住了或需要帮助确定在哪里实现功能，您可以参考此仓库以获取帮助。您可以比较提交之间的差异，以了解发生了什么变化。您可能需要在 mini-lsm 课程中多次修改某些函数，并且您可以在此仓库中了解每个章节期望实现的确切内容。

您可以在 [https://github.com/skyzh/mini-lsm-solution-checkpoint](https://github.com/skyzh/mini-lsm-solution-checkpoint) 访问解决方案检查点仓库。

### 反馈

您的反馈非常宝贵。我们在 2024 年基于学生的反馈从头重写了整个课程。请分享您的学习经验，帮助我们不断改进课程。欢迎加入 [Discord 社区](https://skyzh.dev/join/discord) 并分享您的经验。

为什么我们重写它的长故事：课程最初计划为一般指导，学生从空目录开始，根据我们拥有的规范实现他们想要的任何东西。我们有最少的测试来检查行为是否正确。然而，原始课程过于开放式，这对学习体验造成了巨大障碍。由于学生事先没有整个系统的概述，并且指令模糊，有时很难知道为什么做出设计决策以及需要实现什么目标。课程的一些部分过于紧凑，在一章内交付预期内容是不可能的。因此，我们完全重新设计了课程，以获得更容易的学习曲线和更清晰的学习目标。原来的一周课程现在分为两周（第一周关于存储格式，第二周深入压缩），并额外添加了 MVCC 部分。我们希望您发现本课程有趣且对您的学习和职业有帮助。我们要感谢在 [编码第 1 天后的反馈](https://github.com/skyzh/mini-lsm/issues/11) 和 [你好，下一个更新计划是什么？](https://github.com/skyzh/mini-lsm/issues/7) 中发表评论的每个人 -- 您的反馈大大帮助我们改进了课程。

### 许可证

本课程的源代码根据 Apache 2.0 许可证授权，而书籍根据 CC BY-NC-SA 4.0 许可证授权。

### 本课程会永远免费吗？

是的！现在公开可用的所有内容将永远免费，并接收终身更新和错误修复。同时，我们可能会提供付费代码审查和办公时间服务。对于 DLC 部分（*余生* 章节），我们截至 2024 年还没有计划完成它们，并且尚未决定是否会公开可用。

## 社区

您可以加入 skyzh 的 Discord 服务器，与 mini-lsm 社区一起学习。

[![加入 skyzh 的 Discord 服务器](discord-badge.svg)](https://skyzh.dev/join/discord)

## 开始

现在，您可以在 [Mini-LSM 课程概述](./00-overview.md) 中获取 LSM 结构的概述。

## 关于作者

截至撰写时（2024 年初），Chi 从卡内基梅隆大学获得计算机科学硕士学位，从上海交通大学获得学士学位。他曾在各种数据库系统上工作，包括 [TiKV][db1]、[AgateDB][db2]、[TerarkDB][db3]、[RisingWave][db4] 和 [Neon][db5]。自 2022 年以来，他担任 [CMU 数据库系统课程](https://15445.courses.cs.cmu) 的助教三个学期，在 BusTub 教育系统上添加了许多新功能和更多挑战（查看重新设计的 [查询执行](https://15445.courses.cs.cmu.edu/fall2022/project3/) 项目和超级具有挑战性的 [多版本并发控制](https://15445.courses.cs.cmu.edu/fall2023/project4/) 项目）。除了在 BusTub 教育系统上工作，他还维护 [RisingLight](https://github.com/risinglightdb/risinglight) 教育数据库系统。Chi 对探索 Rust 编程语言如何适应数据库世界感兴趣。如果您也对该主题感兴趣，请查看他之前的课程：构建向量化表达式框架 [type-exercise-in-rust](https://github.com/skyzh/type-exercise-in-rust) 和构建向量数据库 [write-you-a-vector-db](https://github.com/skyzh/write-you-a-vector-db)。

[db1]: https://github.com/tikv/tikv
[db2]: https://github.com/tikv/agatedb
[db3]: https://github.com/bytedance/terarkdb
[db4]: https://github.com/risingwavelabs/risingwave
[db5]: https://github.com/neondatabase/neon

{{#include copyright.md}}
