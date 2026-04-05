<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 环境设置

启动代码和参考解决方案可在 [https://github.com/skyzh/mini-lsm](https://github.com/skyzh/mini-lsm) 获取。

## 安装 Rust

请参见 [https://rustup.rs](https://rustup.rs) 获取更多信息。

## 克隆仓库

```
git clone https://github.com/skyzh/mini-lsm
```

## 启动代码

```
cd mini-lsm/mini-lsm-starter
code .
```

## 安装工具

您需要最新的稳定版 Rust 来编译此项目。最低要求是 `1.74`。

```
cargo x install-tools
```

## 运行测试

```
cargo x copy-test --week 1 --day 1
cargo x scheck
```

现在，您可以开始 [第 1 周：Mini-LSM](./week1-overview.md)。

{{#include copyright.md}}
