# LivePhoto 容器格式 v1

状态：Draft

文档版本：0.1.0

目标实现语言：Rust

文件扩展名：`.livephoto`

建议 MIME 类型：`application/x-livephoto`

文件魔数：`LPHF`

## 1. 范围

本文档定义一种用于动态照片的单文件二进制容器格式。

该格式的目标是支持：

- 单文件存储与传输
- 跨平台解析与播放
- 封装封面静态图与短视频动态片段
- 支持未来扩展的元数据与附加块
- 通过桥接层与 Apple Live Photo、Android Motion Photo 生态互导

该格式不追求与苹果原生 Live Photo 表示形式二进制兼容。与苹果生态的兼容应通过导入导出适配器实现。

## 2. 设计目标

该格式按以下优先级设计：

1. `.livephoto` 文件必须是自包含的。
2. 解析器必须能够在不扫描全文件的前提下定位核心资源。
3. 封面图必须具有明确的语义地位。
4. 容器必须支持平滑演进，而不破坏 v1 读取器。
5. Rust 实现应尽量直接、显式、安全。
6. 非媒体元数据必须与编码媒体流解耦。

## 3. 高层模型

一个 `.livephoto` 文件包含：

- 一个必选 manifest 块
- 一个必选主封面图块
- 一个必选主动态视频块
- 零个或多个可选块，用于缩略图、EXIF、XMP、桥接元数据、厂商扩展等

语义模型如下：

- 封面图是默认静态表示
- 动态视频是动态表示
- manifest 定义时间信息、尺寸信息、默认播放策略以及各块之间的关系
- 所有二进制资源都以带类型的 chunk 形式存储，容器本身不重定义媒体编码

## 4. 二进制约定

### 4.1 字节序

容器头、目录表、chunk 头中的所有整数字段均使用小端序。

### 4.2 对齐

所有 chunk 必须按 8 字节对齐。

写入器：

- 必须将每个 chunk 的 payload 填充到下一个 8 字节边界
- 必须将 `stored_length` 记录为未填充前的真实 payload 长度
- 必须使用零字节填充

读取器：

- 必须使用 `stored_length` 解析 payload
- 必须忽略填充字节

### 4.3 字符编码

- 固定长度标识符使用 ASCII
- JSON 内容使用 UTF-8
- 未来新增的二进制结构中的字符串字段，除非特别说明，否则使用 UTF-8

### 4.4 时间单位

manifest 中所有时长与时间戳字段，除非明确说明，否则统一使用毫秒。

### 4.5 UUID 表示

凡是在 JSON 中出现的 UUID，必须使用标准小写连字符形式，例如：

`"550e8400-e29b-41d4-a716-446655440000"`

本节仅规定“当某个值是 UUID 时应采用的 JSON 文本表示形式”，
并不意味着格式中的所有标识符字段都必须是 UUID。

## 5. 文件布局

顶层布局如下：

```text
+-------------------+
| FileHeader        |
+-------------------+
| Chunk 0           |
+-------------------+
| Chunk 1           |
+-------------------+
| ...               |
+-------------------+
| Chunk N           |
+-------------------+
| TOC Chunk         |
+-------------------+
```

`TOCC` 本身也是普通 chunk，只是类型保留且必须且仅能出现一次。

在 v1 中，建议将 TOC 放在文件末尾，这样写入器可以先顺序写资源，再最终回填索引。

## 6. 文件头

文件以固定 68 字节的 header 开始。

### 6.1 二进制布局

```c
struct LPFileHeaderV1 {
    char     magic[4];             // "LPHF"
    uint16_t major_version;        // 1
    uint16_t minor_version;        // 首个稳定版建议为 0
    uint32_t header_size;          // v1 中必须为 68
    uint64_t flags;                // 文件级 flags
    uint64_t toc_offset;           // TOC chunk header 的绝对偏移
    uint64_t toc_length;           // TOC chunk 总长度，含 header 与 padding
    uint64_t file_size;            // 文件总大小
    uint64_t primary_manifest_id;  // 主 META chunk 的 chunk id
    uint64_t reserved[2];          // v1 中必须为 0
};
```

