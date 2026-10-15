import { access } from 'node:fs/promises'
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

const [packed] = JSON.parse(execSync('npm pack --dry-run --json --ignore-scripts', { encoding: 'utf8' }))
const packedPaths = new Set(packed.files.map(file => file.path))
for (const path of required) {
  if (!packedPaths.has(path)) {
    throw new Error(`Publish artifact excluded from npm package: ${path}. Check npm ignore rules.`)
  }
}

console.log('Package artifacts verified.')
