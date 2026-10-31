import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/main.dart';
import 'package:gennie/src/backend.dart';
import 'assistant_chat_bridge_test.dart' show BridgeBackend, ensureLibrary;
import 'mcp_review_bridge_test.dart' show pumpBridgeUntil;
import 'chats_preview_test.dart' show loadFonts;
import 'widget_test.dart' show rail;

void main() {
  const library = String.fromEnvironment('GENNIE_PROPOSALS_BRIDGE_LIBRARY');
  const helper = String.fromEnvironment('GENNIE_PROPOSALS_BRIDGE_HELPER');
  const env = String.fromEnvironment('GENNIE_PROPOSALS_BRIDGE_ENV');
  const fromChat = bool.fromEnvironment('GENNIE_PROPOSALS_FROM_CHAT');
  var number = 0;
  for (final brightness in Brightness.values) {
    for (final size in [const Size(1100, 720), const Size(640, 420)]) {
      final id = ++number;
      testWidgets('propuestas FRB real ${brightness.name} ${size.width}', (
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
          await t.tap(rail('Encargos'));
          await t.pump();
          await pumpBridgeUntil(
            t,
            () => find.text('Resumen real $id').evaluate().isNotEmpty,
          );
          await t.tap(find.text('Resumen real $id').first);
          await t.pump();
          await pumpBridgeUntil(
            t,
            () => find.text('Mejora real $id').evaluate().isNotEmpty,
          );
          final proposal = find.text('Mejora real $id');
          await Scrollable.ensureVisible(t.element(proposal), alignment: .5);
          await t.pumpAndSettle();
          await t.tap(proposal);
          await t.pumpAndSettle();
          if (fromChat) {
            expect(find.text('Solicitud del chat'), findsOneWidget);
            expect(
              find.text(
                'Genera propuestas para Resumen real $id, conservando las fuentes y permisos actuales.',
              ),
              findsOneWidget,
            );
            final source = jsonDecode(
              (await t.runAsync(
                () => backend.proposalRequest(
                  jsonEncode({'action': 'get', 'review_id': 'review-$id'}),
                ),
              ))!,
            );
            expect(
              source['chat_request']['text'],
              source['scope']['instructions'],
            );
            expect(source['scope']['use_guide'], false);
            expect(source['proposals'][0]['resolution'], 'pending');
          }
          expect(
            find.text('Propuesto: Agrupar solo los cambios nuevos'),
            findsOneWidget,
          );
          expect(find.text('Aplicar cambios').hitTestable(), findsOneWidget);
          await t.runAsync(() async {
            final render =
                boundary.currentContext!.findRenderObject()!
                    as RenderRepaintBoundary;
            final image = await render.toImage();
            final bytes = await image.toByteData(
              format: ui.ImageByteFormat.png,
            );
            File(
              '../.verification/assistant-chat-real/${fromChat ? 'chat-review-real' : 'proposals-real'}-${brightness.name}-${size.width.toInt()}.png',
            ).writeAsBytesSync(bytes!.buffer.asUint8List());
            image.dispose();
          });
          await t.tap(find.text('Aplicar cambios'));
          await t.pump();
          await pumpBridgeUntil(
            t,
            () => find.byType(Dialog).evaluate().isEmpty,
          );
          final result = jsonDecode(
            (await t.runAsync(
              () => backend.proposalRequest(
                jsonEncode({'action': 'list', 'assignment_id': id}),
              ),
            ))!,
          );
          expect(result['reviews'][0]['proposals'][0]['resolution'], 'applied');
          expect(result['reviews'][0]['proposals'][0]['applied_revision'], 2);
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
