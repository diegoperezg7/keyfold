import AppKit
import LlaveroCore
import SwiftUI

struct SecretPromptView: View {
    @Bindable var model: SecretPromptViewModel
    let onFinish: (Bool) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(L10n.t("Valor de \(model.name)", "Value for \(model.name)"))
                .font(.headline)
            if let project = model.project {
                Text(L10n.t("Proyecto: \(project)", "Project: \(project)"))
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            SecureField(L10n.t("Pega aquí la clave", "Paste the key here"), text: $model.value)
                .textFieldStyle(.roundedBorder)
                .onSubmit(submit)
            Text(L10n.t(
                "Se guarda cifrado en el Llavero de macOS. El agente solo recibe la referencia llavero://\(model.name).",
                "Stored encrypted in the macOS Keychain. The agent only gets the reference llavero://\(model.name)."
            ))
            .font(.caption)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
            if let error = model.errorMessage {
                Text(error).font(.caption).foregroundStyle(.red)
            }
            HStack {
                Spacer()
                Button(L10n.t("Cancelar", "Cancel")) {
                    model.cancel()
                    onFinish(false)
                }
                    .keyboardShortcut(.cancelAction)
                Button(L10n.t("Guardar", "Save"), action: submit)
                    .keyboardShortcut(.defaultAction)
                    .disabled(!model.canSave)
            }
        }
        .padding(20)
        .frame(width: 420)
    }

    private func submit() {
        guard model.canSave else { return }
        if model.save() { onFinish(true) }
    }
}

private final class PromptWindowDelegate: NSObject, NSWindowDelegate {
    func windowWillClose(_ notification: Notification) {
        NSApplication.shared.stopModal()
    }
}

/// Shows a native window from the CLI process and blocks until saved or cancelled.
@MainActor
func presentSecretPrompt(name: String, project: String?, save: @escaping (String) throws -> Void) -> Bool {
    let app = NSApplication.shared
    // From the CLI/MCP the app is not running yet; inside `llavero app` keep its policy and menu.
    if !app.isRunning {
        app.setActivationPolicy(.accessory)
        app.mainMenu = editMenu()
    }

    var saved = false
    let model = SecretPromptViewModel(name: name, project: project, onSave: save)
    let window = NSWindow(
        contentRect: NSRect(x: 0, y: 0, width: 420, height: 200),
        styleMask: [.titled, .closable],
        backing: .buffered,
        defer: false
    )
    let delegate = PromptWindowDelegate()
    window.title = "Llavero"
    window.delegate = delegate
    window.level = .floating
    window.isReleasedWhenClosed = false
    window.contentView = NSHostingView(rootView: SecretPromptView(model: model) { ok in
        saved = ok
        app.stopModal()
    })
    window.center()
    app.activate(ignoringOtherApps: true)
    window.makeKeyAndOrderFront(nil)
    app.runModal(for: window)
    window.delegate = nil
    window.orderOut(nil)
    return saved
}

/// Without an Edit menu, Cmd+V does not reach the text field in an accessory app.
func editMenu() -> NSMenu {
    let main = NSMenu()
    let editItem = NSMenuItem()
    let edit = NSMenu(title: "Edit")
    edit.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
    edit.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
    editItem.submenu = edit
    main.addItem(editItem)
    return main
}
