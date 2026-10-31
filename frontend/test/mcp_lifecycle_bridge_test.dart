import 'dart:convert';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/main.dart';
import 'package:gennie/src/backend.dart';
import 'assistant_chat_bridge_test.dart'
    show BridgeBackend, ensureLibrary, capture;
import 'chats_preview_test.dart' show loadFonts;
import 'mcp_review_bridge_test.dart' show pumpBridgeUntil;
import 'widget_test.dart' show rail;

class LifecycleBackend extends BridgeBackend {
  LifecycleBackend(super.env, super.helper);
  final responses = <Map<String, dynamic>>[];
  @override
  Future<String> pluginRequest(String request) async {
    late String result;
    try {
      result = await super.pluginRequest(request);
    } catch (e) {
      responses.add({
        'action': jsonDecode(request)['action'],
        'fixture_error': '$e',
      });
      rethrow;
    }
    responses.add({
      'action': jsonDecode(request)['action'],
      'result': jsonDecode(result),
    });
    return result;
  }
}

void main() {
  const library = String.fromEnvironment('GENNIE_MCP_LIFECYCLE_LIBRARY');
  const helper = String.fromEnvironment('GENNIE_MCP_LIFECYCLE_HELPER');
  const env = String.fromEnvironment('GENNIE_MCP_LIFECYCLE_ENV');
  const program = String.fromEnvironment('GENNIE_MCP_LIFECYCLE_PROGRAM');
  const args = String.fromEnvironment('GENNIE_MCP_LIFECYCLE_ARGS');
  const calls = String.fromEnvironment('GENNIE_MCP_LIFECYCLE_CALLS');
  testWidgets(
    'MCP real por FRB: conexión manual, permisos, incertidumbre y reapertura',
    (t) async {
      t.view.physicalSize = const Size(1100, 720);
      t.view.devicePixelRatio = 1;
      t.platformDispatcher.platformBrightnessTestValue = Brightness.dark;
      addTearDown(t.view.resetPhysicalSize);
      addTearDown(t.view.resetDevicePixelRatio);
      addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
      await t.runAsync(() async {
        await ensureLibrary(library);
        await loadFonts();
      });
      final backend = LifecycleBackend(env, helper);
      final state = DesktopState(backend);
      var closed = false;
      int sent() =>
          File(calls).existsSync() ? File(calls).readAsLinesSync().length : 0;
      Future<void> click(String text) async {
        final target = find.text(text).first;
        final button = find.ancestor(
          of: target,
          matching: find.byWidgetPredicate((w) => w is ButtonStyleButton),
        );
        if (button.evaluate().isNotEmpty) {
          await pumpBridgeUntil(
            t,
            () => t.widget<ButtonStyleButton>(button.first).onPressed != null,
          );
        }
        await t.ensureVisible(target);
        await t.tap(target);
        await t.pump();
      }

      Future<void> enter(String key, String value) async {
        final target = find.byKey(ValueKey(key));
        await t.ensureVisible(target);
        await t.enterText(target, value);
        await t.pump();
      }

      try {
        await t.runAsync(state.start);
        expect(state.failure, isNull);
        await t.pumpWidget(
          RepaintBoundary(
            key: const ValueKey('real-chat-capture'),
            child: GennieApp(state: state),
          ),
        );
        await t.pump();
        await t.tap(rail('Plugins'));
        await t.pump();
        await pumpBridgeUntil(
          t,
          () => find.byIcon(Icons.add).evaluate().isNotEmpty,
        );
        await t.tap(find.byIcon(Icons.add).last);
        await t.pumpAndSettle();
        await click('Conectar un servidor MCP existente');
        await enter('mcp-name', 'Servidor local de prueba');
        await t.tap(find.byKey(const ValueKey('mcp-transport')));
        await t.pumpAndSettle();
        await t.tap(find.text('Programa local · stdio').last);
        await t.pumpAndSettle();
        await enter('mcp-command', program);
        await enter('mcp-args', args);
        await click('Guardar conexión');
        await pumpBridgeUntil(
          t,
          () => find.text('Descubrir herramientas').evaluate().isNotEmpty,
        );
        final saved = backend.responses.lastWhere(
          (r) => r['action'] == 'mcp_save',
        )['result']['server'];
        final id = saved['specification']['id'];
        expect(saved['enabled'], false);
        expect(saved['tools'], isEmpty);
        expect(sent(), 0);
        await click('Descubrir herramientas');
        await pumpBridgeUntil(
          t,
          () =>
              find
                  .widgetWithText(CheckboxListTile, 'echo')
                  .evaluate()
                  .isNotEmpty &&
              t
                      .widget<CheckboxListTile>(
                        find.widgetWithText(CheckboxListTile, 'echo'),
                      )
                      .onChanged !=
                  null,
        );
        expect(sent(), 0);
        await click('Habilitar este plugin');
        await t.ensureVisible(find.widgetWithText(CheckboxListTile, 'echo'));
        await t.tap(find.widgetWithText(CheckboxListTile, 'echo'));
        await t.pump();
        await click('Guardar permisos');
        await pumpBridgeUntil(
          t,
          () => backend.responses.any((r) => r['action'] == 'mcp_permissions'),
        );
        await t.pump();
        expect(sent(), 0);
        await pumpBridgeUntil(
          t,
          () =>
              t
                  .widget<DropdownButtonFormField<String>>(
                    find.byKey(const ValueKey('mcp-tool')),
                  )
                  .onChanged !=
              null,
        );
        await t.ensureVisible(find.byKey(const ValueKey('mcp-tool')));
        await t.tap(find.byKey(const ValueKey('mcp-tool')));
        await t.pumpAndSettle();
        await t.tap(find.text('echo').last);
        await t.pumpAndSettle();
        await click('Ejecutar función');
        expect(find.text('Completa Mensaje.'), findsOneWidget);
        expect(sent(), 0);
        await enter('plugin-param-message', 'synthetic_payload');
        await click('Ejecutar función');
        await pumpBridgeUntil(
          t,
          () => backend.responses.any((r) => r['action'] == 'mcp_run'),
        );
        await t.pump();
        expect(
          backend.responses.lastWhere(
            (r) => r['action'] == 'mcp_run',
          )['result']['status'],
          'ok',
        );
        expect(sent(), 1);
        await Scrollable.ensureVisible(
          t.element(find.byKey(const ValueKey('plugin-result-status'))),
          alignment: .35,
        );
        await t.pump(const Duration(milliseconds: 300));
        await capture(t, 'mcp-lifecycle-success-dark-1100');
        await enter('plugin-param-message', 'desconectar');
        await click('Ejecutar función');
        await pumpBridgeUntil(
          t,
          () =>
              backend.responses.where((r) => r['action'] == 'mcp_run').length ==
              2,
        );
        await pumpBridgeUntil(
          t,
          () => find.text('Resultados pendientes · 1').evaluate().isNotEmpty,
        );
        final uncertain = backend.responses.lastWhere(
          (r) => r['action'] == 'mcp_run',
        )['result'];
        expect(uncertain['status'], 'result_uncertain');
        expect(uncertain['review_required'], true);
        expect(sent(), 2);
        final blocked = jsonDecode(
          (await t.runAsync(
            () => backend.pluginRequest(
              jsonEncode({
                'action': 'mcp_run',
                'server_id': id,
                'tool': 'echo',
                'expected_revision': backend.responses.lastWhere(
                  (r) => r['action'] == 'mcp_permissions',
                )['result']['server']['revision'],
                'arguments': {'message': 'NO_SE_ENVIA'},
              }),
            ),
          ))!,
        );
        // La revisión guardada actual es necesaria: una solicitud antigua se rechaza antes del efecto.
        expect(blocked['executed'], false);
        expect(blocked['status'], 'review_required');
        expect(sent(), 2);
        t.view.physicalSize = const Size(640, 420);
        t.platformDispatcher.platformBrightnessTestValue = Brightness.light;
        await t.pumpAndSettle();
        await t.ensureVisible(find.text('Resultados pendientes · 1'));
        await capture(t, 'mcp-lifecycle-uncertain-light-640');
        await click('Revisar resultado');
        await t.pumpAndSettle();
        expect(
          t
              .widget<FilledButton>(
                find.widgetWithText(FilledButton, 'Guardar revisión'),
              )
              .onPressed,
          isNull,
        );
        await click('La acción se aplicó');
        await t.pumpAndSettle();
        await capture(t, 'mcp-lifecycle-review-light-640');
        await click('Guardar revisión');
        await pumpBridgeUntil(t, () => find.byType(Dialog).evaluate().isEmpty);
        expect(sent(), 2);
        expect(t.takeException(), isNull);
        await t.pumpWidget(const SizedBox());
        await t.runAsync(state.close);
        closed = true;
        final reopened = BridgeBackend(env, helper);
        try {
          await t.runAsync(reopened.initialize);
          final source = jsonDecode(
            (await t.runAsync(
              () => reopened.pluginRequest(
                jsonEncode({'action': 'mcp_get', 'server_id': id}),
              ),
            ))!,
          )['server'];
          expect(source['pending_count'], 0);
          expect(source['recent_reviews'][0]['resolution'], 'applied');
          expect(source['connection_status'], 'disconnected');
          expect(sent(), 2);
        } finally {
          await t.runAsync(reopened.shutdown);
        }
      } finally {
        await t.pumpWidget(const SizedBox());
        if (!closed) await t.runAsync(state.close);
        state.dispose();
      }
    },
    skip:
        library.isEmpty ||
        env.isEmpty ||
        helper.isEmpty ||
        program.isEmpty ||
        calls.isEmpty,
  );
}
