import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/main.dart';
import 'package:gennie/src/backend.dart';
import 'assistant_chat_bridge_test.dart' show BridgeBackend, ensureLibrary;
import 'chats_preview_test.dart' show loadFonts;
import 'widget_test.dart' show rail;

Future<void> pumpBridgeUntil(WidgetTester t, bool Function() done) async {
  for (var i = 0; i < 100; i++) {
    await t.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 20)),
    );
    // Las respuestas FFI usan tiempo real; la transición de ruta usa el reloj del test.
    await t.pump(const Duration(milliseconds: 20));
    if (done()) return;
  }
  throw StateError('El flujo MCP no alcanzó el estado esperado');
}

void main() {
  const library = String.fromEnvironment('GENNIE_MCP_BRIDGE_LIBRARY');
  const helper = String.fromEnvironment('GENNIE_MCP_BRIDGE_HELPER');
  const env = String.fromEnvironment('GENNIE_MCP_BRIDGE_ENV');
  var number = 0;
  for (final brightness in Brightness.values) {
    for (final size in [const Size(1100, 720), const Size(640, 420)]) {
      final id = ++number;
      testWidgets('revisión MCP FRB real ${brightness.name} ${size.width}', (
        t,
      ) async {
        t.view.physicalSize = size;
        t.view.devicePixelRatio = 1;
        t.platformDispatcher.platformBrightnessTestValue = brightness;
        addTearDown(t.view.resetPhysicalSize);
        addTearDown(t.view.resetDevicePixelRatio);
        addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
        await t.runAsync(() async {
          await ensureLibrary(library);
          await loadFonts();
        });
        final backend = BridgeBackend(env, helper);
        final state = DesktopState(backend);
        final boundary = GlobalKey();
        try {
          await t.runAsync(state.start);
          expect(state.failure, isNull);
          expect(state.snapshot?.assistant, isNull);
          await t.pumpWidget(
            RepaintBoundary(
              key: boundary,
              child: GennieApp(state: state),
            ),
          );
          await t.pump();
          await t.tap(rail('Plugins'));
          await t.pump();
          await pumpBridgeUntil(
            t,
            () => find.text('MCP real de prueba $id').evaluate().isNotEmpty,
          );
          await t.tap(find.text('MCP real de prueba $id').first);
          await t.pump();
          await pumpBridgeUntil(
            t,
            () => find.text('Resultados pendientes · 1').evaluate().isNotEmpty,
          );
          final before = jsonDecode(
            (await t.runAsync(
              () => backend.pluginRequest(
                jsonEncode({'action': 'mcp_get', 'server_id': 'fixture_$id'}),
              ),
            ))!,
          );
          expect(before['server']['pending_attempts'][0]['state'], 'uncertain');
          expect(
            before['server']['pending_attempts'][0]['reason'],
            'interrupted',
          );
          final blocked = jsonDecode(
            (await t.runAsync(
              () => backend.pluginRequest(
                jsonEncode({
                  'action': 'mcp_run',
                  'server_id': 'fixture_$id',
                  'tool': 'echo',
                  'expected_revision': 1,
                  'arguments': {'message': 'NO SE ENVÍA'},
                }),
              ),
            ))!,
          );
          expect(blocked['status'], 'review_required');
          expect(blocked['executed'], false);
          final review = find
              .widgetWithText(TextButton, 'Revisar resultado')
              .first;
          await t.ensureVisible(review);
          await t.tap(review);
          await t.pumpAndSettle();
          final choice = find.text('Comprobé que no se aplicó');
          await t.ensureVisible(choice);
          await t.tap(choice);
          await t.pumpAndSettle();
          await t.runAsync(() async {
            final render =
                boundary.currentContext!.findRenderObject()!
                    as RenderRepaintBoundary;
            final image = await render.toImage();
            final bytes = await image.toByteData(
              format: ui.ImageByteFormat.png,
            );
            File(
              '../.verification/assistant-chat-real/mcp-review-real-${brightness.name}-${size.width.toInt()}.png',
            ).writeAsBytesSync(bytes!.buffer.asUint8List());
            image.dispose();
          });
          await t.tap(find.text('Guardar revisión'));
          await t.pump();
          await pumpBridgeUntil(
            t,
            () => find.byType(Dialog).evaluate().isEmpty,
          );
          final after = jsonDecode(
            (await t.runAsync(
              () => backend.pluginRequest(
                jsonEncode({'action': 'mcp_get', 'server_id': 'fixture_$id'}),
              ),
            ))!,
          );
          expect(after['server']['pending_count'], 0);
          expect(after['server']['revision'], 2);
          expect(
            after['server']['recent_reviews'][0]['resolution'],
            'not_applied',
          );
          expect(state.snapshot?.busy, false);
          expect(t.takeException(), isNull);
        } finally {
          await t.pumpWidget(const SizedBox());
          await t.runAsync(state.close);
          state.dispose();
        }
      }, skip: library.isEmpty || env.isEmpty || helper.isEmpty);
    }
  }
}
