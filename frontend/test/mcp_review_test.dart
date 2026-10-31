import 'dart:convert';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'mcp_plugins_test.dart' as fixture;
import 'plugins_test.dart' show mount;
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show measureContrast;
import 'widget_test.dart' show rail;

fixture.McpBackend pendingBackend({bool active = false, int count = 1}) {
  final backend = fixture.McpBackend();
  backend.server['pending_attempts'] = [
    for (var i = 1; i <= count; i++)
      fixture.pendingAttempt(
        i,
        tool: i == 1 ? 'echo' : 'archived_$i',
        active: active,
      ),
  ];
  backend.server['pending_count'] = count;
  return backend;
}

Future<void> openReview(WidgetTester t) async =>
    fixture.click(t, 'Revisar resultado');
Finder dialogText(String text) =>
    find.descendant(of: find.byType(Dialog), matching: find.text(text));

void main() {
  testWidgets(
    'el bloqueo vuelve al cambiar de herramienta, pestaña y reabrir la ficha',
    (t) async {
      final b = pendingBackend();
      await mount(t, b);
      await fixture.open(t);
      await fixture.tool(t, 'echo');
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Ejecutar función'),
            )
            .onPressed,
        isNull,
      );
      await fixture.tool(t, 'nested');
      await fixture.tool(t, 'echo');
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Ejecutar función'),
            )
            .onPressed,
        isNull,
      );
      await t.tap(rail('Chats'));
      await t.pumpAndSettle();
      await t.tap(rail('Plugins'));
      await t.pumpAndSettle();
      await fixture.tool(t, 'echo');
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Ejecutar función'),
            )
            .onPressed,
        isNull,
      );
      expect(b.effects, isEmpty);
      await openReview(t);
      await fixture.click(t, 'Cancelar');
      expect(b.requests.where((r) => r['action'] == 'mcp_reconcile'), isEmpty);
      expect(b.server['pending_count'], 1);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets('una llamada aún activa no se puede revisar ni repetir', (
    t,
  ) async {
    final b = pendingBackend(active: true);
    await mount(t, b);
    await fixture.open(t);
    await fixture.tool(t, 'echo');
    for (final button in t.widgetList<TextButton>(
      find.widgetWithText(TextButton, 'Revisar resultado'),
    )) {
      expect(button.onPressed, isNull);
    }
    expect(
      t
          .widget<FilledButton>(
            find.widgetWithText(FilledButton, 'Ejecutar función'),
          )
          .onPressed,
      isNull,
    );
    expect(b.effects, isEmpty);
    expect(t.takeException(), isNull);
  });
  testWidgets(
    'la revisión no preselecciona, conserva elección ante error y no ejecuta',
    (t) async {
      final b = pendingBackend()..failReview = true;
      await mount(t, b);
      await fixture.open(t);
      await fixture.tool(t, 'echo');
      await openReview(t);
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Guardar revisión'),
            )
            .onPressed,
        isNull,
      );
      await fixture.click(t, 'La acción se aplicó');
      await fixture.click(t, 'Guardar revisión');
      expect(
        find.descendant(
          of: find.byType(Dialog),
          matching: find.textContaining(
            'La revisión cambió. Recarga la ficha.',
          ),
        ),
        findsOneWidget,
      );
      expect(
        t
            .widget<RadioGroup<String>>(find.byType(RadioGroup<String>))
            .groupValue,
        'applied',
      );
      expect(b.server['pending_count'], 1);
      expect(b.effects, isEmpty);
      b.failReview = false;
      await fixture.click(t, 'Guardar revisión');
      expect(find.byType(Dialog), findsNothing);
      expect(b.server['pending_count'], 0);
      expect(b.server['recent_reviews'][0]['resolution'], 'applied');
      expect(
        b.requests
            .where((r) => r['action'] == 'mcp_reconcile')
            .last['expected_revision'],
        1,
      );
      expect(b.effects, isEmpty);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets('la revisión preserva cambios de permisos todavía sin guardar', (
    t,
  ) async {
    final b = pendingBackend();
    await mount(t, b);
    await fixture.open(t);
    await t.ensureVisible(find.widgetWithText(CheckboxListTile, 'nested'));
    await t.tap(find.text('nested').first);
    await t.pumpAndSettle();
    await openReview(t);
    await fixture.click(t, 'Comprobé que no se aplicó');
    await fixture.click(t, 'Guardar revisión');
    final checkbox = t.widget<CheckboxListTile>(
      find.widgetWithText(CheckboxListTile, 'nested'),
    );
    expect(checkbox.value, true);
    expect(b.server['allowed_tools'], ['echo']);
    expect(b.requests.where((r) => r['action'] == 'mcp_permissions'), isEmpty);
    expect(b.effects, isEmpty);
    expect(t.takeException(), isNull);
  });
  testWidgets(
    'los intentos pendientes se cargan por páginas, incluso de funciones retiradas',
    (t) async {
      final b = pendingBackend(count: 52);
      await mount(t, b);
      await fixture.open(t);
      expect(find.text('archived_51'), findsNothing);
      await fixture.click(t, 'Cargar más intentos');
      expect(find.text('archived_51'), findsOneWidget);
      expect(find.text('archived_52'), findsOneWidget);
      expect(find.text('Cargar más intentos'), findsNothing);
      expect(
        b.requests.singleWhere((r) => r['action'] == 'mcp_attempts')['after'],
        50,
      );
      expect(b.effects, isEmpty);
      expect(t.takeException(), isNull);
    },
  );
  for (final brightness in Brightness.values) {
    for (final (size, scale) in [
      (const Size(1100, 720), 1.0),
      (const Size(640, 420), 1.0),
      (const Size(640, 420), 1.5),
    ]) {
      testWidgets(
        'revisión MCP ${brightness.name} ${size.width} texto $scale',
        (t) async {
          t.platformDispatcher.textScaleFactorTestValue = scale;
          addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
          await loadFonts();
          final key = GlobalKey();
          final b = pendingBackend();
          await mount(t, b, size: size, brightness: brightness, boundary: key);
          await fixture.open(t);
          await fixture.tool(t, 'echo');
          await openReview(t);
          await fixture.click(t, 'Comprobé que no se aplicó');
          expect(t.takeException(), isNull);
          final ratios = await measureContrast(t, {
            'titulo': dialogText('Revisar resultado'),
            'guardar': dialogText('Guardar revisión'),
          }, captureBoundary: find.byKey(key));
          final name =
              'mcp-review-${brightness.name}-${size.width.toInt()}-$scale';
          await fixture.capture(t, key, name);
          if (const bool.fromEnvironment('CAPTURE_UI')) {
            File(
              '../.verification/assistant-chat-real/$name-contrast.json',
            ).writeAsStringSync(jsonEncode(ratios));
          }
          await fixture.click(t, 'Guardar revisión');
          expect(b.effects, isEmpty);
          expect(b.server['pending_count'], 0);
          expect(t.takeException(), isNull);
        },
      );
    }
  }
}
