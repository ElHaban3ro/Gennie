import 'package:flutter/material.dart';

// Contenido común para la presentación Flutter y el panel nativo de macOS.
@immutable
class AppTooltipDetails {
  const AppTooltipDetails({
    required this.title,
    required this.status,
    required this.statusColor,
    required this.memory,
    this.message,
    this.footer,
  });
  final String title;
  final String status;
  final Color statusColor;
  final String memory;
  final String? message;
  final String? footer;

  String get semanticLabel =>
      [title, status, memory, ?message, ?footer].join('\n\n');

  Map<String, Object?> toMap() => {
    'title': title,
    'status': status,
    'statusColor': statusColor.toARGB32(),
    'memory': memory,
    'message': message,
    'footer': footer,
  };
}

class AppTooltipContent extends StatelessWidget {
  const AppTooltipContent({super.key, required this.details});
  final AppTooltipDetails details;

  @override
  Widget build(BuildContext context) {
    final ink = Theme.of(context).colorScheme.onSurface;
    final body = TextStyle(
      fontFamily: 'Inter',
      fontSize: 12,
      fontWeight: FontWeight.w400,
      height: 1.45,
      color: ink.withValues(alpha: .88),
    );
    return SizedBox(
      width: 300,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: Text(
                  details.title,
                  key: const ValueKey('tooltip-title'),
                  style: body.copyWith(
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                    color: ink,
                  ),
                ),
              ),
              const SizedBox(width: 12),
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Container(
                    key: const ValueKey('tooltip-status-dot'),
                    width: 7,
                    height: 7,
                    decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      color: details.statusColor,
                    ),
                  ),
                  const SizedBox(width: 5),
                  Text(
                    details.status,
                    key: const ValueKey('tooltip-status'),
                    style: body.copyWith(
                      fontSize: 11,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  Text('  ·  ', style: body.copyWith(fontSize: 11)),
                  Text(
                    details.memory,
                    key: const ValueKey('tooltip-memory'),
                    style: body.copyWith(fontSize: 11),
                  ),
                ],
              ),
            ],
          ),
          if (details.message != null) ...[
            const SizedBox(height: 12),
            Text(
              details.message!,
              key: const ValueKey('tooltip-message'),
              style: body,
            ),
          ],
          if (details.footer != null) ...[
            const SizedBox(height: 12),
            Divider(height: 1, thickness: 1, color: ink.withValues(alpha: .12)),
            const SizedBox(height: 10),
            Text(
              details.footer!,
              key: const ValueKey('tooltip-footer'),
              style: body.copyWith(
                fontSize: 11,
                color: ink.withValues(alpha: .75),
              ),
            ),
          ],
        ],
      ),
    );
  }
}
