import 'dart:async';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:url_launcher/url_launcher.dart';
import 'backend.dart';

DateTime? messageLocalDay(String timestamp) {
  final date = DateTime.tryParse(timestamp)?.toLocal();
  return date == null ? null : DateTime(date.year, date.month, date.day);
}

String messageTimeLabel(String timestamp) {
  final date = DateTime.tryParse(timestamp)?.toLocal();
  if (date == null) return '';
  return '${date.hour.toString().padLeft(2, '0')}:${date.minute.toString().padLeft(2, '0')}';
}

String chatDayLabel(String timestamp, {required DateTime now}) {
  final date = messageLocalDay(timestamp);
  if (date == null) return '';
  final localNow = now.toLocal();
  final days = DateTime.utc(
    localNow.year,
    localNow.month,
    localNow.day,
  ).difference(DateTime.utc(date.year, date.month, date.day)).inDays;
  if (days == 0) return 'Hoy';
  if (days == 1) return 'Ayer';
  if (days == 2) return 'Antier';
  const weekdays = [
    'lunes',
    'martes',
    'miércoles',
    'jueves',
    'viernes',
    'sábado',
    'domingo',
  ];
  const months = [
    'enero',
    'febrero',
    'marzo',
    'abril',
    'mayo',
    'junio',
    'julio',
    'agosto',
    'septiembre',
    'octubre',
    'noviembre',
    'diciembre',
  ];
  final year = date.year == localNow.year ? '' : ' de ${date.year}';
  return '${weekdays[date.weekday - 1]}, ${date.day} de ${months[date.month - 1]}$year';
}

class ChatDayBadge extends StatelessWidget {
  const ChatDayBadge({super.key, required this.label});
  final String label;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(top: 4, bottom: 24),
    child: Center(
      child: ClipRRect(
        borderRadius: BorderRadius.circular(20),
        child: BackdropFilter(
          filter: ui.ImageFilter.blur(sigmaX: 4, sigmaY: 4),
          blendMode: BlendMode.src,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 5),
            decoration: BoxDecoration(
              color: const Color(0x14808080),
              borderRadius: BorderRadius.circular(20),
            ),
            child: Text(
              label,
              style: TextStyle(
                fontSize: 10,
                fontWeight: FontWeight.w500,
                color: Theme.of(
                  context,
                ).colorScheme.onSurface.withValues(alpha: .6),
              ),
            ),
          ),
        ),
      ),
    ),
  );
}

class ChatMessage extends StatelessWidget {
  const ChatMessage({super.key, required this.message});
  final Message message;

  @override
  Widget build(BuildContext context) {
    final user = message.speaker == 'user';
    final system = message.speaker == 'system';
    final time = messageTimeLabel(message.createdAt);
    return LayoutBuilder(
      builder: (context, constraints) => Padding(
        padding: const EdgeInsets.only(bottom: 24),
        child: Align(
          alignment: user ? Alignment.centerRight : Alignment.centerLeft,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(16),
            child: BackdropFilter(
              filter: ui.ImageFilter.blur(sigmaX: 4, sigmaY: 4),
              // Sustituir el fondo filtrado: srcOver conserva el patrón nítido
              // al componer sobre el canvas transparente de macOS.
              blendMode: BlendMode.src,
              child: Container(
                constraints: BoxConstraints(
                  minWidth: constraints.maxWidth * .4,
                  maxWidth: constraints.maxWidth * .7,
                ),
                padding: const EdgeInsets.symmetric(
                  horizontal: 16,
                  vertical: 12,
                ),
                decoration: BoxDecoration(
                  color: const Color(0x14808080),
                  borderRadius: BorderRadius.circular(16),
                ),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    Flexible(
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          if (system)
                            Padding(
                              padding: const EdgeInsets.only(bottom: 6),
                              child: Text(
                                message.createdAt.isEmpty
                                    ? 'Sistema · sin guardar'
                                    : 'Sistema',
                                style: const TextStyle(
                                  fontSize: 11,
                                  fontWeight: FontWeight.w500,
                                ),
                              ),
                            ),
                          if (user || system)
                            Text(
                              message.text,
                              style: const TextStyle(
                                fontSize: 14,
                                height: 1.65,
                              ),
                            )
                          else
                            MarkdownBody(
                              data: message.text,
                              selectable: false,
                              styleSheet: MarkdownStyleSheet(
                                p: const TextStyle(
                                  fontFamily: 'Inter',
                                  fontSize: 14,
                                  height: 1.65,
                                ),
                              ),
                              onTapLink: (text, href, title) {
                                final uri = Uri.tryParse(href ?? '');
                                if (uri != null &&
                                    ['https', 'http'].contains(uri.scheme)) {
                                  unawaited(
                                    launchUrl(
                                      uri,
                                      mode: LaunchMode.externalApplication,
                                    ),
                                  );
                                }
                              },
                            ),
                        ],
                      ),
                    ),
                    if (time.isNotEmpty)
                      Padding(
                        padding: const EdgeInsets.only(left: 12),
                        child: Transform.translate(
                          offset: const Offset(0, 6),
                          child: Text(
                            time,
                            style: TextStyle(
                              fontSize: 10,
                              color: Theme.of(
                                context,
                              ).colorScheme.onSurface.withValues(alpha: .5),
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
