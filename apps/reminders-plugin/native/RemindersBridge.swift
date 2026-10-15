import EventKit
import Foundation

// Protocolo: una petición JSON por stdin y una respuesta JSON por stdout.
// `watch` deja el proceso vivo y emite avisos sin el contenido privado.
struct Request: Decodable {
    let action: String
    let id: String?
    let listId: String?
    let title: String?
    let notes: String?
    let dueAt: String?
    let completed: Bool?
    let clearNotes: Bool?
    let clearDueAt: Bool?
}

struct ReminderView: Encodable {
    let id: String
    let listId: String
    let title: String
    let notes: String?
    let dueAt: String?
    let completed: Bool
}

struct ListView: Encodable {
    let id: String
    let title: String
}

struct ChangeView: Encodable {
    let kind: String
    let id: String
}

struct Output<T: Encodable>: Encodable {
    let result: T?
    let error: String?
}

enum BridgeError: LocalizedError {
    case invalidInput(String)
    case missing(String)
    case denied

    var errorDescription: String? {
        switch self {
        case .invalidInput(let detail): return detail
        case .missing(let resource): return "No se encontró \(resource)"
        case .denied: return "Acceso a Recordatorios denegado en macOS"
        }
    }
}

let store = EKEventStore()
let encoder = JSONEncoder()
let decoder = JSONDecoder()
let dateFormatter = ISO8601DateFormatter()
dateFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
let fallbackDateFormatter = ISO8601DateFormatter()

func emit<T: Encodable>(_ result: T) {
    let output = Output(result: result, error: Optional<String>.none)
    if let data = try? encoder.encode(output), let line = String(data: data, encoding: .utf8) {
        print(line)
        fflush(stdout)
    }
}

func emitError(_ message: String) {
    let output = Output<String>(result: nil, error: message)
    if let data = try? encoder.encode(output), let line = String(data: data, encoding: .utf8) {
        print(line)
        fflush(stdout)
    }
}

func access() throws {
    let semaphore = DispatchSemaphore(value: 0)
    var granted = false
    var failure: Error?
    store.requestFullAccessToReminders { allowed, error in
        granted = allowed
        failure = error
        semaphore.signal()
    }
    guard semaphore.wait(timeout: .now() + 30) == .success else {
        throw BridgeError.invalidInput("Tiempo de espera agotado al pedir acceso a Recordatorios")
    }
    if let failure { throw failure }
    if !granted { throw BridgeError.denied }
}

func allReminders() throws -> [EKReminder] {
    let predicate = store.predicateForReminders(in: nil)
    let semaphore = DispatchSemaphore(value: 0)
    var reminders: [EKReminder]?
    store.fetchReminders(matching: predicate) { found in
        reminders = found
        semaphore.signal()
    }
    guard semaphore.wait(timeout: .now() + 30) == .success else {
        throw BridgeError.invalidInput("Tiempo de espera agotado al consultar Recordatorios")
    }
    guard let reminders else {
        throw BridgeError.invalidInput("EventKit no pudo consultar Recordatorios")
    }
    return reminders
}

func view(_ reminder: EKReminder) -> ReminderView {
    ReminderView(
        id: reminder.calendarItemIdentifier,
        listId: reminder.calendar.calendarIdentifier,
        title: reminder.title ?? "",
        notes: reminder.notes?.isEmpty == true ? nil : reminder.notes,
        dueAt: reminder.dueDateComponents?.date.map { dateFormatter.string(from: $0) },
        completed: reminder.isCompleted
    )
}

func reminder(_ id: String) throws -> EKReminder {
    guard let item = store.calendarItem(withIdentifier: id) as? EKReminder else {
        throw BridgeError.missing("el recordatorio")
    }
    return item
}

