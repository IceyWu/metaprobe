import { access } from 'node:fs/promises'

const required = [
  'native/index.js',
  'native/index.d.ts',
  'wasm/index.js',
  'wasm/types.d.ts',
  'wasm/index_bg.wasm',
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

console.log('Package artifacts verified.')
