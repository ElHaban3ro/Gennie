// Panel de producción con FRB/SQLite; contenedor de prueba sin catálogo/navegación web.
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/plugins.dart';
import 'assistant_chat_bridge_test.dart' show BridgeBackend, ensureLibrary;
import 'mcp_review_bridge_test.dart' show pumpBridgeUntil;

void main() {
  const library = String.fromEnvironment('GENNIE_WEB_REVIEW_LIBRARY');
  const helper = String.fromEnvironment('GENNIE_WEB_REVIEW_HELPER');
  const env = String.fromEnvironment('GENNIE_WEB_REVIEW_ENV');
  for (final resolution in ['applied', 'not_applied']) {
    testWidgets('revisión web $resolution por controles y FRB sin operación', (
      t,
    ) async {
      t.view.physicalSize = const Size(640, 420);
      t.view.devicePixelRatio = 1;
      addTearDown(t.view.resetPhysicalSize);
      addTearDown(t.view.resetDevicePixelRatio);
      await t.runAsync(() => ensureLibrary(library));
      final backend = BridgeBackend(env, helper);
      final calls = <String>[];
      Future<Map<String, dynamic>> call(Map<String, dynamic> request) async {
        final action = request['action'] as String;
        if (!['web_attempts', 'web_reconcile'].contains(action)) {
          throw StateError(
            'La sonda no puede ejecutar o consultar el navegador',
          );
        }
        calls.add(action);
        return Map<String, dynamic>.from(
          jsonDecode(await backend.pluginRequest(jsonEncode(request))) as Map,
        );
      }

      try {
        final snapshot = await t.runAsync(backend.initialize);
        expect(snapshot!.assistant, isNull);
        final initial = (await t.runAsync(
          () => call({'action': 'web_attempts', 'plugin_id': 'fixture_notice'}),
        ))!;
        final chosen = (initial['attempts'] as List).first as Map;
        expect(chosen['active'], false);
        await t.pumpWidget(
          MaterialApp(
            theme: ThemeData(fontFamily: 'Inter', useMaterial3: true),
            home: Scaffold(
              body: SingleChildScrollView(
                child: WebAttemptPanel(
                  pluginId: 'fixture_notice',
                  initial: initial,
                  call: call,
                ),
              ),
            ),
          ),
        );
        await t.pump();
        final review = find.text('Revisar resultado').first;
        await t.ensureVisible(review);
        await t.tap(review);
        await t.pumpAndSettle();
        final option = find.text(
          resolution == 'applied'
              ? 'La acción se aplicó'
              : 'Comprobé que no se aplicó',
        );
        await t.ensureVisible(option);
        await t.tap(option);
        await t.pumpAndSettle();
        final save = find.text('Guardar revisión');
        expect(save.hitTestable(), findsOneWidget);
        await t.tap(save);
        await t.pump();
        await pumpBridgeUntil(t, () => find.byType(Dialog).evaluate().isEmpty);
        final after = (await t.runAsync(
          () => call({'action': 'web_attempts', 'plugin_id': 'fixture_notice'}),
        ))!;
        expect(after['total'], (initial['total'] as int) - 1);
        expect(
          (after['recent_reviews'] as List).any(
            (r) => r['id'] == chosen['id'] && r['resolution'] == resolution,
          ),
          true,
        );
        expect(calls.where((c) => c == 'web_reconcile').length, 1);
        expect(t.takeException(), isNull);
      } finally {
        await t.pumpWidget(const SizedBox());
        await t.runAsync(backend.shutdown);
      }
    }, skip: library.isEmpty || helper.isEmpty || env.isEmpty);
  }
}
