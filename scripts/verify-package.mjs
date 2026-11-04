import { access, readFile } from 'node:fs/promises'
import { execSync } from 'node:child_process'

const required = [
  'native/index.js',
  'native/index.d.ts',
  'wasm/index.js',
  'wasm/index.d.ts',
  'wasm/package.json',
  'wasm/index_bg.wasm',
  'miniprogram/index.js',
  'miniprogram/index.d.ts',
  'miniprogram/types.d.ts',
  'miniprogram/glue.d.ts',
  'miniprogram/package.json',
  'miniprogram/utf8.js',
  'miniprogram/glue.js',
  'miniprogram/index_bg.wasm',
  'react-native/index.ts',
  'expo-module.config.json',
  'metaprobe.podspec',
  'android/build.gradle',
  'android/src/main/java/expo/modules/metaprobe/MetaprobeModule.kt',
  'android/src/main/jniLibs/arm64-v8a/libmetaprobe_android.so',
  'ios/MetaprobeModule.swift',
  'Sources/MetaprobeSwift/Metaprobe.swift',
  'Metaprobe.xcframework/Info.plist',
  'Metaprobe.xcframework/ios-arm64/libmetaprobe_ios_ffi.a',
  'Metaprobe.xcframework/ios-arm64_x86_64-simulator/libmetaprobe_ios_ffi_sim.a',
]

if (process.env.METAPROBE_VERIFY_ALL_TARGETS === '1') {
  required.push(
    'native/metaprobe.linux-x64-gnu.node',
    'native/metaprobe.linux-arm64-gnu.node',
    'native/metaprobe.win32-x64-msvc.node',
    'native/metaprobe.darwin-x64.node',
    'native/metaprobe.darwin-arm64.node',
  )
}

await Promise.all(required.map(async path => {
  try {
    await access(new URL(`../${path}`, import.meta.url))
  } catch {
    throw new Error(`Missing publish artifact: ${path}. Run pnpm build first.`)
  }
}))

const { version } = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'))
const [podspec, gradle] = await Promise.all([
  readFile(new URL('../metaprobe.podspec', import.meta.url), 'utf8'),
  readFile(new URL('../android/build.gradle', import.meta.url), 'utf8'),
])
if (!podspec.includes(`s.version        = '${version}'`)) {
  throw new Error(`CocoaPods version does not match package version ${version}. Run pnpm version-packages.`)
}
if (!podspec.includes(':tag => "metaprobe@#{s.version}"')) {
  throw new Error('CocoaPods source tag must match the metaprobe@<version> GitHub release tag.')
}
if (!gradle.includes(`version = '${version}'`) || !gradle.includes(`versionName '${version}'`)) {
  throw new Error(`Android module version does not match package version ${version}. Run pnpm version-packages.`)
}

const [packed] = JSON.parse(execSync('npm pack --dry-run --json --ignore-scripts', { encoding: 'utf8' }))
const packedPaths = new Set(packed.files.map(file => file.path))
for (const path of required) {
  if (!packedPaths.has(path)) {
    throw new Error(`Publish artifact excluded from npm package: ${path}. Check npm ignore rules.`)
  }
}

console.log('Package artifacts verified.')
