import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/chat_message.dart';

void main() {
  test('Fechas del chat por calendario local y hora sin fecha completa', () {
    final now = DateTime(2026, 10, 25, 0, 1);
    String label(DateTime date) =>
        chatDayLabel(date.toUtc().toIso8601String(), now: now);
    expect(label(now), 'Hoy');
    expect(label(DateTime(2026, 10, 24, 23, 59)), 'Ayer');
    expect(label(DateTime(2026, 10, 23)), 'Antier');
    expect(label(DateTime(2026, 10, 16)), 'viernes, 16 de octubre');
    expect(label(DateTime(2025, 12, 31)), 'miércoles, 31 de diciembre de 2025');
    expect(chatDayLabel('inválida', now: now), '');
    expect(messageTimeLabel(''), '');
    expect(
      messageTimeLabel(DateTime(2026, 10, 25, 9, 5).toUtc().toIso8601String()),
      '09:05',
    );
    expect(
      messageLocalDay(DateTime(2026, 10, 25, 23).toUtc().toIso8601String()),
      DateTime(2026, 10, 25),
    );
  });

  for (final width in [256.0, 736.0, 1200.0]) {
    for (final speaker in ['user', 'assistant', 'system']) {
      for (final content in [
        'Hola',
        List.filled(12, 'Mensaje largo').join(' '),
      ]) {
        testWidgets('$speaker: ancho 40–70 % en $width con $content', (
          tester,
        ) async {
          await tester.binding.setSurfaceSize(Size(width + 64, 1400));
          addTearDown(() => tester.binding.setSurfaceSize(null));
          await tester.pumpWidget(
            MaterialApp(
              home: Scaffold(
                body: Center(
                  child: SizedBox(
                    width: width,
                    child: ChatMessage(
                      message: Message(
                        speaker: speaker,
                        text: content,
                        createdAt: DateTime(
                          2026,
                          10,
                          25,
                          9,
                          5,
                        ).toUtc().toIso8601String(),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          );
          expect(tester.takeException(), isNull);
          final container = find.descendant(
            of: find.byType(ChatMessage),
            matching: find.byType(Container),
          );
          final bounds = tester.getRect(container);
          expect(bounds.width, greaterThanOrEqualTo(width * .4 - .01));
          expect(bounds.width, lessThanOrEqualTo(width * .7 + .01));
          if (content == 'Hola') {
            if (width >= 736) {
              expect(bounds.width, closeTo(width * .4, .01));
            }
            final text = tester.getRect(find.text('Hola', findRichText: true));
            final time = tester.getRect(find.text('09:05'));
            expect(time.top, lessThan(text.bottom));
            expect(time.bottom, greaterThan(text.top));
            expect(time.left, greaterThan(text.right));
          } else {
            expect(bounds.width, closeTo(width * .7, .01));
          }
        });
      }
    }
  }

  for (final speaker in ['user', 'assistant']) {
    testWidgets('$speaker: texto y hora en la última línea al ancho mínimo', (
      tester,
    ) async {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: Center(
              child: SizedBox(
                width: 256,
                child: SelectionArea(
                  child: ChatMessage(
                    message: Message(
                      speaker: speaker,
                      text:
                          'Mensaje de varias líneas para comprobar su presentación.',
                      createdAt: DateTime(
                        2026,
                        10,
                        25,
                        9,
                        5,
                      ).toUtc().toIso8601String(),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      );
      expect(tester.takeException(), isNull);
      expect(find.text('Tú'), findsNothing);
      expect(find.text('Gennie'), findsNothing);
      final text = tester.getRect(
        find.text(
          'Mensaje de varias líneas para comprobar su presentación.',
          findRichText: true,
        ),
      );
      final time = tester.getRect(find.text('09:05'));
      expect(time.top, greaterThan(text.top));
      expect(time.bottom, closeTo(text.bottom + 6, .01));
      final bubble = tester.getRect(
        find.descendant(
          of: find.byType(ChatMessage),
          matching: find.byType(Container),
        ),
      );
      expect(bubble.height, closeTo(text.height + 24, .01));
      expect(time.left, greaterThan(text.right));
      expect(tester.widget<Text>(find.text('09:05')).style!.fontSize, 10);
    });
  }
}
