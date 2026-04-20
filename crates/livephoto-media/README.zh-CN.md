# livephoto-media

`livephoto-media` 是工作区里专门负责图片和视频输出格式转换规划的 crate。

它是平台无关层，适合处理这类问题：

- 图片输出格式转换，比如 `heic -> jpg`
- 视频输出格式转换，比如 `mov -> mp4`
- 在转码前对比输入格式和目标格式

它不负责 Apple Live Photo、Android Motion Photo 或其他平台桥接格式本身的打包逻辑。

这里的输入/输出格式枚举直接复用了 `livephoto-format` 的定义，这样媒体层支持的格式范围会和 `.livephoto` 本身允许的格式保持一致。

## 职责范围

当前职责：

- 根据 MIME 和扩展名识别图片格式
- 根据 MIME 和扩展名识别视频格式
- 规划输出目标图片和视频格式
- 作为未来转码 backend 的公共落点

推荐分层：

- `livephoto-format`：`.livephoto` 容器格式
- `livephoto-toolkit`：asset 级通用流程
- `livephoto-media`：媒体输出格式转换
- `bridges/apple`、`bridges/android`：平台导入导出

## 示例

```rust
use livephoto_media::{PhotoFormat, PhotoInput, VideoFormat, VideoInput, plan_photo_output, plan_video_output};

let photo_plan = plan_photo_output(
    &PhotoInput {
        mime: Some("image/heic".into()),
        extension: Some("heic".into()),
    },
    Some(PhotoFormat::Jpeg),
)?;

let video_plan = plan_video_output(
    &VideoInput {
        mime: Some("video/quicktime".into()),
        extension: Some("mov".into()),
    },
    Some(VideoFormat::Mp4),
)?;
```

调用方可以通过比较 `photo_plan.input` 和 `photo_plan.output`，以及 `video_plan.input` 和 `video_plan.output`，自行判断是否需要转码。
