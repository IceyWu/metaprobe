<p align="center">
  <img src="./assets/metaprobe-logo.png" width="156" alt="metaprobe 徽标" />
</p>

<h1 align="center">metaprobe</h1>

<p align="center">
  基于 Rust 的图片与视频元数据解析库。<br />
  一套核心解析逻辑，同时支持 Node.js 原生绑定和浏览器 WASM。
</p>

<p align="center">
  <a href="./README.md">English</a> · 简体中文
</p>

<p align="center">
  <img src="https://img.shields.io/badge/license-MIT-7c3aed" alt="MIT license" />
  <img src="https://img.shields.io/badge/core-Rust-f97316" alt="Rust 核心" />
  <img src="https://img.shields.io/badge/runtime-Node.js%20%7C%20Browser-06b6d4" alt="Node.js 和浏览器" />
  <img src="https://img.shields.io/badge/formats-content--detected%20image%20%26%20video%20containers-2563eb" alt="按内容识别图片和视频容器" />
  <img src="https://img.shields.io/badge/npm-metaprobe-CB3837?logo=npm&logoColor=white" alt="npm 包 metaprobe" />
  <img src="https://img.shields.io/badge/npm%20version-0.0.1-CB3837?logo=npm&logoColor=white" alt="npm 版本 0.0.1" />
  <img src="https://img.shields.io/badge/npm%20status-not%20published-lightgrey?logo=npm&logoColor=white" alt="npm 尚未发布" />
</p>

`metaprobe` 不依赖 `ffprobe`、MediaInfo、ExifTool 或其他外部二进制程序，
即可解析图片和视频元数据。所有解析逻辑集中在 Rust 核心中，并通过 Node.js
N-API 和浏览器 WASM 两种适配层对外提供能力。

## 功能特性

- 优先根据文件内容识别格式，扩展名只作为兜底提示。
- 支持常见图片格式，包括 JPEG、PNG、GIF、BMP、TIFF/RAW、WebP、ICO、SVG、
  JPEG 2000、JPEG XL、HEIC、AVIF、CR3 等。
- 支持常见视频容器，包括 MOV/MP4、AVI、WebM/MKV、FLV、MPEG-TS、MPEG
  程序流、Ogg 等。
- 已识别的容器不要求固定文件后缀；暂时无法提取细节时会安全返回容器级结果，
  不会因为后缀不同直接拒绝。
- 图片支持 EXIF、GPS、ICC Profile、色彩空间、尺寸和 SHA-256。
- 视频支持 QuickTime/ISO-BMFF 元数据、拍摄时间、容器时间、编码器、时长、
  帧率和码率。
- Node.js 原生 API 支持文件路径和并行批量解析。
- WASM API 支持 `Uint8Array`、快速解析、可选哈希和批量解析。
- Swift Package 支持 iOS，底层使用相同的 Rust 解析核心，可解析照片和视频元数据。
- 核心元数据解析路径没有运行时 JavaScript 解析依赖。

## 安装

首次发布到 npm 后：

```bash
pnpm add metaprobe
```

本地开发时，需要先构建原生绑定和 WASM 产物：

```bash
pnpm install
pnpm build
```

> 当前 npm 徽标中的 `not published` 是真实状态标记，不代表已经可以从
> npm registry 安装。

## Node.js API

```ts
import { extractMeta, extractMetaBatch } from 'metaprobe'

const meta = await extractMeta('./IMG_6920.MOV', { hash: false })

console.log(meta.kind)                  // "video"
console.log(meta.width, meta.height)
console.log(meta.duration, meta.frameRate)
console.log(meta.exif.GPSLatitude, meta.exif.GPSLongitude)
console.log(meta.creationTime)          // 相机记录的拍摄时间
console.log(meta.containerCreationTime) // 容器创建时间

const results = await extractMetaBatch(
  ['./photo.jpg', './video.mov'],
  { concurrency: 4, hash: false },
)
```

原生 API 默认计算文件哈希。如果不需要内容哈希，可以设置 `hash: false`
以获得更低延迟。

## 浏览器 / WASM API

```ts
import init, { extractMetaFastSized } from 'metaprobe'

await init()

const bytes = new Uint8Array(await file.arrayBuffer())
const meta = extractMetaFastSized(bytes, file.name, file.size)
```

