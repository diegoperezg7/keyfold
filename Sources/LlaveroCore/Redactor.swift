import Foundation

/// Replaces secret values in a byte stream with `[llavero:NAME]`.
/// Works line by line so a value split across two reads is still caught.
public struct Redactor {
    private let secrets: [(value: Data, mask: Data)]
    private let maxValueLength: Int
    private var buffer = Data()
    private static let maxBuffer = 64 * 1024

    public init(secrets: [(name: String, value: String)]) {
        self.secrets = secrets
            .filter { !$0.value.isEmpty }
            .sorted { $0.value.utf8.count > $1.value.utf8.count }
            .map { (Data($0.value.utf8), Data("[llavero:\($0.name)]".utf8)) }
        maxValueLength = self.secrets.first?.value.count ?? 0
    }

    /// Returns what can be safely emitted now.
    public mutating func feed(_ data: Data) -> Data {
        buffer.append(data)
        if let newline = buffer.lastIndex(of: UInt8(ascii: "\n")) {
            let ready = buffer[buffer.startIndex...newline]
            buffer = Data(buffer[buffer.index(after: newline)...])
            return redact(Data(ready))
        }
        // Long output without newlines: emit all but a tail that could hold a partial value.
        guard buffer.count > Self.maxBuffer else { return Data() }
        var redacted = redact(buffer)
        let keep = max(maxValueLength - 1, 0)
        buffer = Data(redacted.suffix(keep))
        redacted.removeLast(keep)
        return redacted
    }

    /// Flushes what is left at end of stream.
    public mutating func finish() -> Data {
        defer { buffer = Data() }
        return redact(buffer)
    }

    public func redact(_ data: Data) -> Data {
        var output = data
        for secret in secrets {
            var searchStart = output.startIndex
            while let range = output.range(of: secret.value, in: searchStart..<output.endIndex) {
                output.replaceSubrange(range, with: secret.mask)
                searchStart = range.lowerBound + secret.mask.count
            }
        }
        return output
    }
}
