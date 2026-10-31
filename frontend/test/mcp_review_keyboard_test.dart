import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/plugins.dart';

Map<String, dynamic> attempt() => {
  'id': 1,
  'source_id': 'fixture',
  'source_revision': 1,
  'server_name': 'Servidor ficticio',
  'tool': 'echo',
  'state': 'uncertain',
  'active': false,
  'created_at': '2026-10-29T00:00:00Z',
};

Future<FocusNode> openReview(
  WidgetTester tester,
  Future<void> Function(String) confirm,
) async {
  final focus = FocusNode();
  addTearDown(focus.dispose);
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: Builder(
          builder: (context) => FilledButton(
            focusNode: focus,
            onPressed: () => showDialog<void>(
              context: context,
              barrierDismissible: false,
              builder: (_) => McpAttemptReviewDialog(
                attempt: attempt(),
                onConfirm: confirm,
              ),
            ),
            child: const Text('Abrir revisión'),
          ),
        ),
      ),
    ),
  );
  focus.requestFocus();
  await tester.pump();
  await tester.sendKeyEvent(LogicalKeyboardKey.space);
  await tester.pumpAndSettle();
  expect(find.byType(McpAttemptReviewDialog), findsOneWidget);
  return focus;
}

void main() {
  testWidgets(
    'Escape cancela revisión MCP libre sin resolver y devuelve foco',
    (tester) async {
      var confirmations = 0;
      final focus = await openReview(tester, (_) async {
        confirmations++;
      });
      expect(
        tester
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Guardar revisión'),
            )
            .onPressed,
        isNull,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.byType(McpAttemptReviewDialog), findsNothing);
      expect(confirmations, 0);
      expect(focus.hasPrimaryFocus, isTrue);
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await tester.pumpAndSettle();
      expect(find.byType(McpAttemptReviewDialog), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );

  testWidgets(
    'Escape espera al guardado MCP y vuelve a cancelar después del error',
    (tester) async {
      final pending = Completer<void>();
      final choices = <String>[];
      await openReview(tester, (choice) {
        choices.add(choice);
        return pending.future;
      });
      await tester.tap(find.text('La acción se aplicó'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Guardar revisión'));
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.byType(McpAttemptReviewDialog), findsOneWidget);
      expect(
        tester
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Guardando…'),
            )
            .onPressed,
        isNull,
      );
      expect(choices, ['applied']);
      pending.completeError(StateError('Fallo ficticio de guardado'));
      await tester.pumpAndSettle();
      expect(find.textContaining('Fallo ficticio'), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.byType(McpAttemptReviewDialog), findsNothing);
      expect(choices, ['applied']);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );
}