如果希望由 WASM 计算 SHA-256，使用 `extractMeta`。如果希望通过 Web Crypto
API 单独计算哈希，使用 `extractMetaFast`。处理视频切片时，推荐使用
`extractMetaFastSized`，它可以保留原始文件大小并正确计算码率。

## Swift / iOS API

在 Xcode 中通过 **File > Add Package Dependencies…** 添加：

```text
https://github.com/IceyWu/metaprobe.git
```

选择 `MetaprobeSwift` 产品，然后把 `PhotosPicker`、`PHPickerViewController`
或其他文件读取器得到的二进制数据传给解析器：

```swift
import MetaprobeSwift

let metadata = try Metaprobe.parse(data: data, filename: "IMG_0001.HEIC")
print(metadata.kind, metadata.format, metadata.width, metadata.height)
print(metadata.exif["Make"] ?? "")
```

Swift Package 支持 iOS 13 及以上。可运行的 SwiftUI 示例位于
[`Examples/MetaprobeDemo`](./Examples/MetaprobeDemo)，当前示例为适配最新
Xcode 模拟器而使用 iOS 27，并通过 `PhotosPicker` 选择照片或视频。

## 返回数据

图片常用字段：

```ts
meta.exif.Make
meta.exif.Model
meta.exif.DateTimeOriginal
meta.exif.GPSLatitude
meta.exif.GPSLongitude
meta.exif.GPSAltitude
meta.icc.ProfileDescription
meta.colorSpace
```

视频额外提供：

```ts
meta.duration
meta.codec
meta.frameRate
meta.overallBitrate
meta.videoBitrate
meta.creationTime
meta.containerCreationTime
meta.metadata // QuickTime 元数据，例如位置和设备信息
```

## 项目架构

```text
Node.js 调用方 -> native/index.js -> crates/napi -> crates/core
浏览器调用方  -> wasm/index.js   -> crates/wasm -> crates/core
Swift/iOS 调用方 -> MetaprobeSwift -> Metaprobe.xcframework -> crates/ios-ffi -> crates/core
                                                     \-> 共用 Rust 解析逻辑
Playground     -> React/Vite 对比页面
                 -> metaprobe WASM 对比 exifr / mediainfo.js
```

核心层不依赖 Node.js 或浏览器 API。适配层只负责 I/O、JavaScript 数据转换
和平台相关的调度，从而保证原生端和浏览器端的解析结果保持一致。

## 性能

性能会受到文件大小、元数据复杂度、运行时、硬件以及是否计算哈希等因素影响。
仓库不包含固定的性能测试媒体，因此这里不固化具体速度数字。发布或对比性能
前，请使用自己的代表性图片和视频进行测试：

```bash
pnpm benchmark -- ./photo.jpg ./video.mov
```

## Playground

Playground 使用 `exifr` 对比图片，使用 `mediainfo.js` 对比视频：

```bash
pnpm build:wasm
pnpm --dir playground dev
```

这些参考库只用于开发和性能对比，不会成为发布版 `metaprobe` 的运行时依赖。

## 开发与检查

```bash
pnpm build:napi
pnpm build:wasm
pnpm --dir playground lint
pnpm --dir playground build

cargo test -p metaprobe-core
cargo clippy --workspace -- -D warnings
```

CI 会为 Windows、Linux 和 macOS 构建原生绑定，验证 WASM 和 playground，
并编译所有支持的 iOS Rust target。iOS XCFramework 单独构建并提交给 Swift
Package 使用；Release 工作流只处理 npm 产物。

## 发布

项目使用 Changesets 管理版本和 npm 发布。当前包版本为 `0.0.1`，
后续面向用户的变更需要先创建 changeset：

```bash
pnpm changeset
pnpm version-packages
pnpm release
```

GitHub 发布工作流会先构建支持平台的原生绑定，汇总 WASM 产物，然后通过
Changesets 创建版本 PR 或执行发布。首次发布前，
需要在 GitHub 仓库配置 `NPM_TOKEN` secret。Swift Package 直接从 Git 仓库
使用；如果要固定稳定版本，发布并使用带版本号的 Git tag 即可。

## 许可证

MIT
