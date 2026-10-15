// Mini-program logic contexts need not expose TextEncoder/TextDecoder.
export class Utf8Encoder {
  encode(value: string = ''): Uint8Array {
    const text = String(value)
    const output = new Uint8Array(text.length * 3)
    return output.subarray(0, this.encodeInto(text, output).written)
  }

  encodeInto(text: string, output: Uint8Array): { read: number; written: number } {
    let read = 0
    let written = 0
    while (read < text.length) {
      let point = text.charCodeAt(read)
      let units = 1
      if (point >= 0xd800 && point <= 0xdbff) {
        const low = text.charCodeAt(read + 1)
        if (low >= 0xdc00 && low <= 0xdfff) {
          point = 0x10000 + (point - 0xd800) * 0x400 + low - 0xdc00
          units = 2
        } else point = 0xfffd
      } else if (point >= 0xdc00 && point <= 0xdfff) point = 0xfffd
      const length = point < 0x80 ? 1 : point < 0x800 ? 2 : point < 0x10000 ? 3 : 4
      if (written + length > output.length) break
      if (length === 1) output[written++] = point
      else {
        output[written++] = (length === 2 ? 0xc0 : length === 3 ? 0xe0 : 0xf0) + Math.floor(point / 64 ** (length - 1))
        for (let shift = length - 2; shift >= 0; shift--) {
          output[written++] = 0x80 + Math.floor(point / 64 ** shift) % 64
        }
      }
      read += units
    }
    return { read, written }
  }
}

export class Utf8Decoder {
  private readonly ignoreBOM: boolean

  constructor(label = 'utf-8', options: { ignoreBOM?: boolean; fatal?: boolean } = {}) {
    if (label.toLowerCase() !== 'utf-8') throw new RangeError('Only UTF-8 is supported')
    this.ignoreBOM = options.ignoreBOM ?? false
  }

  decode(input: ArrayBuffer | ArrayBufferView = new Uint8Array()): string {
    const bytes = input instanceof ArrayBuffer ? new Uint8Array(input)
      : new Uint8Array(input.buffer, input.byteOffset, input.byteLength)
    let text = ''
    for (let offset = 0; offset < bytes.length;) {
      const first = bytes[offset++]
      if (first < 0x80) {
        text += String.fromCharCode(first)
        continue
      }
      const length = first >= 0xc2 && first <= 0xdf ? 2
        : first >= 0xe0 && first <= 0xef ? 3
        : first >= 0xf0 && first <= 0xf4 ? 4 : 0
      if (!length || offset + length - 1 > bytes.length) throw new TypeError('Invalid UTF-8')
      let point = first % 2 ** (7 - length)
      for (let index = 1; index < length; index++) {
        const next = bytes[offset++]
        if (next < 0x80 || next > 0xbf) throw new TypeError('Invalid UTF-8')
        point = point * 64 + next - 0x80
      }
      if (point < (length === 2 ? 0x80 : length === 3 ? 0x800 : 0x10000)
          || point > 0x10ffff || (point >= 0xd800 && point <= 0xdfff)) throw new TypeError('Invalid UTF-8')
      text += String.fromCodePoint(point)
    }
    return !this.ignoreBOM && text.charCodeAt(0) === 0xfeff ? text.slice(1) : text
  }
}
