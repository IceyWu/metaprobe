import Foundation
import MetaprobeFFI

public struct MediaMeta: Decodable, Sendable {
    public let kind: String
    public let format: String
    public let width: UInt32
    public let height: UInt32
    public let colorSpace: String
    public let exif: [String: String]
    public let icc: [String: String]
    public let duration: Double?
    public let creationTime: String?
    public let metadata: [String: String]
    public let fileHash: String?
}

public enum MetaprobeError: Error {
    case invalidResult
    case parse(String)
}

public enum Metaprobe {
    public static func parse(data: Data, filename: String) throws -> MediaMeta {
        let result: String? = data.withUnsafeBytes { bytes in
            filename.withCString { name in
                guard let base = bytes.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                    return nil
                }
                guard let pointer = metaprobe_extract_json(base, data.count, name) else {
                    return nil
                }
                defer { metaprobe_free_string(pointer) }
                return String(cString: pointer)
            }
        }
        guard let result, let json = result.data(using: .utf8) else {
            throw MetaprobeError.invalidResult
        }
        if let error = try? JSONDecoder().decode([String: String].self, from: json),
           let message = error["error"] {
            throw MetaprobeError.parse(message)
        }
        return try JSONDecoder().decode(MediaMeta.self, from: json)
    }
}
