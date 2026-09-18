import SwiftUI
import PhotosUI
import MetaprobeSwift

struct ContentView: View {
    @State private var selectedItem: PhotosPickerItem?
    @State private var output = "请选择一张照片或一个视频"
    @State private var isImporting = false

    var body: some View {
        NavigationView {
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

                PhotosPicker(
                    selection: $selectedItem,
                    matching: .any(of: [.images, .videos]),
                    photoLibrary: .shared()
                ) {
                    Label("选择照片或视频", systemImage: "photo.on.rectangle")
                }
                .buttonStyle(.borderedProminent)
                .disabled(isImporting)
            }
            .padding()
            .navigationTitle("Metaprobe Demo")
            .onChange(of: selectedItem) { _, item in
                guard let item else { return }
                Task { await handleImport(item) }
            }
        }
    }

    private func handleImport(_ item: PhotosPickerItem) async {
        isImporting = true
        defer { isImporting = false }

        do {
            guard let data = try await item.loadTransferable(type: Data.self) else {
                throw MetaprobeError.invalidResult
            }
            let filename = item.itemIdentifier ?? "photo-or-video"
            let metadata = try Metaprobe.parse(data: data, filename: filename)
            let jsonData = try JSONEncoder.pretty.encode(metadata)
            let json = String(decoding: jsonData, as: UTF8.self)
            output = "资源：\(filename)\n大小：\(data.count) bytes\n\n\(json)"
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
