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
  const library = String.fromEnvironment('GENNIE_EXECUTION_BRIDGE_LIBRARY');
  const helper = String.fromEnvironment('GENNIE_EXECUTION_BRIDGE_HELPER');
  const env = String.fromEnvironment('GENNIE_EXECUTION_BRIDGE_ENV');
  var number = 0;
  for (final brightness in Brightness.values) {
    for (final size in [const Size(1100, 720), const Size(640, 420)]) {
      final id = ++number;
      testWidgets('ejecución FRB real ${brightness.name} ${size.width}', (
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
            () => find.text('Ejecución real $id').evaluate().isNotEmpty,
          );
          await t.tap(find.text('Ejecución real $id').first);
          await t.pump();
          await pumpBridgeUntil(
            t,
            () => find.text('Pausar ejecuciones').evaluate().isNotEmpty,
          );
          Future<void> press(String label) async {
            final f = find.text(label).first;
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

          final disabled = t.widget<TextButton>(
            find.widgetWithText(TextButton, 'Configurar ejecución'),
          );
          expect(disabled.onPressed, isNull);
          await press('Pausar ejecuciones');
          await pumpBridgeUntil(
            t,
            () => find.text('Pausar ejecuciones').evaluate().isEmpty,
          );
          await press('Ver ejecuciones');
          await pumpBridgeUntil(
            t,
            () => find.text('Revisar resultado').evaluate().isNotEmpty,
          );
          await press('Revisar resultado');
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
          await press('Comprobé que no se aplicó');
          expect(
            find.textContaining('Solo la fuente de prueba'),
            findsOneWidget,
          );
          await pumpBridgeUntil(
            t,
            () =>
                ModalRoute.of(
                  t.element(find.text('Guardar comprobación')),
                )?.animation?.isCompleted ==
                true,
          );
          // Completa la transición de color del botón tras elegir el resultado.
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
              '../.verification/assistant-chat-real/execution-real-${brightness.name}-${size.width.toInt()}.png',
            ).writeAsBytesSync(bytes!.buffer.asUint8List());
            image.dispose();
          });
          await press('Guardar comprobación');
          await pumpBridgeUntil(
            t,
            () => find.text('Comprobado: no se aplicó').evaluate().isNotEmpty,
          );
          final detail = jsonDecode(
            (await t.runAsync(
              () => backend.executionRequest(
                jsonEncode({'action': 'run_details', 'run_id': id}),
              ),
            ))!,
          );
          expect(detail['resolution'], 'not_applied');
          expect(detail['configuration']['arguments']['query'], 'Original $id');
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
