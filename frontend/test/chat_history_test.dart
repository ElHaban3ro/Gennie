import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/chat_history.dart';

void main() {
  final now = DateTime(2026, 10, 25, 0, 1);
  String label(DateTime date) =>
      conversationDateLabel(date.toUtc().toIso8601String(), now: now);
  test('Fecha local: días calendario, semana, fecha corta y dato ausente', () {
    expect(label(DateTime(2026, 10, 25)), 'Hoy');
    expect(label(DateTime(2026, 10, 24, 23, 59)), 'Ayer');
    expect(label(DateTime(2026, 10, 23)), 'Antier');
    expect(label(DateTime(2026, 10, 22)), 'Jueves');
    expect(label(DateTime(2026, 10, 19)), 'Lunes');
    expect(label(DateTime(2026, 10, 18)), '18/10/2026');
    expect(label(DateTime(2027, 1, 1)), '01/01/2027');
    expect(conversationDateLabel('', now: now), '');
    expect(conversationDateLabel('inválida', now: now), '');
  });

  for (final width in [216.0, 420.0]) {
    testWidgets('Título y último mensaje limitados sin overflow a $width px', (
      tester,
    ) async {
      await tester.pumpWidget(
        MaterialApp(
          home: Center(
            child: SizedBox(
              width: width,
              child: ChatHistoryTile(
                chat: Conversation(
                  id: 'test',
                  title: 'Título largo ' * 30,
                  updatedAt: DateTime(2026, 9, 1).toUtc().toIso8601String(),
                  lastMessage:
                      'Último mensaje\ncon saltos\t de línea y emojis 🦀 ' * 30,
                ),
                now: now,
                selected: true,
                onSelect: () {},
                onRename: () {},
                onDelete: () {},
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(tester.getSize(find.byType(ChatHistoryTile)).height, 64);
      final title = find.text('Título largo ' * 30);
      final preview = find.textContaining('Último mensaje');
      for (final finder in [title, preview]) {
        final text = tester.widget<Text>(finder);
        expect(text.maxLines, 1);
        expect(text.overflow, TextOverflow.ellipsis);
      }
      expect(tester.widget<Text>(preview).data, isNot(contains('\n')));
      expect(
        tester.getRect(title).right,
        lessThanOrEqualTo(tester.getRect(find.text('01/09/2026')).left),
      );
    });
  }

  testWidgets('Menú por teclado conserva selección y permite eliminar', (
    tester,
  ) async {
    var selections = 0, deletions = 0;
    await tester.pumpWidget(
      MaterialApp(
        home: ChatHistoryTile(
          chat: const Conversation(
            id: 'test',
            title: 'Chat',
            updatedAt: '',
            lastMessage: null,
          ),
          selected: false,
          onSelect: () => selections++,
          onRename: () {},
          onDelete: () => deletions++,
        ),
      ),
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.f10);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.pumpAndSettle();
    expect(find.text('Cambiar nombre'), findsOneWidget);
    expect(selections, 0);
    await tester.tap(find.text('Eliminar chat'));
    await tester.pumpAndSettle();
    expect(deletions, 1);
    expect(selections, 0);
  });
}
