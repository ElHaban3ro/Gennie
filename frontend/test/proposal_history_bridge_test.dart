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
  const library = String.fromEnvironment('GENNIE_PROPOSAL_HISTORY_LIBRARY');
  const helper = String.fromEnvironment('GENNIE_PROPOSAL_HISTORY_HELPER');
  const env = String.fromEnvironment('GENNIE_PROPOSAL_HISTORY_ENV');
  var number = 0;
  for (final brightness in Brightness.values) {
    for (final size in [const Size(1100, 720), const Size(640, 420)]) {
      final id = ++number;
      testWidgets('archivo FRB ${brightness.name} ${size.width}', (t) async {
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
        Future<Map<String, dynamic>> request(Map<String, dynamic> r) async =>
            Map<String, dynamic>.from(
              jsonDecode(await backend.proposalRequest(jsonEncode(r))) as Map,
            );
        Future<void> press(Finder finder) async {
          await Scrollable.ensureVisible(t.element(finder), alignment: .5);
          await t.pumpAndSettle();
          await t.tap(finder);
          await t.pump();
        }

        Future<void> capture(String phase) async {
          await t.runAsync(() async {
            final render =
                boundary.currentContext!.findRenderObject()!
                    as RenderRepaintBoundary;
            final image = await render.toImage();
            final bytes = await image.toByteData(
              format: ui.ImageByteFormat.png,
            );
            File(
              '../.verification/assistant-chat-real/proposal-history-real-${brightness.name}-${size.width.toInt()}-$phase.png',
            ).writeAsBytesSync(bytes!.buffer.asUint8List());
            image.dispose();
          });
        }

        try {
          await t.runAsync(state.start);
          expect(state.failure, isNull);
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
          await press(find.text('Cargar revisiones anteriores'));
          await pumpBridgeUntil(
            t,
            () => find.text('20 de 24 revisiones').evaluate().isNotEmpty,
          );
          await press(find.text('Cargar revisiones anteriores'));
          await pumpBridgeUntil(
            t,
            () => find.text('24 de 24 revisiones').evaluate().isNotEmpty,
          );
          expect(find.text('Historial real $id 00'), findsOneWidget);
          await Scrollable.ensureVisible(
            t.element(find.text('Historial real $id 00')),
            alignment: .5,
          );
          await t.pumpAndSettle();
          await capture('pages');
          await press(find.text('Archivar revisión').first);
          await t.pumpAndSettle();
          await capture('confirm');
          await t.tap(find.text('Archivar'));
          await t.pump();
          await pumpBridgeUntil(
            t,
            () =>
                find.byType(Dialog).evaluate().isEmpty &&
                find.text('10 de 23 revisiones').evaluate().isNotEmpty,
          );
          await press(find.text('Revisiones archivadas'));
          await pumpBridgeUntil(
            t,
            () => find.text('1 de 1 revisiones').evaluate().isNotEmpty,
          );
          await press(find.text('Mejora real $id'));
          await t.pumpAndSettle();
          await capture('detail');
          expect(
            t
                .widget<FilledButton>(
                  find.widgetWithText(FilledButton, 'Aplicar cambios'),
                )
                .onPressed,
            isNull,
          );
          expect(
            t
                .widget<TextButton>(
                  find.widgetWithText(TextButton, 'Descartar'),
                )
                .onPressed,
            isNull,
          );
          await t.tap(find.text('Cerrar'));
          await t.pump();
          await pumpBridgeUntil(
            t,
            () =>
                find.byType(Dialog).evaluate().isEmpty &&
                find.byType(LinearProgressIndicator).evaluate().isEmpty,
          );
          final review = (await t.runAsync(
            () => request({'action': 'get', 'review_id': 'review-$id'}),
          ))!;
          expect(review['archived'], true);
          expect(review['proposals'][0]['resolution'], 'pending');
          final denied = await t.runAsync(() async {
            try {
              await request({
                'action': 'resolve',
                'proposal_id': review['proposals'][0]['id'],
                'apply': false,
              });
              return false;
            } catch (_) {
              return true;
            }
          });
          expect(denied, true);
          await t.runAsync(
            () => request({'action': 'archive', 'review_id': 'review-$id'}),
          );
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