### 6.2 Header 规则

写入器：

- 必须写入 `magic = "LPHF"`
- 必须写入 `major_version = 1`
- 建议写入 `minor_version = 0`
- 必须写入 `header_size = 68`
- 必须将所有保留字段写为 0
- 必须保证 `toc_offset`、`toc_length`、`file_size` 一致
- 必须将 `primary_manifest_id` 指向必选 `META` chunk 的 id

读取器：

- 必须拒绝非法 magic
- 必须拒绝 `header_size != 68`
- 必须拒绝 `toc_offset == 0`
- 必须拒绝 `file_size < 68`
- 应拒绝不一致的偏移和长度字段
- 在严格模式下，可拒绝非零保留字段

### 6.3 文件级 Flags

文件 flags 是一个 `u64` 位图。

v1 中已定义位：

- bit 0：文件包含加密 chunk
- bit 1：文件包含 Apple Live Photo 桥接元数据
- bit 2：文件包含 Android Motion Photo 桥接元数据

其余位在 v1 中保留，写入器必须写 0。

## 7. Chunk 模型

每个 chunk 由固定长度 header 与可变长度 payload 组成。

### 7.1 Chunk Header

```c
struct LPChunkHeaderV1 {
    char     chunk_type[4];        // ASCII，例如 "META"、"PHOT"、"VIDE"
    uint16_t chunk_version;        // chunk 自身 schema 版本，通常为 1
    uint16_t header_size;          // v1 中必须为 48
    uint64_t chunk_id;             // 文件内唯一
    uint64_t flags;                // chunk 级 flags
    uint64_t stored_length;        // payload 实际长度，不含 padding
    uint64_t crc32c;               // v1 中仅低 32 位有效，高位写 0
    uint64_t reserved;             // v1 中必须为 0
};
```

chunk header 在 v1 中固定为 48 字节。

写入器：

- 必须写入 `header_size = 48`

读取器：

- 必须拒绝 `header_size != 48`

### 7.2 Chunk 顺序

chunk 顺序本身不具有语义意义，但以下规则必须满足：

- `file_header.toc_offset` 必须指向一个 canonical `TOCC`
- 主资源所需的 `META`、`PHOT`、`VIDE` 必须存在
- `TOCC` 应放在最后

严格模式读取器必须拒绝包含额外非 canonical `TOCC` 的文件。
恢复模式读取器可以忽略额外的非 canonical `TOCC`。

建议写入顺序：

1. `META`
2. `PHOT`
3. `VIDE`
4. 可选 chunk
5. `TOCC`

### 7.3 Chunk Flags

chunk flags 是一个 `u64` 位图。

v1 中已定义位：

- bit 0：该 chunk 为主播放流程必需
- bit 1：payload 已压缩
- bit 2：payload 已加密
- bit 3：保留
- bit 4：该 chunk 只包含桥接元数据，不参与直接播放

读取器在非严格模式下应忽略未知位。

### 7.4 完整性

`crc32c` 在 v1 中是可选字段。

规则：

- 若 `crc32c == 0`，表示未提供该 chunk 的校验值
- 若非零，读取器应基于未填充 payload 验证 CRC32C
- 写入器要么写入正确 CRC32C，要么写 0

## 8. 目录表 Chunk

目录表 chunk 类型为 `TOCC`。

其 payload 是二进制索引，用于快速定位所有 chunk。

### 8.1 TOC Payload 布局

```c
struct LPToCPayloadV1 {
    uint32_t entry_count;
    uint32_t reserved0;
    LPToCEntryV1 entries[entry_count];
};

struct LPToCEntryV1 {
    uint64_t chunk_id;
    char     chunk_type[4];
    uint32_t reserved1;
    uint64_t file_offset;          // chunk header 的绝对偏移
    uint64_t total_length;         // chunk 总长度，含 header、payload、padding
    uint64_t stored_length;        // payload 长度，不含 padding
    uint64_t flags;
};
```

### 8.2 TOC 要求

写入器：

