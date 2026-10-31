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

// Sonda optativa con SQLite ficticio; llm/browser permanecen deshabilitados.
void main() {
  const library = String.fromEnvironment('GENNIE_BOT_BRIDGE_LIBRARY');
  const helper = String.fromEnvironment('GENNIE_BOT_BRIDGE_HELPER');
  const env = String.fromEnvironment('GENNIE_BOT_BRIDGE_ENV');
  for (final brightness in Brightness.values) {
    for (final size in [const Size(1100, 720), const Size(640, 420)]) {
      testWidgets('aviso bot FRB real ${brightness.name} ${size.width}', (
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
          expect(state.snapshot?.activeId, '2');
          await t.pumpWidget(
            RepaintBoundary(
              key: boundary,
              child: GennieApp(state: state),
            ),
          );
          await pumpBridgeUntil(
            t,
            () => find.text('Continuar').evaluate().isNotEmpty,
          );
          expect(
            find.text('Actualización de una conversación anterior'),
            findsOneWidget,
          );
          expect(find.textContaining('PRIVATE_HELPER_TEXT'), findsNothing);
          final page = await t.runAsync(() => backend.loadChatPage(null));
          final notice = page!.messages.singleWhere(
            (m) => m.delegation != null,
          );
          expect(notice.conversationId, '1');
          expect(notice.message.speaker, 'assistant');
          expect(notice.delegation!.jobId, '1');
          expect(notice.delegation!.sourceSequence, '1');
          expect(notice.delegation!.pluginId, 'fixture_notice');
          final button = find.text('Continuar');
          // El borde del lector tiene un fundido visual; centrar evita capturar
          // una acción dentro de esa franja aunque todavía acepte hit testing.
          await Scrollable.ensureVisible(t.element(button), alignment: .5);
          await t.pumpAndSettle();
          expect(button.hitTestable(), findsOneWidget);
          expect(t.getRect(button).bottom, lessThan(size.height - 140));
          expect(t.getRect(button).top, greaterThan(96));
          final render =
              boundary.currentContext!.findRenderObject()!
                  as RenderRepaintBoundary;
          await t.runAsync(() async {
            final image = await render.toImage();
            final bytes = await image.toByteData(
              format: ui.ImageByteFormat.png,
            );
            Directory(
              '../.verification/assistant-chat-real',
            ).createSync(recursive: true);
            File(
              '../.verification/assistant-chat-real/bot-notice-real-${brightness.name}-${size.width.toInt()}.png',
            ).writeAsBytesSync(bytes!.buffer.asUint8List());
            image.dispose();
          });
          await t.tap(button);
          await t.pump();
          await pumpBridgeUntil(
            t,
            () => find
                .textContaining('No se pudo realizar la acción:')
                .evaluate()
                .isNotEmpty,
          );
          expect(state.snapshot?.activeId, '2');
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
