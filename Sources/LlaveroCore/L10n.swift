import Foundation

/// ES/EN text. Spanish unless the user's first preferred language is not Spanish.
public enum L10n {
    public static var isSpanish: Bool {
        if let forced = ProcessInfo.processInfo.environment["LLAVERO_LANG"] {
            return forced.hasPrefix("es")
        }
        return Locale.preferredLanguages.first?.hasPrefix("es") ?? true
    }

    public static func t(_ es: String, _ en: String) -> String {
        isSpanish ? es : en
    }
}
