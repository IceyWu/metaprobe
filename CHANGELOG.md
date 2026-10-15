# Changelog

## 0.1.0

### Minor Changes

- d9f931b: Add a TypeScript-based `metaprobe/miniprogram` entry that loads packaged WASM through WeChat's `WXWebAssembly`, without browser or Node globals. Include matching WASM assets, declarations, UTF-8 codecs, initialization retry/deduplication, and image/video metadata regression tests.

## 0.0.1

首个公开版本：

- 基于 Rust Core 的 JPEG、PNG、WebP、HEIC、AVIF、MOV、MP4 和 M4V 元数据解析。
- 提供 Node.js N-API 原生接口和浏览器 WASM 接口。
- 支持 EXIF、GPS、ICC、色彩空间、QuickTime 元数据、视频时长、帧率和码率。
- 提供批量解析、可选 SHA-256 和 playground 对比工具。
