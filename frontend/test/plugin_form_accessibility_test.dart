import 'dart:async';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/plugins.dart';

Map<String, dynamic> function() => {
  'name': 'echo',
  'description': 'Herramienta ficticia sin efectos externos.',
  'transport': 'mcp',
  'path': 'echo',
  'parameters': {
    'message': {
      'type': 'string',
      'label': 'Mensaje',
      'description': 'Texto literal de la prueba.',
    },
    'count': {'type': 'integer', 'label': 'Cantidad', 'required': false},
  },
};

Finder field(String name) => find.descendant(
  of: find.byKey(ValueKey('plugin-param-$name')),
  matching: find.byType(TextField),
);

Future<void> mount(
  WidgetTester tester,
  Future<Map<String, dynamic>> Function(Map<String, dynamic>) call,
) async {
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: SingleChildScrollView(
          child: FunctionProbe(
            record: {
              'manifest': {'id': 'fixture'},
              'revision': 1,
              'verified': {'echo': true},
            },
            function: function(),
            call: call,
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

void expectField(
  WidgetTester tester,
  String name,
  String label,
  String value, {
  bool editable = true,
}) {
  final data = tester.getSemantics(field(name)).getSemanticsData();
  expect(data.label, contains(label));
  expect(data.label.split('\n').where((line) => line == label), hasLength(1));
  expect(data.value, value);
  expect(data.flagsCollection.isTextField, isTrue);
  expect(data.flagsCollection.isReadOnly, !editable);
  expect(data.hasAction(ui.SemanticsAction.focus), editable);
  expect(
    data.hasAction(ui.SemanticsAction.setText),
    editable && data.flagsCollection.isFocused == ui.Tristate.isTrue,
  );
}

void main() {
  testWidgets(
    'los parámetros conservan nombre y valor al editar, limpiar y validar',
    (tester) async {
      final semantics = tester.ensureSemantics();
      try {
        var calls = 0;
        await mount(tester, (_) async {
          calls++;
          return {'status': 'ok'};
        });
        expectField(tester, 'message', 'Mensaje', '');
        expectField(tester, 'count', 'Cantidad (opcional)', '');
        final message = tester.getSemantics(field('message'));
        message.owner!.performAction(message.id, ui.SemanticsAction.focus);
        await tester.pumpAndSettle();
        expectField(tester, 'message', 'Mensaje', '');
        final focused = tester.getSemantics(field('message'));
        focused.owner!.performAction(
          focused.id,
          ui.SemanticsAction.setText,
          'Texto ficticio 😀',
        );
        await tester.pumpAndSettle();
        await tester.enterText(field('count'), '0');
        await tester.pumpAndSettle();
        expectField(tester, 'message', 'Mensaje', 'Texto ficticio 😀');
        expectField(tester, 'count', 'Cantidad (opcional)', '0');
        await tester.enterText(field('message'), '');
        await tester.pumpAndSettle();
        await tester.tap(find.text('Ejecutar función'));
        await tester.pumpAndSettle();
        expectField(tester, 'message', 'Mensaje', '');
        expect(find.text('Completa Mensaje.'), findsOneWidget);
        expect(calls, 0);
        expect(tester.takeException(), isNull);
      } finally {
        semantics.dispose();
      }
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );

  testWidgets(
    'los parámetros conservan nombre y borrador durante IO y su rechazo',
    (tester) async {
      final semantics = tester.ensureSemantics();
      try {
        final pending = Completer<Map<String, dynamic>>();
        final calls = <Map<String, dynamic>>[];
        await mount(tester, (request) {
          calls.add(request);
          return pending.future;
        });
        await tester.enterText(field('message'), 'Borrador ficticio');
        await tester.enterText(field('count'), '0');
        await tester.pumpAndSettle();
        await tester.tap(find.text('Ejecutar función'));
        await tester.pump();
        expectField(
          tester,
          'message',
          'Mensaje',
          'Borrador ficticio',
          editable: false,
        );
        expectField(
          tester,
          'count',
          'Cantidad (opcional)',
          '0',
          editable: false,
        );
        expect(calls, hasLength(1));
        pending.complete({
          'status': 'invalid_arguments',
          'executed': false,
          'fields': [
            {'name': 'message', 'reason': 'type'},
          ],
        });
        await tester.pumpAndSettle();
        expectField(tester, 'message', 'Mensaje', 'Borrador ficticio');
        expect(find.text('Revisa Mensaje antes de ejecutar.'), findsOneWidget);
        expect(calls, hasLength(1));
        expect(tester.takeException(), isNull);
      } finally {
        semantics.dispose();
      }
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );

  testWidgets('el JSON MCP mantiene etiqueta y valor tras validar y bloquear', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    try {
      var calls = 0;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SingleChildScrollView(
              child: McpToolProbe(
                definition: {
                  'name': 'batch',
                  'description': 'Caso ficticio de argumentos complejos.',
                  'inputSchema': {
                    'type': 'object',
                    'properties': {
                      'items': {
                        'type': 'array',
                        'items': {'type': 'integer'},
                      },
                    },
                    'required': ['items'],
                  },
                },
                call: (_) async {
                  calls++;
                  return {'status': 'result_uncertain'};
                },
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      final input = find.descendant(
        of: find.byKey(const ValueKey('mcp-arguments')),
        matching: find.byType(TextField),
      );
      void check(String value, {bool editable = true}) {
        final data = tester.getSemantics(input).getSemanticsData();
        expect(data.label, contains('Argumentos (objeto JSON)'));
        expect(data.value, value);
        expect(data.flagsCollection.isTextField, isTrue);
        expect(data.flagsCollection.isReadOnly, !editable);
        expect(data.hasAction(ui.SemanticsAction.focus), editable);
      }

      check('{}');
      await tester.enterText(input, '[');
      await tester.pumpAndSettle();
      await tester.tap(find.text('Ejecutar herramienta'));
      await tester.pumpAndSettle();
      check('[');
      expect(find.text('Introduce un objeto JSON válido.'), findsOneWidget);
      expect(calls, 0);
      const arguments = '{"items":[0,1]}';
      await tester.enterText(input, arguments);
      await tester.pumpAndSettle();
      await tester.tap(find.text('Ejecutar herramienta'));
      await tester.pumpAndSettle();
      check(arguments, editable: false);
      expect(calls, 1);
      expect(find.textContaining('Resultado pendiente.'), findsOneWidget);
      final button = tester
          .getSemantics(
            find.widgetWithText(FilledButton, 'Ejecutar herramienta'),
          )
          .getSemanticsData();
      expect(button.flagsCollection.isEnabled, ui.Tristate.isFalse);
      expect(button.hasAction(ui.SemanticsAction.tap), isFalse);
      expect(tester.takeException(), isNull);
    } finally {
      semantics.dispose();
    }
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
}