- 必须将所有非 TOC chunk 写入 TOC
- 不得将 canonical `TOCC` 自身写入目录项
- 必须保证 chunk id 全局唯一

读取器：

- 应优先使用 TOC 导航
- 必须将 `file_header.toc_offset` 指向的 `TOCC` 视为 canonical TOC
- 必须拒绝 `chunk_type == TOCC` 的 canonical TOC entry
- 若 TOC 校验失败，在恢复模式下可以回退为线性扫描

厂商自定义的辅助索引必须使用 `VEND` 或其他非 `TOCC` chunk type。

## 9. 必选 Chunk 类型

### 9.1 `META` Manifest Chunk

`META` 的 payload 是 UTF-8 JSON。

它定义逻辑关系、播放默认值与元数据语义。

v1 中必须存在且仅存在一个主 `META`。

#### 9.1.1 必选字段示例

```json
{
  "schema": "livephoto/v1",
  "asset_id": "asset-demo-01",
  "created_at_ms": 1776038462000,
  "duration_ms": 1800,
  "width": 1440,
  "height": 1920,
  "cover_timestamp_ms": 900,
  "photo_chunk_id": 2,
  "video_chunk_id": 3,
  "photo_mime": "image/jpeg",
  "video_mime": "video/mp4",
  "has_audio": true,
  "playback": {
    "autoplay": false,
    "loop": false,
    "bounce": false,
    "muted_by_default": false,
    "return_to_cover": true
  }
}
```

#### 9.1.2 Manifest 字段定义

v1 中定义以下根对象字段。

| 名称 | 类型 | 是否必须 | 说明 |
| --- | --- | --- | --- |
| `schema` | 字符串字面量 | 是 | 必须为 `"livephoto/v1"`。 |
| `asset_id` | string | 是 | 逻辑资源的不透明唯一标识。推荐使用 UUID，但非强制；若使用 UUID，必须符合 4.5 节规定的标准小写连字符形式。 |
| `created_at_ms` | 整数（`u64`） | 是 | 逻辑创建时间戳，单位毫秒。 |
| `duration_ms` | 整数（`u64`） | 是 | 动态视频总时长，单位毫秒。必须大于 0。 |
| `width` | 整数（`u32`） | 是 | 显示宽度，单位像素。必须大于 0。 |
| `height` | 整数（`u32`） | 是 | 显示高度，单位像素。必须大于 0。 |
| `cover_timestamp_ms` | 整数（`u64`） | 是 | 封面图在动态片段中对应的时间点，单位毫秒。不得超过 `duration_ms`。 |
| `photo_chunk_id` | 整数（`u64`） | 是 | 主封面图 chunk 的 id。必须引用主 `PHOT` chunk。 |
| `video_chunk_id` | 整数（`u64`） | 是 | 主动态视频 chunk 的 id。必须引用主 `VIDE` chunk。 |
| `photo_mime` | string | 是 | 主封面图 payload 的 MIME 类型。支持值见 9.2 节。 |
| `video_mime` | string | 是 | 主动态视频 payload 的 MIME 类型。支持值见 9.3 节。 |
| `has_audio` | boolean | 是 | 主动态视频是否包含可听音频。 |
| `playback` | object | 是 | 播放策略对象。详见 9.1.3 节。 |
| `title` | string | 否 | 可选的人类可读标题。 |
| `description` | string | 否 | 可选的人类可读描述。 |
| `author` | string | 否 | 可选的作者或创建者标识。 |
| `timezone` | string | 否 | 与逻辑拍摄上下文相关的可选时区提示。 |
| `rotation_degrees` | 整数（`i32`） | 否 | 可选的显示旋转元数据，单位为度。 |
| `thumbnail_chunk_id` | 整数（`u64`） | 否 | 可选缩略图 chunk 的 id。必须引用 `THMB` chunk。 |
| `exif_chunk_id` | 整数（`u64`） | 否 | 可选 EXIF 元数据 chunk 的 id。必须引用 `EXIF` chunk。 |
| `xmp_chunk_id` | 整数（`u64`） | 否 | 可选 XMP packet chunk 的 id。必须引用 `XMP_` chunk。 |
| `apple_bridge_chunk_id` | 整数（`u64`） | 否 | 可选 Apple bridge 元数据块的 chunk id。必须引用 `APPL` chunk。 |
| `android_bridge_chunk_id` | 整数（`u64`） | 否 | 可选 Android bridge 元数据块的 chunk id。必须引用 `ANDR` chunk。 |
| `bridges` | 对象数组 | 否 | 可选的结构化 bridge 描述符列表。数组元素为下文定义的 `BridgeDescriptorV1` 对象。 |
| `tags` | 字符串数组 | 否 | 可选的自由标签列表。 |
| `capture_device` | string | 否 | 可选的拍摄设备标识或型号提示。 |
| `software` | string | 否 | 可选的写入器、导入器或源软件标识。 |
| `color_space` | string | 否 | 可选的主展示色彩空间提示。 |
| `alpha_mode` | 字符串枚举 | 否 | 可选的 alpha 解释提示。允许值见下文。 |
| `poster_strategy` | 字符串枚举 | 否 | 可选的封面选择策略提示。允许值见下文。 |
| `preferred_seek_pre_roll_ms` | 整数（`u64`） | 否 | 可选播放器提示，表示在 seek 到目标位置前建议预解码的时长，单位毫秒。 |
| `extensions` | `string -> 任意 JSON 值` 的对象映射 | 否 | 可选的厂商扩展或未来兼容字段。读取器必须忽略未知键。 |

