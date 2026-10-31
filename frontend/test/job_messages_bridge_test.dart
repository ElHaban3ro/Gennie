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
import 'mcp_review_bridge_test.dart' show pumpBridgeUntil;
import 'widget_test.dart' show rail;

void main() {
  const library = String.fromEnvironment('GENNIE_JOB_MESSAGES_LIBRARY');
  const helper = String.fromEnvironment('GENNIE_JOB_MESSAGES_HELPER');
  const env = String.fromEnvironment('GENNIE_JOB_MESSAGES_ENV');
  var count = 0;
  for (final brightness in Brightness.values) {
    for (final size in [const Size(1100, 720), const Size(640, 420)]) {
      final number = ++count;
      testWidgets(
        'ficha por FRB ${brightness.name} ${size.width}: solo texto confirmado del bot',
        (t) async {
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
              () => find.text('invalid.example').evaluate().isNotEmpty,
            );
            await t.tap(find.text('invalid.example').first);
            await t.pump();
            final message = find.text(
              'La preparación se interrumpió. Puedes revisar el trabajo y continuar cuando quieras.',
            );
            await pumpBridgeUntil(t, () => message.evaluate().isNotEmpty);
            expect(find.textContaining('PRIVATE_HELPER_TEXT'), findsNothing);
            final result = jsonDecode(
              (await t.runAsync(
                () => backend.pluginRequest(
                  jsonEncode({'action': 'get_job', 'job_id': 1}),
                ),
              ))!,
            );
            expect(result['job']['bot_presentation']['status'], 'ready');
            expect(
              result['job']['bot_presentation']['messages'][0]['message_id'],
              3,
            );
            expect(
              result['job']['bot_presentation']['messages'][0]['sequence'],
              1,
            );
            final history = find.text('Últimas actualizaciones de Gennie');
            await t.ensureVisible(history);
            await t.tap(history);
            await t.pumpAndSettle();
            final body = find.textContaining(
              '1. La preparación se interrumpió.',
            );
            await Scrollable.ensureVisible(t.element(body), alignment: .25);
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
                '../.verification/assistant-chat-real/job-messages-real-${brightness.name}-${size.width.toInt()}.png',
              ).writeAsBytesSync(bytes!.buffer.asUint8List());
              image.dispose();
            });
            if (number == 4) {
              await t.runAsync(() => backend.delete('1'));
              await pumpBridgeUntil(
                t,
                () => find
                    .textContaining('El chat de origen ya no está disponible.')
                    .evaluate()
                    .isNotEmpty,
              );
              expect(message, findsNothing);
              expect(find.textContaining('PRIVATE_HELPER_TEXT'), findsNothing);
              final stale = jsonDecode(
                (await t.runAsync(
                  () => backend.pluginRequest(
                    jsonEncode({'action': 'get_job', 'job_id': 1}),
                  ),
                ))!,
              );
              expect(
                stale['job']['bot_presentation']['status'],
                'origin_unavailable',
              );
            }
            expect(t.takeException(), isNull);
          } finally {
            await t.pumpWidget(const SizedBox());
            await t.runAsync(state.close);
            state.dispose();
          }
        },
        skip: library.isEmpty || helper.isEmpty || env.isEmpty,
      );
    }
  }
}
