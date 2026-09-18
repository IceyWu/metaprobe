# Metaprobe iOS Demo

这是一个最小 SwiftUI 示例：选择照片或视频后，读取文件并调用 `MetaprobeSwift`，解析结果会显示在界面中，同时打印到 Xcode 控制台。

## 使用方式

1. 安装 [XcodeGen](https://github.com/yonaskolb/XcodeGen)，然后在本目录执行 `xcodegen generate`；也可以直接打开已生成的 `MetaprobeDemo.xcodeproj`。
2. 通过 `File > Add Package Dependencies...` 添加（如果使用项目内 package，Xcode 会自动解析本地路径）：

   `https://github.com/IceyWu/metaprobe.git`

3. 将产品 `MetaprobeSwift` 加入 App target。
4. 在 iOS 27 模拟器或真机运行，点击“选择照片或视频”。

示例使用 `PhotosPicker`，会从系统照片图库选择照片或视频，并在界面和
Xcode 控制台输出解析后的 JSON 元数据。

如果还没发布远程包，也可以在 Xcode 中选择 `Add Local...`，指向仓库根目录。