`alpha_mode` 在 v1 中允许的值：

- `"none"`
- `"straight"`
- `"premultiplied"`

`poster_strategy` 在 v1 中允许的值：

- `"explicit"`
- `"video_frame"`
- `"generated"`

`BridgeDescriptorV1` 对象字段：

| 名称 | 类型 | 是否必须 | 说明 |
| --- | --- | --- | --- |
| `target` | string | 是 | bridge 目标标识，例如 `"apple-live-photo"` 或 `"android-motion-photo"`。 |
| `chunk_id` | 整数（`u64`） | 是 | 对应 bridge 元数据 chunk 的 id。 |

#### 9.1.3 Playback 对象

`playback` 在 v1 中定义的字段：

| 名称 | 类型 | 是否必须 | 说明 |
| --- | --- | --- | --- |
| `autoplay` | boolean | 是 | 资源展示时是否应自动开始播放。 |
| `loop` | boolean | 是 | 到达末尾后是否应从头重新播放。 |
| `bounce` | boolean | 是 | 是否在支持时尝试正向后再反向播放。 |
| `muted_by_default` | boolean | 是 | 若存在音频，播放开始时是否默认静音。 |
| `return_to_cover` | boolean | 是 | 播放结束后是否应恢复到封面图表示。 |
| `hold_last_frame` | boolean | 否 | 若为 `true` 且 `return_to_cover == false`，播放器可以停留在最后一帧。 |
| `interaction_hint` | 字符串枚举 | 否 | 可选的交互触发提示。允许值见下文。 |

`interaction_hint` 在 v1 中允许的值：

- `"tap"`
- `"hover"`
- `"press"`
- `"viewport"`
- `"programmatic"`

#### 9.1.4 Manifest 校验规则

读取器应至少校验：

- `duration_ms > 0`
- `width > 0`
- `height > 0`
- `cover_timestamp_ms <= duration_ms`
- `photo_chunk_id != video_chunk_id`
- 若能探测媒体实际格式，则 `photo_mime` 与 `video_mime` 应与实际内容匹配

### 9.2 `PHOT` 主封面图 Chunk

`PHOT` 存储主静态图。

payload：

- 原始编码图片字节

v1 支持的 MIME 类型：

- `image/jpeg`
- `image/heic`
- `image/heif`
- `image/avif`
- `image/png`
- `image/webp`

写入器优先建议：

- 为兼容性优先使用 `image/jpeg`
- 若考虑 Apple bridge，可优先使用 `image/heic`

读取器必须：

- 将 `PHOT` 视为不透明编码字节流
- 除 manifest 声明外，不应假设其具体编码

