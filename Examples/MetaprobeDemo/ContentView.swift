import SwiftUI
import UniformTypeIdentifiers
import MetaprobeSwift

struct ContentView: View {
    @State private var showingPicker = false
    @State private var output = "请选择一张照片或一个视频"
    @State private var isImporting = false

    var body: some View {
        NavigationStack {
            VStack(spacing: 16) {
                ScrollView {
                    Text(output)
                        .font(.system(.body, design: .monospaced))
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .textSelection(.enabled)
                        .padding()
                }
                .background(Color(.secondarySystemBackground))
                .clipShape(RoundedRectangle(cornerRadius: 12))

                Button("选择照片或视频") {
                    showingPicker = true
                }
                .buttonStyle(.borderedProminent)
                .disabled(isImporting)
            }
            .padding()
            .navigationTitle("Metaprobe Demo")
            .fileImporter(
                isPresented: $showingPicker,
                allowedContentTypes: [.image, .movie, .video],
                allowsMultipleSelection: false,
                onCompletion: handleImport
            )
        }
    }

    private func handleImport(_ result: Result<[URL], Error>) {
        guard case let .success(urls) = result, let url = urls.first else { return }
        isImporting = true
        defer { isImporting = false }

        do {
            let accessed = url.startAccessingSecurityScopedResource()
            defer { if accessed { url.stopAccessingSecurityScopedResource() } }
            let data = try Data(contentsOf: url)
            let metadata = try Metaprobe.parse(data: data, filename: url.lastPathComponent)
            let json = try JSONEncoder.pretty.encode(metadata)
            output = "文件：\(url.lastPathComponent)\n大小：\(data.count) bytes\n\n\(json)"
            print(output)
        } catch {
            output = "解析失败：\(error)"
            print(output)
        }
    }
}

private extension JSONEncoder {
    static var pretty: JSONEncoder {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        return encoder
    }
}
