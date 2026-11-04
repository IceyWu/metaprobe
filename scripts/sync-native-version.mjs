import { readFile, writeFile } from 'node:fs/promises'

const { version } = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'))
const podspecPath = new URL('../metaprobe.podspec', import.meta.url)
const gradlePath = new URL('../android/build.gradle', import.meta.url)

const podspec = await readFile(podspecPath, 'utf8')
if (!/s\.version\s*=\s*'[^']+'/.test(podspec)) {
  throw new Error('Could not find s.version in metaprobe.podspec')
}
await writeFile(podspecPath, podspec.replace(/s\.version\s*=\s*'[^']+'/, `s.version        = '${version}'`))

const gradle = await readFile(gradlePath, 'utf8')
if (!/version\s*=\s*'[^']+'/.test(gradle) || !/versionName\s+'[^']+'/.test(gradle)) {
  throw new Error('Could not find Android module version fields in android/build.gradle')
}
await writeFile(gradlePath, gradle
  .replace(/version\s*=\s*'[^']+'/, `version = '${version}'`)
  .replace(/versionName\s+'[^']+'/, `versionName '${version}'`))

console.log(`Synced CocoaPods and Android module versions to ${version}.`)