### 9.3 `VIDE` 主动态视频 Chunk

`VIDE` 存储主动态视频。

payload：

- 原始编码视频容器字节

v1 支持的 MIME 类型：

- `video/mp4`
- `video/quicktime`
- `video/webm`

写入器优先建议：

- 为最大兼容性优先使用 `video/mp4`，编码建议 H.264/AAC

读取器必须：

- 将 `VIDE` 视为不透明编码字节流
- 不应仅根据 `.livephoto` 容器推断其内部 sample 结构

## 10. 可选 Chunk 类型

### 10.1 `THMB` 缩略图 Chunk

存储低成本预览图。

payload：

- 原始编码图片字节

适用场景：

- 列表页
- 快速预览
- 服务端索引

### 10.2 `EXIF` 元数据 Chunk

用于存储 EXIF 信息，可采用以下两种表示之一：

- 原始 EXIF block
- UTF-8 JSON 表示

manifest 应通过 `extensions` 指明表示形式，例如：

```json
{
  "extensions": {
    "exif_format": "raw"
  }
}
```

或：

```json
{
  "extensions": {
    "exif_format": "json"
  }
}
```

若需要保留源文件完整性，建议优先保留 raw 表示。

### 10.3 `XMP_` XMP 元数据 Chunk

用于存储 XMP packet，内容为 UTF-8 XML。

由于 chunk type 固定为 4 字节 ASCII，因此采用 `XMP_` 表示。

### 10.4 `APPL` Apple Bridge Chunk

用于保存导出 Apple Live Photo 所需的桥接信息。

建议字段：

```json
{
  "asset_identifier": "550e8400-e29b-41d4-a716-446655440000",
  "still_image_time_ms": 900,
  "photo_codec_hint": "image/heic",
  "video_codec_hint": "video/quicktime",
  "maker_apple_key_17": "550e8400-e29b-41d4-a716-446655440000",
  "quicktime_content_identifier": "550e8400-e29b-41d4-a716-446655440000"
}
```

该 chunk 不能让 `.livephoto` 文件被 Apple Photos 直接识别为原生 Live Photo。
它仅用于保留导出桥接时所需的信息。

### 10.5 `ANDR` Android Bridge Chunk

用于保存导出 Android Motion Photo 所需的桥接信息。

建议字段：

```json
{
  "presentation_timestamp_us": 900000,
  "xmp_format": "container",
  "primary_image_role": "display",
  "embedded_video_role": "motion"
}
```

### 10.6 `VEND` 厂商扩展 Chunk

用于存储实现方私有数据。

若 payload 使用 JSON，建议在顶层携带厂商命名空间标识。

读取器可以忽略未知的可选 chunk。

## 11. MIME 与编码规则

该容器本身不定义图像或视频编码。
它只负责承载已经编码好的媒体 payload。

规则如下：

- manifest 必须声明 `photo_mime` 与 `video_mime`
- 读取器在可行时应对 payload 做格式探测，并在不匹配时警告
- 写入器不应依赖 payload 内部文件扩展名判断格式

## 12. 播放语义

该格式明确区分“存储结构”与“播放语义”。

### 12.1 默认渲染

默认状态下：

- 显示封面图
- 除非 `playback.autoplay == true`，否则不自动播放
- 若存在音频，则遵循 `muted_by_default`

### 12.2 封面语义

`cover_timestamp_ms` 用于定义封面图与动态视频之间的精确语义关系。

用法：

- 若封面图从视频抽帧得到，则该值指向源帧时间点
- 若封面图独立拍摄，则该值指向视频中最接近该静态语义的时间点
- 播放器在回退到封面语义时，应以该时间点作为参考

### 12.3 播放结束策略

若 `return_to_cover == true`：

- 播放器在播放结束后应恢复到封面表现

若 `hold_last_frame == true` 且 `return_to_cover == false`：

- 播放器可停留在最终解码帧

若 `bounce == true`：

- 播放器可以执行正放后反放
- 若不支持反向播放，可以忽略该字段

## 13. 兼容性策略

### 13.1 前向兼容

