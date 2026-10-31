import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'plugins_test.dart' show PluginsBackend, mount, select;
import 'widget_test.dart' show snapshot;
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show measureContrast;

Map<String, dynamic> job({String? status = 'ready', bool message = true}) => {
  'id': 7,
  'plugin_id': 'gmail_test',
  'origin': 'https://example.com',
  'state': 'interrupted',
  'objective': 'Consultar novedades',
  'trigger_conversation_id': 1,
  'attempts': 1,
  'max_attempts': 8,
  'summary': 'PRIVATE_HELPER_SUMMARY',
  'events': [
    {
      'id': 4,
      'message': 'PRIVATE_HELPER_STEP',
      'created_at': '2026-10-29T13:00:00Z',
    },
  ],
  'updated_at': '2026-10-29T13:00:00Z',
  if (status != null)
    'bot_presentation': {
      'status': status,
      'messages': [
        if (message)
          {
            'message_id': 9,
            'sequence': 4,
            'text': 'He conservado el trabajo para retomarlo cuando quieras.',
            'created_at': '2026-10-29T12:00:00Z',
          },
      ],
    },
};

void main() {
  testWidgets(
    'metadatos de bot inválidos no habilitan texto técnico como reserva',
    (t) async {
      final backend = PluginsBackend()..jobs = [job()];
      (backend.jobs[0]['bot_presentation']['messages'][0] as Map)['sequence'] =
          0;
      await mount(t, backend);
      await select(t);
      expect(find.textContaining('PRIVATE_HELPER'), findsNothing);
      expect(
        find.textContaining('Recarga la ficha para reintentar.'),
        findsOneWidget,
      );
      expect(
        find.text('He conservado el trabajo para retomarlo cuando quieras.'),
        findsNothing,
      );
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'una actualización del bot durante una lectura provoca otra consulta',
    (t) async {
      final backend = PluginsBackend()..jobs = [job(message: false)];
      final state = await mount(t, backend);
      await select(t);
      final before = backend.requests
          .where((r) => r['action'] == 'list')
          .length;
      final pending = Completer<String>();
      backend.pendingList = pending;
      await t.tap(find.byIcon(Icons.refresh));
      await t.pump();
      backend.events.add(snapshot(executionRevision: 'bot-update'));
      await t.pump();
      backend.jobs = [job()];
      backend.pendingList = null;
      pending.complete(
        jsonEncode({
          'plugins': backend.records,
          'jobs': [job(message: false)],
        }),
      );
      await t.pumpAndSettle();
      expect(
        backend.requests.where((r) => r['action'] == 'list').length,
        greaterThanOrEqualTo(before + 2),
      );
      expect(
        find.text('He conservado el trabajo para retomarlo cuando quieras.'),
        findsOneWidget,
      );
      await t.pumpWidget(const SizedBox());
      await t.runAsync(state.close);
      state.dispose();
    },
  );
  testWidgets(
    'la ficha presenta solo mensajes confirmados del bot y usa su nombre',
    (t) async {
      final backend = PluginsBackend()..value = snapshot(title: 'Mi asistente');
      backend.jobs = [job()];
      await mount(t, backend);
      await select(t);
      expect(find.text('PRIVATE_HELPER_SUMMARY'), findsNothing);
      expect(find.text('PRIVATE_HELPER_STEP'), findsNothing);
      expect(
        find.text('He conservado el trabajo para retomarlo cuando quieras.'),
        findsOneWidget,
      );
      final history = find.text('Últimas actualizaciones de Mi asistente');
      await t.ensureVisible(history);
      await t.tap(history);
      await t.pumpAndSettle();
      expect(
        find.textContaining(
          'He conservado el trabajo para retomarlo cuando quieras.',
        ),
        findsNWidgets(2),
      );
      expect(find.textContaining('PRIVATE_HELPER'), findsNothing);
      expect(t.takeException(), isNull);
    },
  );
  for (final status in <String?>[
    null,
    'unavailable',
    'origin_unavailable',
    'unattributed',
    'ready',
  ]) {
    testWidgets('sin texto del bot se conserva estado seguro: $status', (
      t,
    ) async {
      final backend = PluginsBackend()
        ..jobs = [job(status: status, message: false)];
      await mount(t, backend);
      await select(t);
      expect(find.textContaining('PRIVATE_HELPER'), findsNothing);
      expect(find.textContaining('Construcción interrumpida.'), findsOneWidget);
      expect(find.textContaining('Últimas actualizaciones de'), findsNothing);
      if (status == 'unavailable') {
        expect(
          find.textContaining('Recarga la ficha para reintentar.'),
          findsOneWidget,
        );
      }
      expect(t.takeException(), isNull);
    });
  }
  for (final brightness in Brightness.values) {
    for (final (size, scale) in [
      (const Size(1100, 720), 1.0),
      (const Size(640, 420), 1.0),
      (const Size(640, 420), 1.5),
    ]) {
      testWidgets(
        'mensajes del bot en ficha ${brightness.name} ${size.width} texto $scale',
        (t) async {
          t.platformDispatcher.textScaleFactorTestValue = scale;
          addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
          await loadFonts();
          final value = job();
          value['bot_presentation']['messages'][0]['text'] =
              'Conservé el candidato y las comprobaciones disponibles. Revisa el acceso en el chat de origen antes de continuar.';
          final backend = PluginsBackend()
            ..records = []
            ..jobs = [value]
            ..value = snapshot(title: 'Mi asistente');
          final boundary = GlobalKey();
          final state = await mount(
            t,
            backend,
            size: size,
            brightness: brightness,
            boundary: boundary,
          );
          await t.tap(find.text('example.com').first);
          await t.pumpAndSettle();
          final heading = find.text('Última actualización de Mi asistente');
          await t.ensureVisible(heading);
          await t.pumpAndSettle();
          final ratios = await measureContrast(t, {
            'heading': heading,
          }, captureBoundary: find.byKey(boundary));
          Future<void> capture(String suffix) async {
            if (!const bool.fromEnvironment('CAPTURE_UI')) return;
            await t.runAsync(() async {
              final render =
                  boundary.currentContext!.findRenderObject()!
                      as RenderRepaintBoundary;
              final image = await render.toImage();
              final bytes = await image.toByteData(
                format: ui.ImageByteFormat.png,
              );
              File(
                '../.verification/assistant-chat-real/job-messages-${brightness.name}-${size.width.toInt()}-$scale-$suffix.png',
              ).writeAsBytesSync(bytes!.buffer.asUint8List());
              image.dispose();
            });
          }

          await capture('summary');
          final history = find.text('Últimas actualizaciones de Mi asistente');
          await t.ensureVisible(history);
          await t.pumpAndSettle();
          ratios.addAll(
            await measureContrast(t, {
              'activity': history,
            }, captureBoundary: find.byKey(boundary)),
          );
          await t.tap(history);
          await t.pumpAndSettle();
          await capture('activity');
          expect(find.textContaining('PRIVATE_HELPER'), findsNothing);
          expect(t.takeException(), isNull);
          if (const bool.fromEnvironment('CAPTURE_UI')) {
            File(
              '../.verification/assistant-chat-real/job-messages-${brightness.name}-${size.width.toInt()}-$scale-contrast.json',
            ).writeAsStringSync(jsonEncode(ratios));
          }
          await t.pumpWidget(const SizedBox());
          await t.runAsync(state.close);
          state.dispose();
        },
      );
    }
  }
}
