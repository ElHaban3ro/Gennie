import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/semantics.dart';
import 'backend.dart';
import 'sidebar.dart';

// Comparar días de calendario evita errores por horas y cambios de horario.
String conversationDateLabel(String timestamp, {required DateTime now}) {
  final parsed = DateTime.tryParse(timestamp);
  if (parsed == null) return '';
  final date = parsed.toLocal();
  final localNow = now.toLocal();
  final days = DateTime.utc(
    localNow.year,
    localNow.month,
    localNow.day,
  ).difference(DateTime.utc(date.year, date.month, date.day)).inDays;
  if (days == 0) return 'Hoy';
  if (days == 1) return 'Ayer';
  if (days == 2) return 'Antier';
  if (days >= 3 && days <= 6) {
    const weekdays = [
      'Lunes',
      'Martes',
      'Miércoles',
      'Jueves',
      'Viernes',
      'Sábado',
      'Domingo',
    ];
    return weekdays[date.weekday - 1];
  }
  return '${date.day.toString().padLeft(2, '0')}/${date.month.toString().padLeft(2, '0')}/${date.year}';
}

class ChatHistoryTile extends StatelessWidget {
  const ChatHistoryTile({
    super.key,
    required this.chat,
    required this.selected,
    required this.onSelect,
    required this.onRename,
    required this.onDelete,
    this.now,
  });
  final Conversation chat;
  final bool selected;
  final VoidCallback onSelect, onRename, onDelete;
  final DateTime? now;

  Future<void> _menu(BuildContext context, Offset position) async {
    final overlay =
        Overlay.of(context).context.findRenderObject()! as RenderBox;
    final action = await showMenu<String>(
      context: context,
      position: RelativeRect.fromRect(
        position & const Size(1, 1),
        Offset.zero & overlay.size,
      ),
      items: const [
        PopupMenuItem(value: 'rename', child: Text('Cambiar nombre')),
        PopupMenuItem(value: 'delete', child: Text('Eliminar chat')),
      ],
    );
    if (!context.mounted) return;
    if (action == 'rename') onRename();
    if (action == 'delete') onDelete();
  }

  @override
  Widget build(BuildContext context) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    final ink = dark ? const Color(0xffe3e3e1) : const Color(0xff333333);
    final date = conversationDateLabel(
      chat.updatedAt,
      now: now ?? DateTime.now(),
    );
    final preview =
        chat.lastMessage?.replaceAll(RegExp(r'\s+'), ' ').trim() ?? '';
    void keyboardMenu() {
      final box = context.findRenderObject()! as RenderBox;
      _menu(context, box.localToGlobal(box.size.center(Offset.zero)));
    }

    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.f10, shift: true):
            keyboardMenu,
        const SingleActivator(LogicalKeyboardKey.contextMenu): keyboardMenu,
      },
      child: Semantics(
        selected: selected,
        customSemanticsActions: {
          const CustomSemanticsAction(label: 'Cambiar nombre'): onRename,
          const CustomSemanticsAction(label: 'Eliminar chat'): onDelete,
        },
        child: GestureDetector(
          onSecondaryTapUp: (details) => _menu(context, details.globalPosition),
          child: SidebarTile(
            title: chat.title,
            subtitle: preview,
            selected: selected,
            onSelect: onSelect,
            trailing: date.isEmpty
                ? null
                : Text(
                    date,
                    maxLines: 1,
                    softWrap: false,
                    style: TextStyle(
                      fontFamily: 'Inter',
                      fontSize: 10,
                      fontWeight: FontWeight.w600,
                      color: ink.withValues(alpha: .65),
                    ),
                  ),
          ),
        ),
      ),
    );
  }
}