读取器应：

- 拒绝未知 `major_version`
- 忽略未知可选 chunk
- 忽略未知 manifest 字段
- 在可行时于无损重写中保留未知 chunk

### 13.2 向后兼容

未来的 v1.x 修订必须：

- 保持 68 字节文件头契约不变
- 保持 48 字节 chunk 头契约不变
- 不得通过增大 `header_size` 在 v1.x 内引入头部扩展
- 不改变已有必选字段的语义
- 仅新增可选字段或新的可选 chunk 类型

## 14. 错误处理

Rust 中建议定义如下错误类别：

- invalid magic
- unsupported major version
- malformed header
- malformed TOC
- duplicate or non-canonical `TOCC`
- duplicate chunk id
- required chunk missing
- manifest parse failure
- manifest validation failure
- checksum mismatch
- invalid offset or length
- unsupported codec
- I/O failure

严格模式应在以下情况直接失败：

- header 字段非法
- 缺失必选 chunk
- manifest 不一致
- chunk 偏移重叠
- 存在额外的非 canonical `TOCC`
- canonical TOC entry 引用了 `TOCC`

恢复模式可以：

- 在 TOC 损坏时回退为线性扫描
- 忽略非法可选 chunk
- 忽略缺失的 checksum

## 15. 流式读取与随机访问

v1 优先面向文件读取，而不是实时流式传输。

但它仍具备以下能力：

- header 可直接定位 TOC
- TOC 可直接定位各 chunk 偏移
- 解析器可通过内存映射借用 payload 切片

写入器不应假设所有 payload 都足够小，可以一次性完整载入内存。

## 16. 安全性考虑

读取器必须：

- 在分配内存前校验所有长度字段
- 校验所有偏移是否落在文件边界内
- 拒绝重叠或越界的 chunk 区域
- 为 JSON manifest 设定大小上限
- 不应无条件信任 MIME 声明
- 将内嵌媒体解码视为不可信输入

v1 建议的实现限制：

- manifest 最大大小：1 MiB
- 缩略图最大大小：32 MiB
- 文件最大大小：由实现定义，但必须全程使用 checked arithmetic

## 17. Rust 映射建议

建议参考以下 Rust 类型：

```rust
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FileHeaderV1 {
    pub magic: [u8; 4],
    pub major_version: u16,
    pub minor_version: u16,
    pub header_size: u32,
    pub flags: u64,
    pub toc_offset: u64,
    pub toc_length: u64,
    pub file_size: u64,
    pub primary_manifest_id: u64,
    pub reserved: [u64; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ChunkHeaderV1 {
    pub chunk_type: [u8; 4],
    pub chunk_version: u16,
    pub header_size: u16,
    pub chunk_id: u64,
    pub flags: u64,
    pub stored_length: u64,
    pub crc32c: u64,
    pub reserved: u64,
}
```

建议的解析后模型：

```rust
#[derive(Debug, Clone)]
pub struct LivePhotoFile {
    pub header: FileHeaderV1,
    pub chunks: Vec<ChunkRecord>,
    pub manifest: ManifestV1,
}

#[derive(Debug, Clone)]
pub struct ChunkRecord {
    pub header: ChunkHeaderV1,
    pub file_offset: u64,
    pub total_length: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ManifestV1 {
    pub schema: String,
    pub asset_id: String,
    pub created_at_ms: u64,
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    pub cover_timestamp_ms: u64,
    pub photo_chunk_id: u64,
    pub video_chunk_id: u64,
    pub photo_mime: String,
    pub video_mime: String,
    pub has_audio: bool,
    pub playback: PlaybackPolicyV1,
    #[serde(default)]
    pub extensions: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlaybackPolicyV1 {
    pub autoplay: bool,
    pub loop: bool,
    pub bounce: bool,
    pub muted_by_default: bool,
    pub return_to_cover: bool,
    #[serde(default)]
    pub hold_last_frame: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction_hint: Option<String>,
}
```

## 18. Writer 算法建议

推荐写入流程：

