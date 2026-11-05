import Foundation
import LlaveroCore
import XCTest

final class RedactorTests: XCTestCase {
    func testMasksValueSplitAcrossChunks() {
        var redactor = Redactor(secrets: [("EXA_API_KEY", "abc123secret")])
        var output = redactor.feed(Data("key=abc12".utf8))
        output += redactor.feed(Data("3secret\nnext ".utf8))
        output += redactor.finish()
        XCTAssertEqual(String(decoding: output, as: UTF8.self), "key=[llavero:EXA_API_KEY]\nnext ")
    }

    func testMasksValueInLongOutputWithoutNewlines() {
        let value = "zzz-secret-value"
        var redactor = Redactor(secrets: [("K", value)])
        let filler = String(repeating: "a", count: 70 * 1024)
        var output = redactor.feed(Data((filler + value.prefix(5)).utf8))
        output += redactor.feed(Data((value.dropFirst(5) + "end").utf8))
        output += redactor.finish()
        let text = String(decoding: output, as: UTF8.self)
        XCTAssertFalse(text.contains(value))
        XCTAssertTrue(text.hasSuffix("[llavero:K]end"))
    }
}
