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
  const library = String.fromEnvironment('GENNIE_REFINEMENT_LIBRARY');
  const helper = String.fromEnvironment('GENNIE_REFINEMENT_HELPER');
  const env = String.fromEnvironment('GENNIE_REFINEMENT_ENV');
  var number = 0;
  for (final brightness in Brightness.values) {
    for (final size in [const Size(1100, 720), const Size(640, 420)]) {
      final id = ++number;
      testWidgets('versiones FRB ${brightness.name} ${size.width}', (t) async {
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
        Future<Map<String, dynamic>> get(String review) async =>
            Map<String, dynamic>.from(
              jsonDecode(
                    await backend.proposalRequest(
                      jsonEncode({'action': 'get', 'review_id': review}),
                    ),
                  )
                  as Map,
            );
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
              '../.verification/assistant-chat-real/refinement-real-${brightness.name}-${size.width.toInt()}-$phase.png',
            ).writeAsBytesSync(bytes!.buffer.asUint8List());
            image.dispose();
          });
        }

        try {
          await t.runAsync(state.start);
          expect(state.failure, isNull);
          final parent = (await t.runAsync(() => get('review-$id')))!;
          final parentProposal = parent['proposals'][0]['id'];
          expect(parent['version']['next_review_id'], 'continued-$id');
          expect(parent['current'], false);
          final denied = await t.runAsync(() async {
            try {
              await backend.proposalRequest(
                jsonEncode({
                  'action': 'resolve',
                  'proposal_id': parentProposal,
                  'apply': true,
                }),
              );
              return false;
            } catch (_) {
              return true;
            }
          });
          expect(denied, true);
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
          await capture('previous');
          final next = find.text('Ver versión posterior');
          await Scrollable.ensureVisible(t.element(next), alignment: .5);
          await t.pumpAndSettle();
          await t.tap(next);
          await t.pump();
          await pumpBridgeUntil(
            t,
            () => find.text('Propuesta acordada $id').evaluate().isNotEmpty,
          );
          await t.pumpAndSettle();
          expect(
            t
                .widget<FilledButton>(
                  find.widgetWithText(FilledButton, 'Aplicar cambios'),
                )
                .onPressed,
            isNotNull,
          );
          await capture('current');
          await t.tap(find.text('Aplicar cambios'));
          await t.pump();
          await pumpBridgeUntil(
            t,
            () => find.byType(Dialog).evaluate().isEmpty,
          );
          final child = (await t.runAsync(() => get('continued-$id')))!;
          expect(child['proposals'][0]['resolution'], 'applied');
          expect(child['proposals'][0]['applied_revision'], 2);
          final receipt = jsonDecode(
            (await t.runAsync(
              () => backend.proposalRequest(
                jsonEncode({
                  'action': 'resolve',
                  'proposal_id': child['proposals'][0]['id'],
                  'apply': true,
                }),
              ),
            ))!,
          );
          expect(receipt['applied_revision'], 2);
          expect(
            (await t.runAsync(
              () => get('review-$id'),
            ))!['proposals'][0]['resolution'],
            'pending',
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
