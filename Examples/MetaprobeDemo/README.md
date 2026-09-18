# Metaprobe iOS Demo

这是一个最小 SwiftUI 示例：选择照片或视频后，读取文件并调用 `MetaprobeSwift`，解析结果会显示在界面中，同时打印到 Xcode 控制台。

## 使用方式

1. 安装 [XcodeGen](https://github.com/yonaskolb/XcodeGen)，然后在本目录执行 `xcodegen generate`；也可以在 Xcode 中创建一个 iOS App（SwiftUI，iOS 14+），再将本目录下的两个 Swift 文件加入项目。
3. 通过 `File > Add Package Dependencies...` 添加：

   `https://github.com/IceyWu/metaprobe.git`

4. 将产品 `MetaprobeSwift` 加入 App target。
5. 在模拟器或真机运行，点击“选择照片或视频”。

如果还没发布远程包，也可以在 Xcode 中选择 `Add Local...`，指向仓库根目录。
