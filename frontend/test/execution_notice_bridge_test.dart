import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/main.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/chat_message.dart';
import 'package:gennie/src/chat_timeline.dart';
import 'assistant_chat_bridge_test.dart' show BridgeBackend, ensureLibrary;
import 'mcp_review_bridge_test.dart' show pumpBridgeUntil;
import 'chats_preview_test.dart' show loadFonts;

void main() {
  const library = String.fromEnvironment('GENNIE_EXECUTION_NOTICE_LIBRARY');
  const helper = String.fromEnvironment('GENNIE_EXECUTION_NOTICE_HELPER');
  const env = String.fromEnvironment('GENNIE_EXECUTION_NOTICE_ENV');
  var number = 0;
  for (final brightness in Brightness.values) {
    for (final size in [const Size(1100, 720), const Size(640, 420)]) {
      final id = ++number;
      testWidgets('aviso de ejecución FRB ${brightness.name} ${size.width}', (
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
          expect(state.snapshot?.activeId, '2');
          await t.pumpWidget(
            RepaintBoundary(
              key: boundary,
              child: GennieApp(state: state),
            ),
          );
          await pumpBridgeUntil(
            t,
            () => find.byType(ChatTimeline).evaluate().isNotEmpty,
          );
          await pumpBridgeUntil(
            t,
            () => find.text('Ver ejecución del encargo').evaluate().isNotEmpty,
          );
          final message = find.byWidgetPredicate(
            (w) => w is ChatMessage && w.execution?.runId == '$id',
          );
          final scroll = t
              .widget<ChatTimeline>(find.byType(ChatTimeline))
              .controller;
          for (var n = 0; n < 60 && message.evaluate().isEmpty; n++) {
            scroll.jumpTo(
              (scroll.offset + 120).clamp(0, scroll.position.maxScrollExtent),
            );
            await t.pump(const Duration(milliseconds: 20));
          }
          expect(message, findsOneWidget);
          final link = find.descendant(
            of: message,
            matching: find.text('Ver ejecución del encargo'),
          );
          await Scrollable.ensureVisible(t.element(link), alignment: .5);
          await t.pumpAndSettle();
          final page = await t.runAsync(() => backend.loadChatPage(null));
          final entry = page!.messages.singleWhere(
            (m) => m.execution?.runId == '$id',
          );
          expect(entry.conversationId, '1');
          expect(entry.message.speaker, 'assistant');
          expect(
            entry.execution!.instanceId,
            id.toRadixString(16).padLeft(32, '0'),
          );
          expect(entry.delegation, isNull);
          Future<void> capture(String suffix) async {
            await t.runAsync(() async {
              final render =
                  boundary.currentContext!.findRenderObject()!
                      as RenderRepaintBoundary;
              final image = await render.toImage();
              final bytes = await image.toByteData(
                format: ui.ImageByteFormat.png,
              );
              File(
                '../.verification/assistant-chat-real/execution-notice-real-${brightness.name}-${size.width.toInt()}-$suffix.png',
              ).writeAsBytesSync(bytes!.buffer.asUint8List());
              image.dispose();
            });
          }

          await capture('chat');
          await t.tap(link);
          await t.pump();
          await pumpBridgeUntil(
            t,
            () => find.text('Guardar comprobación').evaluate().isNotEmpty,
          );
          expect(
            t
                .widget<FilledButton>(
                  find.widgetWithText(FilledButton, 'Guardar comprobación'),
                )
                .onPressed,
            isNull,
          );
          final radio = find.text('Comprobé que no se aplicó');
          await Scrollable.ensureVisible(t.element(radio), alignment: .5);
          await pumpBridgeUntil(
            t,
            () => radio.hitTestable().evaluate().isNotEmpty,
          );
          await t.tap(radio);
          await t.pump();
          await pumpBridgeUntil(
            t,
            () =>
                ModalRoute.of(
                  t.element(find.text('Guardar comprobación')),
                )?.animation?.isCompleted ==
                true,
          );
          await t.pump(kThemeAnimationDuration);
          await capture('review');
          await t.tap(find.text('Guardar comprobación'));
          await t.pump();
          await pumpBridgeUntil(
            t,
            () => find.byType(Dialog).evaluate().isEmpty,
          );
          final detail = jsonDecode(
            (await t.runAsync(
              () => backend.executionRequest(
                jsonEncode({'action': 'run_details', 'run_id': id}),
              ),
            ))!,
          );
          expect(detail['resolution'], 'not_applied');
          expect(state.snapshot?.activeId, '2');
          expect(t.takeException(), isNull);
        } finally {
          await t.pumpWidget(const SizedBox());
          await t.runAsync(state.close);
          state.dispose();
        }
      }, skip: library.isEmpty || helper.isEmpty || env.isEmpty);
    }
  }
}
