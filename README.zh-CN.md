# livephoto

`livephoto` 是一个跨平台的动态照片容器项目。

它定义了一种单文件 `.livephoto` 格式，用于封装：

- 一张封面图
- 一段短视频
- manifest 元数据
- 可选的桥接和扩展元数据

这个项目的目标不是复刻苹果私有的 Live Photo 表示方式，而是提供一个开放的、可实现的、单文件的动态照片容器层，以及围绕它的工具链，服务于 Web、移动端、桌面端和服务端系统。

## 为什么要做这个项目

现有动态照片方案长期处于碎片化状态。

- Apple Live Photo 在苹果生态中体验很好，但本质上仍然是配对资源模型。
- Android Motion Photo 更接近单文件思路，但不同实现之间仍然存在差异。
- 很多业务系统最终只能把动态照片当成“图片 + 视频”的临时组合，而不是正式媒体对象。

这个项目希望让动态照片更容易：

- 存储
- 传输
- 检查
- 跨生态桥接
- 脱离原厂框架播放

## 仓库当前包含什么

- `docs/livephoto-format-v1.md`
  英文格式规范
- `docs/livephoto-format-v1.zh-CN.md`
  中文格式规范
- `docs/project-introduction.md`
  英文项目介绍
- `docs/project-introduction.zh-CN.md`
  中文项目介绍
- `crates/livephoto-format`
  `.livephoto` 格式的 Rust 实现
- `crates/livephoto-cli`
  用于打包、检查、解包 `.livephoto` 的命令行工具
- `web/`
  最小 Web 播放器

## 当前能力

当前实现已经支持：

- 文件头、chunk 头和 TOC 的读写
- `META`、`PHOT`、`VIDE`、`TOCC` 的核心语义
- 缩略图、EXIF、XMP、哈希、Apple bridge、Android bridge、备用版本、签名、厂商扩展和未知 chunk 等可选块
- manifest 校验
- checksum 校验
- TOC 损坏时的恢复模式扫描
- 重写过程中对未知可选 chunk 的无损保留
- 一个可读取本地 `.livephoto` 文件的最小浏览器播放器

## 快速开始

### 构建与检查

```bash
cd /Users/nailao/Code/livephoto
cargo test
cargo clippy --workspace --all-targets -- -D warnings
```

### 打包 `.livephoto`

```bash
cd /Users/nailao/Code/livephoto

cargo run -p livephoto-cli -- \
  pack \
  --manifest /path/to/manifest.json \
  --photo /path/to/photo.heic \
  --video /path/to/video.mov \
  --out /path/to/output.livephoto
```

### 检查 `.livephoto`

```bash
cd /Users/nailao/Code/livephoto

cargo run -p livephoto-cli -- inspect /path/to/output.livephoto
```

### 解包 `.livephoto`

```bash
cd /Users/nailao/Code/livephoto

cargo run -p livephoto-cli -- \
  unpack \
  /path/to/output.livephoto \
  --out-dir /path/to/unpacked
```

### 运行最小 Web 播放器

```bash
cd /Users/nailao/Code/livephoto/web
python3 -m http.server 8080
```

然后打开：

[http://localhost:8080](http://localhost:8080)

## 格式要点

当前格式是基于 chunk 的二进制容器，采用：

- magic: `LPHF`
- 小端整数编码
- 固定 68 字节文件头
- 固定 48 字节 chunk 头
- 8 字节 chunk payload 对齐
- `META` 使用 JSON manifest

完整规范见：

- [English spec](./docs/livephoto-format-v1.md)
- [中文规范](./docs/livephoto-format-v1.zh-CN.md)

## 项目定位

这个项目应被理解为：

- 一个跨平台动态照片容器
- 一套面向动态照片的解析与生成工具链
- 一个连接 Apple Live Photo、Android Motion Photo 与自定义播放环境的桥接层

它不应被理解为：

- 苹果原生 Live Photo 格式
- 系统相册的替代品
- 自定义图片编码或视频编码

## 后续方向

比较合理的下一步包括：

- 更完整的 Web 播放控制
- iOS 和 Android SDK
- Apple / Android 导入导出桥接工具
- 封面替换、裁剪、元数据更新等编辑能力
- 面向服务端的转码和检查流程

## License

MIT，见 [LICENSE](./LICENSE)。
