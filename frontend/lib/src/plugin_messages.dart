part of 'plugins.dart';

/// La vista nunca toma summary/events.message técnicos como comunicación del asistente.
class _JobPresentation {
  const _JobPresentation(this.status, this.messages);
  final String status;
  final List<Map<String, dynamic>> messages;

  factory _JobPresentation.read(Map job) {
    final data = job['bot_presentation'];
    if (data is! Map) return const _JobPresentation('unattributed', []);
    if (data['status'] != 'ready') {
      return _JobPresentation(
        data['status'] is String ? data['status'] as String : 'unavailable',
        const [],
      );
    }
    final rows = data['messages'];
    if (rows is! List || rows.length > 12) {
      return const _JobPresentation('unavailable', []);
    }
    final messages = <Map<String, dynamic>>[];
    final ids = <int>{};
    var previous = 0, characters = 0;
    for (final row in rows) {
      if (row is! Map ||
          row['message_id'] is! int ||
          row['message_id'] <= 0 ||
          row['sequence'] is! int ||
          row['sequence'] <= previous ||
          row['text'] is! String ||
          (row['text'] as String).trim().isEmpty ||
          (row['text'] as String).runes.length > 1600 ||
          (row['text'] as String).contains('\u0000') ||
          row['created_at'] is! String ||
          DateTime.tryParse(row['created_at'] as String) == null ||
          !ids.add(row['message_id'] as int)) {
        return const _JobPresentation('unavailable', []);
      }
      previous = row['sequence'] as int;
      characters += (row['text'] as String).runes.length;
      if (characters > 8000) return const _JobPresentation('unavailable', []);
      messages.add(Map<String, dynamic>.from(row));
    }
    return _JobPresentation('ready', messages);
  }

  String summary(Map job) {
    if (messages.isNotEmpty) return messages.last['text'] as String;
    final state = switch (job['state']) {
      'queued' => 'Construcción en cola.',
      'running' => 'Preparación en curso.',
      'waiting_user' => 'Construcción a la espera de intervención.',
      'completed' => 'Construcción finalizada.',
      'blocked' => 'Construcción detenida.',
      'cancelled' => 'Construcción cancelada.',
      'interrupted' => 'Construcción interrumpida.',
      _ => 'Estado de construcción no disponible.',
    };
    final detail = switch (status) {
      'ready' => 'Aún no hay mensajes del asistente para esta construcción.',
      'unattributed' =>
        'Esta construcción no tiene mensajes asociados en el chat.',
      'origin_unavailable' => 'El chat de origen ya no está disponible.',
      _ =>
        'No se pudieron cargar las actualizaciones del asistente. Recarga la ficha para reintentar.',
    };
    return '$state\n\n$detail';
  }

  String? updatedAt(Map job) => messages.isEmpty
      ? _jobUpdatedAt(job)
      : _jobUpdatedAt({'updated_at': messages.last['created_at']});
}
