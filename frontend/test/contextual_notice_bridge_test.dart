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
  const library = String.fromEnvironment('GENNIE_CONTEXTUAL_LIBRARY');
  const helper = String.fromEnvironment('GENNIE_CONTEXTUAL_HELPER');
  const env = String.fromEnvironment('GENNIE_CONTEXTUAL_ENV');
  var number = 0;
  for (final brightness in Brightness.values) {
    for (final size in [const Size(1100, 720), const Size(640, 420)]) {
      final id = ++number;
      testWidgets('guía contextual FRB ${brightness.name} ${size.width}', (
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
            () => find.text('Ejecución real $id').evaluate().isNotEmpty,
          );
          await t.tap(find.text('Ejecución real $id').first);
          await t.pump();
          await pumpBridgeUntil(
            t,
            () => find.text('Desactivar guía y avisos').evaluate().isNotEmpty,
          );
          Future<void> press(String label) async {
            final f = find.text(label).last;
            await Scrollable.ensureVisible(t.element(f), alignment: .5);
            await pumpBridgeUntil(
              t,
              () =>
                  f.hitTestable().evaluate().isNotEmpty &&
                  find.byType(LinearProgressIndicator).evaluate().isEmpty,
            );
            await t.tap(f);
            await t.pump();
          }

          Future<void> capture(String suffix) async {
            await t.pump(kThemeAnimationDuration);
            await t.runAsync(() async {
              final render =
                  boundary.currentContext!.findRenderObject()!
                      as RenderRepaintBoundary;
              final image = await render.toImage();
              final bytes = await image.toByteData(
                format: ui.ImageByteFormat.png,
              );
              File(
                '../.verification/assistant-chat-real/contextual-real-${brightness.name}-${size.width.toInt()}-$suffix.png',
              ).writeAsBytesSync(bytes!.buffer.asUint8List());
              image.dispose();
            });
          }

          expect(
            t
                .widget<TextButton>(
                  find.widgetWithText(TextButton, 'Configurar ejecución'),
                )
                .onPressed,
            isNull,
          );
          await press('Ver ejecuciones');
          await pumpBridgeUntil(
            t,
            () => find.text('Ver detalle').evaluate().isNotEmpty,
          );
          await press('Ver detalle');
          await pumpBridgeUntil(
            t,
            () =>
                find.text('Resultado guardado sin aviso').evaluate().isNotEmpty,
          );
          await Scrollable.ensureVisible(
            t.element(find.text('Resultado guardado sin aviso')),
            alignment: .5,
          );
          await t.pump();
          expect(
            find.textContaining('La valoración no quedó confirmada'),
            findsOneWidget,
          );
          expect(find.text('Guardar comprobación'), findsNothing);
          await capture('status');
          await press('Cerrar');
          await press('Cerrar');
          await press('Desactivar guía y avisos');
          await pumpBridgeUntil(
            t,
            () =>
                find.text('Desactivar guía y avisos').evaluate().isEmpty &&
                find.byType(LinearProgressIndicator).evaluate().isEmpty,
          );
          await Scrollable.ensureVisible(
            t.element(find.text('Preparación · Pausada')),
            alignment: .1,
          );
          await t.pump();
          await capture('revoked');
          final raw = (await t.runAsync(
            () => backend.executionRequest(
              jsonEncode({'action': 'run_details', 'run_id': id}),
            ),
          ))!;
          final detail = jsonDecode(raw) as Map;
          expect(detail['state'], 'succeeded');
          expect(detail['resolution'], isNull);
          expect(detail['configuration']['contextual_notice'], true);
          expect(detail['notification']['action'], 'record');
          final listed =
              jsonDecode(
                    (await t.runAsync(
                      () => backend.executionRequest(
                        jsonEncode({'action': 'list', 'assignment_id': id}),
                      ),
                    ))!,
                  )
                  as Map;
          final current = (listed['policies'] as List).singleWhere(
            (p) => p['configuration']['trigger'] == 'preparation',
          );
          expect(current['revision'], 2);
          expect(current['configuration']['contextual_notice'], false);
          expect(current['configuration']['delivery'], isNull);
          expect(current['enabled'], false);
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