1. 校验输入图片与视频资源。
2. 分配稳定的 chunk id。
3. 生成带完整引用关系的 manifest JSON。
4. 先写固定文件头，占位 TOC 与文件大小字段。
5. 顺序写入 `META`、`PHOT`、`VIDE` 及可选 chunk。
6. 记录每个 chunk 的偏移和总长度。
7. 构造并写入只包含非 `TOCC` 目录项的 canonical `TOCC` chunk。
8. 回填文件头。
9. 除非显式关闭，写入器应默认写入有效的 `crc32c` 值。

## 19. Reader 算法建议

推荐读取流程：

1. 读取并校验固定 header。
2. 跳转到 `toc_offset`。
3. 解析并校验 canonical `TOCC`。
4. 通过 `primary_manifest_id` 找到主 `META`。
5. 解析 manifest JSON。
6. 通过 `photo_chunk_id` 和 `video_chunk_id` 解析主封面图与主视频。
7. 向上层暴露 payload 切片、reader 或 blob 源。

严格模式读取器之后应线性扫描全文件，确认不存在额外的 `TOCC`。

## 20. 最小合法文件

一个最小合法 `.livephoto` 文件必须包含：

- 合法文件头
- 一个 `META`
- 一个 `PHOT`
- 一个 `VIDE`
- 一个 `TOCC`

缩略图、EXIF、XMP、桥接 metadata 都不是必需的。

## 21. 示例 Manifest

```json
{
  "schema": "livephoto/v1",
  "asset_id": "asset-demo-01",
  "created_at_ms": 1776070800000,
  "duration_ms": 1500,
  "width": 1080,
  "height": 1440,
  "cover_timestamp_ms": 800,
  "photo_chunk_id": 2,
  "video_chunk_id": 3,
  "photo_mime": "image/jpeg",
  "video_mime": "video/mp4",
  "has_audio": true,
  "playback": {
    "autoplay": false,
    "loop": false,
    "bounce": false,
    "muted_by_default": false,
    "return_to_cover": true,
    "hold_last_frame": false,
    "interaction_hint": "press"
  },
  "title": "sample asset",
  "software": "livephoto-rs/0.1.0",
  "extensions": {
    "thumbnail_mime": "image/jpeg"
  }
}
```

## 22. 后续建议扩展

以下能力故意留到后续版本：

- chunk 级压缩封装
- chunk 级加密封装
- chunk 级完整性哈希
- detached signatures
- 多音轨
- 字幕或 caption track
- 除视频外的图像序列承载能力
- edit decision list
- 使用 CBOR 等二进制 manifest 编码
- 面向同步系统的 delta-friendly chunk 排布

## 23. 实践建议

第一版 Rust 实现建议采用：

- manifest 使用 JSON
- `PHOT` 优先用 `image/jpeg` 或 `image/heic`
- `VIDE` 优先用 `video/mp4`
- `serde_json` 解析 manifest
- 默认启用 `crc32c` 做 per-chunk 完整性校验

首个 v1 里程碑不建议一开始就做：

- 加密
- chunk 级哈希或签名
- 多版本媒体自动选择
- 对未知 chunk 的有损重写

## 24. 建议仓库结构

若用 Rust 实现，建议从以下结构开始：

```text
livephoto/
  crates/
    livephoto-format/     # 二进制结构、parser、writer
    livephoto-codec/      # bridge 与媒体辅助逻辑
    livephoto-player/     # 播放抽象层
    livephoto-cli/        # pack、unpack、inspect
  docs/
    livephoto-format-v1.md
    livephoto-format-v1.zh-CN.md
```

## 25. 总结

`.livephoto` v1 是一种基于 chunk 的二进制容器格式，具有：

- 固定 68 字节文件头
- 固定 48 字节 chunk 头
- 必选的 `META`、`PHOT`、`VIDE`、`TOCC`
- 基于 JSON manifest 的资源关系定义
- 将媒体 payload 作为不透明编码资源承载
- 明确的前向兼容策略

它的目标是在足够容易落地的前提下，为 Rust 首版实现提供稳定基础，并为后续向更完整的媒体容器演进预留空间。
