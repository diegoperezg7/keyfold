import AppKit
import LlaveroCore
import SwiftUI

struct KeysView: View {
    @Bindable var model: KeysViewModel
    /// Opens the existing paste window for a key.
    let requestValue: (String) -> Void

    @State private var assigningKey: String?
    @State private var projectName = ""

    var body: some View {
        VStack(spacing: 0) {
            topBar
            Divider()
            content
            Divider()
            footer
        }
        .frame(minWidth: 780, minHeight: 340)
        .sheet(isPresented: Binding(get: { assigningKey != nil }, set: { if !$0 { assigningKey = nil } })) {
            assignSheet
        }
        // An agent may create slots over MCP while the window is open.
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
            model.reload()
        }
    }

    // MARK: - Top bar

    private var topBar: some View {
        HStack(spacing: 12) {
            HStack(spacing: 6) {
                Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                TextField(L10n.t("Buscar por nombre o proyecto", "Search by name or project"), text: $model.searchText)
                    .textFieldStyle(.plain)
                if !model.searchText.isEmpty {
                    Button { model.searchText = "" } label: {
                        Image(systemName: "xmark.circle.fill")
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(.secondary)
                    .help(L10n.t("Borrar búsqueda", "Clear search"))
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .background(RoundedRectangle(cornerRadius: 6).fill(Color(nsColor: .textBackgroundColor)))
            .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Color(nsColor: .separatorColor)))
            .frame(maxWidth: 300)

            Picker(L10n.t("Estado", "Status"), selection: $model.statusFilter) {
                ForEach(KeyStatusFilter.allCases) { filter in
                    Text("\(title(for: filter)) \(model.count(for: filter))").tag(filter)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .fixedSize()

            Spacer()

            Button(action: model.reload) {
                Image(systemName: "arrow.clockwise")
            }
            .help(L10n.t("Actualizar", "Refresh"))
        }
        .padding(12)
    }

    private func title(for filter: KeyStatusFilter) -> String {
        switch filter {
        case .all: L10n.t("Todas", "All")
        case .stored: L10n.t("Guardadas", "Stored")
        case .empty: L10n.t("Vacías", "Empty")
        }
    }

    // MARK: - Content

    @ViewBuilder
    private var content: some View {
        if model.keys.isEmpty {
            ContentUnavailableView(
                L10n.t("Sin claves", "No keys"),
                systemImage: "key",
                description: Text(L10n.t(
                    "Pide a tu agente que cree una, o usa: llavero create NOMBRE",
                    "Ask your agent to create one, or run: llavero create NAME"
                ))
            )
            .frame(maxHeight: .infinity)
        } else if model.visibleKeys.isEmpty {
            ContentUnavailableView(
                L10n.t("Sin resultados", "No results"),
                systemImage: "magnifyingglass",
                description: Text(L10n.t("Ninguna clave coincide con el filtro.", "No key matches the filter."))
            )
            .frame(maxHeight: .infinity)
        } else {
            table
        }
    }

    private var table: some View {
        Table(model.visibleKeys, sortOrder: $model.sortOrder) {
            TableColumn(L10n.t("Nombre", "Name"), value: \.name) { key in
                Label {
                    Text(key.name).font(.body.monospaced())
                } icon: {
                    Image(systemName: "key.fill").foregroundStyle(.secondary)
                }
            }
            .width(min: 160, ideal: 220)

            TableColumn(L10n.t("Estado", "Status"), value: \.statusRank) { key in
                StatusBadge(hasValue: key.hasValue)
            }
            .width(min: 100, ideal: 110)

            TableColumn(L10n.t("Proyectos", "Projects")) { key in
                ProjectChips(projects: key.projects)
            }
            .width(min: 140, ideal: 200)

            TableColumn("") { key in
                actions(for: key)
            }
            .width(min: 270, ideal: 280)
        }
    }

    private func actions(for key: SlotInfo) -> some View {
        HStack(spacing: 8) {
            if key.hasValue {
                Button { requestValue(key.name) } label: {
                    Label(L10n.t("Cambiar valor", "Change value"), systemImage: "arrow.triangle.2.circlepath")
                }
            } else {
                // Empty slots are the pending action: make it stand out.
                Button { requestValue(key.name) } label: {
                    Label(L10n.t("Añadir valor", "Add value"), systemImage: "plus")
                }
                .buttonStyle(.borderedProminent)
            }
            Button {
                projectName = ""
                model.errorMessage = nil
                assigningKey = key.name
            } label: {
                Label(L10n.t("Asignar proyecto", "Assign project"), systemImage: "folder.badge.plus")
            }
        }
        .controlSize(.small)
    }

    // MARK: - Footer

    private var footer: some View {
        HStack(spacing: 6) {
            Image(systemName: "lock.shield").foregroundStyle(.secondary)
            Text(L10n.t(
                "Cifradas en el Llavero de macOS. Los valores nunca se muestran aquí.",
                "Encrypted in the macOS Keychain. Values are never shown here."
            ))
            .foregroundStyle(.secondary)
            Spacer()
            if let error = model.errorMessage, assigningKey == nil {
                Text(error).foregroundStyle(.red)
            } else {
                Text(summary).foregroundStyle(.secondary)
            }
        }
        .font(.caption)
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    private var summary: String {
        let total = model.keys.count
        return L10n.t(
            "\(total) \(total == 1 ? "clave" : "claves") · \(model.storedCount) \(model.storedCount == 1 ? "guardada" : "guardadas") · \(model.emptyCount) \(model.emptyCount == 1 ? "vacía" : "vacías")",
            "\(total) \(total == 1 ? "key" : "keys") · \(model.storedCount) stored · \(model.emptyCount) empty"
        )
    }

    // MARK: - Assign sheet

    private var assignSheet: some View {
        let current = model.keys.first { $0.name == assigningKey }?.projects ?? []
        let suggestions = model.knownProjects.filter { !current.contains($0) }
        return VStack(alignment: .leading, spacing: 12) {
            Text(L10n.t("Asignar \(assigningKey ?? "") a un proyecto", "Assign \(assigningKey ?? "") to a project"))
                .font(.headline)
            if !current.isEmpty {
                HStack(spacing: 6) {
                    Text(L10n.t("Ya asignada a:", "Already assigned to:")).foregroundStyle(.secondary)
                    ProjectChips(projects: current)
                }
                .font(.caption)
            }
            if !suggestions.isEmpty {
                Text(L10n.t("Proyectos existentes", "Existing projects"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(suggestions, id: \.self) { project in
                            Button(project) { projectName = project }
                                .buttonStyle(.bordered)
                                .controlSize(.small)
                        }
                    }
                }
            }
            TextField(L10n.t("Nombre del proyecto", "Project name"), text: $projectName)
                .textFieldStyle(.roundedBorder)
                .onSubmit(assign)
            if let error = model.errorMessage {
                Text(error).font(.caption).foregroundStyle(.red)
            }
            HStack {
                Spacer()
                Button(L10n.t("Cancelar", "Cancel")) { assigningKey = nil }
                    .keyboardShortcut(.cancelAction)
                Button(L10n.t("Asignar", "Assign"), action: assign)
                    .keyboardShortcut(.defaultAction)
                    .disabled(projectName.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(20)
        .frame(width: 380)
    }

    private func assign() {
        guard let name = assigningKey else { return }
        if model.assign(name, to: projectName) { assigningKey = nil }
    }
}

private struct StatusBadge: View {
    let hasValue: Bool

    var body: some View {
        let tint: Color = hasValue ? .green : .orange
        Label(
            hasValue ? L10n.t("guardado", "stored") : L10n.t("vacío", "empty"),
            systemImage: hasValue ? "checkmark.seal.fill" : "exclamationmark.circle"
        )
        .font(.caption.weight(.medium))
        .foregroundStyle(tint)
        .padding(.horizontal, 8)
        .padding(.vertical, 3)
        .background(Capsule().fill(tint.opacity(0.15)))
    }
}

private struct ProjectChips: View {
    let projects: [String]

    var body: some View {
        if projects.isEmpty {
            Text(L10n.t("Sin proyecto", "No project")).foregroundStyle(.tertiary)
        } else {
            HStack(spacing: 4) {
                ForEach(projects, id: \.self) { project in
                    Text(project)
                        .font(.caption)
                        .lineLimit(1)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Capsule().strokeBorder(Color(nsColor: .separatorColor)))
                }
            }
        }
    }
}

private final class KeysAppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}

/// `llavero app`: a regular app window listing keys. Blocks until the app quits.
@MainActor
func runKeysApp() -> Int32 {
    let app = NSApplication.shared
    app.setActivationPolicy(.regular)
    app.mainMenu = appMenu()
    let delegate = KeysAppDelegate()
    app.delegate = delegate

    let model = KeysViewModel(llavero: llavero)
    model.reload()

    let window = NSWindow(
        contentRect: NSRect(x: 0, y: 0, width: 860, height: 440),
        styleMask: [.titled, .closable, .miniaturizable, .resizable],
        backing: .buffered,
        defer: false
    )
    window.title = "Llavero"
    window.isReleasedWhenClosed = false
    window.contentView = NSHostingView(rootView: KeysView(model: model) { name in
        _ = promptAndSave(name, project: nil)
        model.reload()
    })
    window.center()
    window.makeKeyAndOrderFront(nil)
    app.activate(ignoringOtherApps: true)
    app.run()
    return 0
}

private func appMenu() -> NSMenu {
    let main = NSMenu()
    let appItem = NSMenuItem()
    let appSubmenu = NSMenu(title: "Llavero")
    appSubmenu.addItem(
        withTitle: L10n.t("Salir de Llavero", "Quit Llavero"),
        action: #selector(NSApplication.terminate(_:)),
        keyEquivalent: "q"
    )
    appItem.submenu = appSubmenu
    main.addItem(appItem)
    let edit = editMenu()
    for item in edit.items {
        edit.removeItem(item)
        main.addItem(item)
    }
    return main
}
