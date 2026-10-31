import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'assignment_execution_test.dart' as fixture;
import 'widget_test.dart' as base;
import 'package:gennie/src/backend.dart' show AssignmentRecord;

Finder textField(String label) => find.widgetWithText(TextFormField, label);

EditableText editor(WidgetTester tester, String label) =>
    tester.widget<EditableText>(
      find.descendant(
        of: textField(label),
        matching: find.byType(EditableText),
      ),
    );

class ComplexFixture extends fixture.ExecutionFixture {
  ComplexFixture() : super(preparationOnly: true);
  @override
  Future<String> pluginRequest(String text) async => jsonEncode({
    'server': {
      'revision': 2,
      'tools': {
        'read': {
          'description': 'Consulta ficticia estructurada',
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
      },
    },
  });
}

class MultipleFunctionsFixture extends fixture.ExecutionFixture {
  MultipleFunctionsFixture() : super(preparationOnly: true) {
    document['method']['functions'] = ['read', 'other'];
    final record = value.assignments.single;
    value = base.snapshot(
      plans: [
        AssignmentRecord(
          id: record.id,
          instanceId: record.instanceId,
          revision: record.revision,
          createdAt: record.createdAt,
          updatedAt: record.updatedAt,
          document: jsonEncode(document),
        ),
      ],
    );
  }
  @override
  Future<String> pluginRequest(String text) async => jsonEncode({
    'server': {
      'revision': 2,
      'tools': {
        for (final name in ['read', 'other'])
          name: {
            'title': name == 'read' ? 'Primera consulta' : 'Segunda consulta',
            'description': 'Consulta ficticia',
            'inputSchema': {
              'type': 'object',
              'properties': {},
              'additionalProperties': false,
            },
          },
      },
    },
  });
}

void main() {
  testWidgets(
    'el retraso inválido recibe foco aunque se editó otro campo después',
    (tester) async {
      final backend = await fixture.mount(
        tester,
        backend: fixture.ExecutionFixture(preparationOnly: true),
      );
      await fixture.press(tester, 'Configurar ejecución');
      await tester.ensureVisible(textField('Retraso máximo en minutos'));
      await tester.enterText(textField('Retraso máximo en minutos'), '0');
      await tester.ensureVisible(textField('Consulta'));
      await tester.enterText(textField('Consulta'), 'novedades ficticias');
      await fixture.press(tester, 'Guardar pausada');
      expect(
        find.text('Usa entre 1 y 1440 minutos.').hitTestable(),
        findsOneWidget,
      );
      expect(
        editor(tester, 'Retraso máximo en minutos').focusNode.hasFocus,
        isTrue,
      );
      expect(editor(tester, 'Consulta').controller.text, 'novedades ficticias');
      expect(
        backend.requests.where((r) => r['action'] == 'configure'),
        isEmpty,
      );
      await tester.enterText(textField('Retraso máximo en minutos'), '10');
      await tester.pumpAndSettle();
      expect(find.text('Usa entre 1 y 1440 minutos.'), findsNothing);
      expect(
        backend.requests.where((r) => r['action'] == 'configure'),
        isEmpty,
      );
      await fixture.press(tester, 'Guardar pausada');
      final saved =
          backend.requests.singleWhere(
                (r) => r['action'] == 'configure',
              )['configuration']
              as Map;
      expect(saved['max_lateness_minutes'], 10);
      expect(saved['enabled'], false);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );

  testWidgets('el JSON inválido recibe foco desde el último campo editado', (
    tester,
  ) async {
    final backend = await fixture.mount(tester, backend: ComplexFixture());
    await fixture.press(tester, 'Configurar ejecución');
    await tester.ensureVisible(textField('Parámetros (JSON avanzado)'));
    await tester.enterText(textField('Parámetros (JSON avanzado)'), '[');
    await tester.ensureVisible(textField('Retraso máximo en minutos'));
    await tester.enterText(textField('Retraso máximo en minutos'), '10');
    await fixture.press(tester, 'Guardar pausada');
    expect(
      find.text('Introduce un objeto JSON.').hitTestable(),
      findsOneWidget,
    );
    expect(
      editor(tester, 'Parámetros (JSON avanzado)').focusNode.hasFocus,
      isTrue,
    );
    expect(backend.requests.where((r) => r['action'] == 'configure'), isEmpty);
    await tester.enterText(
      textField('Parámetros (JSON avanzado)'),
      '{"items":[]}',
    );
    await tester.pumpAndSettle();
    expect(find.text('Introduce un objeto JSON.'), findsNothing);
    expect(backend.requests.where((r) => r['action'] == 'configure'), isEmpty);
    expect(tester.takeException(), isNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets(
    'el horario ausente se puede elegir por teclado después del rechazo',
    (tester) async {
      final backend = await fixture.mount(tester);
      await fixture.press(tester, 'Configurar ejecución');
      await tester.ensureVisible(textField('Consulta'));
      await tester.enterText(textField('Consulta'), 'novedades ficticias');
      await fixture.press(tester, 'Guardar pausada');
      expect(find.text('Elige un horario.').hitTestable(), findsOneWidget);
      expect(
        backend.requests.where((r) => r['action'] == 'configure'),
        isEmpty,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await tester.pumpAndSettle();
      expect(
        find.text('Horario de entrega del encargo').hitTestable(),
        findsOneWidget,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.text('Configurar ejecución'), findsWidgets);
      expect(editor(tester, 'Consulta').controller.text, 'novedades ficticias');
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await tester.pumpAndSettle();
      await tester.tap(
        find.text('Horario de entrega del encargo').hitTestable(),
      );
      await tester.pumpAndSettle();
      expect(find.text('Elige un horario.'), findsNothing);
      expect(
        backend.requests.where((r) => r['action'] == 'configure'),
        isEmpty,
      );
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );
  testWidgets('el error de un parámetro desaparece al corregir sin guardar', (
    tester,
  ) async {
    final backend = await fixture.mount(
      tester,
      backend: fixture.ExecutionFixture(preparationOnly: true),
    );
    await fixture.press(tester, 'Configurar ejecución');
    expect(find.text('Completa Consulta.'), findsNothing);
    await fixture.press(tester, 'Guardar pausada');
    expect(find.text('Completa Consulta.').hitTestable(), findsOneWidget);
    await tester.enterText(textField('Consulta'), 'novedades ficticias');
    await tester.pumpAndSettle();
    expect(find.text('Completa Consulta.'), findsNothing);
    expect(editor(tester, 'Consulta').controller.text, 'novedades ficticias');
    expect(backend.requests.where((r) => r['action'] == 'configure'), isEmpty);
    expect(tester.takeException(), isNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
  testWidgets('la función ausente recibe foco y retira el error al elegir', (
    tester,
  ) async {
    final backend = await fixture.mount(
      tester,
      backend: MultipleFunctionsFixture(),
    );
    await fixture.press(tester, 'Configurar ejecución');
    await fixture.press(tester, 'Guardar pausada');
    expect(find.text('Elige una función.').hitTestable(), findsOneWidget);
    expect(backend.requests.where((r) => r['action'] == 'configure'), isEmpty);
    await tester.sendKeyEvent(LogicalKeyboardKey.space);
    await tester.pumpAndSettle();
    expect(find.text('Primera consulta').hitTestable(), findsOneWidget);
    await tester.tap(find.text('Primera consulta').hitTestable());
    await tester.pumpAndSettle();
    expect(find.text('Elige una función.'), findsNothing);
    expect(backend.requests.where((r) => r['action'] == 'configure'), isEmpty);
    expect(tester.takeException(), isNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
}
