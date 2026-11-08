---
"metaprobe": patch
---

Fix Expo iOS module registration by specifying the podspec path and Swift module name. Use the same static library filename for device and simulator XCFramework slices so CocoaPods can install and link the package.