func process(_ request: Request) throws {
    switch request.action {
    case "lists":
        emit(store.calendars(for: .reminder).map { ListView(id: $0.calendarIdentifier, title: $0.title) })
    case "list":
        let items = try allReminders().filter { request.listId == nil || $0.calendar.calendarIdentifier == request.listId }
        emit(items.map(view))
    case "get":
        guard let id = request.id else { throw BridgeError.invalidInput("Falta id") }
        emit(view(try reminder(id)))
    case "create":
        guard let title = request.title, !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw BridgeError.invalidInput("Falta title")
        }
        let item = EKReminder(eventStore: store)
        if let listId = request.listId {
            guard let list = store.calendar(withIdentifier: listId), list.allowedEntityTypes.contains(.reminder) else {
                throw BridgeError.missing("la lista")
            }
            item.calendar = list
        } else {
            guard let list = store.defaultCalendarForNewReminders() else { throw BridgeError.missing("la lista predeterminada") }
            item.calendar = list
        }
        item.title = title
        item.notes = request.notes
        if let dueAt = request.dueAt {
            guard let date = dateFormatter.date(from: dueAt) ?? fallbackDateFormatter.date(from: dueAt) else {
                throw BridgeError.invalidInput("dueAt debe ser RFC 3339")
            }
            item.dueDateComponents = Calendar.current.dateComponents(in: TimeZone(secondsFromGMT: 0)!, from: date)
        }
        try store.save(item, commit: true)
        emit(view(item))
    case "update":
        guard let id = request.id else { throw BridgeError.invalidInput("Falta id") }
        let item = try reminder(id)
        if let title = request.title { item.title = title }
        if request.clearNotes == true { item.notes = nil }
        else if let notes = request.notes { item.notes = notes }
        if let completed = request.completed { item.isCompleted = completed }
        if request.clearDueAt == true { item.dueDateComponents = nil }
        else if let dueAt = request.dueAt {
            guard let date = dateFormatter.date(from: dueAt) ?? fallbackDateFormatter.date(from: dueAt) else {
                throw BridgeError.invalidInput("dueAt debe ser RFC 3339")
            }
            item.dueDateComponents = Calendar.current.dateComponents(in: TimeZone(secondsFromGMT: 0)!, from: date)
        }
        try store.save(item, commit: true)
        emit(view(item))
    case "delete":
        guard let id = request.id else { throw BridgeError.invalidInput("Falta id") }
        try store.remove(try reminder(id), commit: true)
        emit("deleted")
    case "watch":
        var previous = Dictionary(uniqueKeysWithValues: try allReminders().map { ($0.calendarItemIdentifier, fingerprint($0)) })
        let queue = DispatchQueue(label: "soc.reminders.changes")
        emit(ChangeView(kind: "ready", id: ""))
        let observer = NotificationCenter.default.addObserver(forName: .EKEventStoreChanged, object: store, queue: nil) { _ in
            queue.async {
                do {
                    let current = Dictionary(uniqueKeysWithValues: try allReminders().map { ($0.calendarItemIdentifier, fingerprint($0)) })
                    for (id, value) in current where previous[id] != value {
                        emit(ChangeView(kind: previous[id] == nil ? "created" : "updated", id: id))
                    }
                    for id in previous.keys where current[id] == nil {
                        emit(ChangeView(kind: "deleted", id: id))
                    }
                    previous = current
                } catch {
                    emitError(error.localizedDescription)
                }
            }
        }
        withExtendedLifetime(observer) { RunLoop.current.run() }
    default:
        throw BridgeError.invalidInput("Acción desconocida")
    }
}

func fingerprint(_ reminder: EKReminder) -> String {
    let item = view(reminder)
    return [item.listId, item.title, item.notes ?? "", item.dueAt ?? "", String(item.completed)].joined(separator: "\u{1f}")
}

do {
    guard let line = readLine(), let data = line.data(using: .utf8) else {
        throw BridgeError.invalidInput("Falta petición JSON")
    }
    let request = try decoder.decode(Request.self, from: data)
    try access()
    try process(request)
} catch {
    emitError(error.localizedDescription)
    exit(1)
}
