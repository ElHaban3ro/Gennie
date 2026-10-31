import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'plugins_test.dart' show PluginsBackend, mount, select;
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show measureContrast;

Map<String, dynamic> attempt(int id, {bool active = false}) => {
  'id': id,
  'plugin_id': 'gmail_test',
  'server_name': 'Correo de prueba',
  'tool': 'list_messages',
  'source_revision': 1,
  'candidate_id': 'job_9',
  'operation': 'verify',
  'state': active ? 'running' : 'uncertain',
  'reason': 'interrupted',
  'created_at': '2026-10-29T15:00:00Z',
  'active': active,
};

class WebBackend extends PluginsBackend {
  WebBackend({bool active = false, int count = 1}) {
    pendingRows = [
      for (var id = 1; id <= count; id++) attempt(id, active: active),
    ];
    sync();
  }
  List<Map<String, dynamic>> pendingRows = [];
  final reviewed = <Map<String, dynamic>>[];
  bool failReview = false, uncertain = false, failJournal = false;
  int effects = 0;
  Map<String, dynamic> page({int after = 0}) {
    final rows = pendingRows.where((a) => a['id'] > after).toList();
    return {
      'status': 'ready',
      'total': pendingRows.length,
      'attempts': rows.take(2).toList(),
      'next_cursor': rows.length > 2 ? rows[1]['id'] : null,
    };
  }

  void sync() {
    records.first['web_pending'] = page();
    records.first['web_recent_reviews'] = reviewed.toList();
  }

  @override
  Future<String> pluginRequest(String text) async {
    final r = Map<String, dynamic>.from(jsonDecode(text) as Map);
    if (r['action'] == 'web_attempts') {
      if (failJournal) throw StateError('No se pudo leer el diario');
      requests.add(r);
      return jsonEncode(page(after: r['after'] as int? ?? 0));
    }
    if (r['action'] == 'web_reconcile') {
      requests.add(r);
      if (failReview) throw StateError('No se guardó la comprobación.');
      final original = pendingRows.singleWhere(
        (a) => a['id'] == r['attempt_id'],
      );
      expect(r['expected_revision'], original['source_revision']);
      reviewed.add({
        ...original,
        'state': 'reviewed',
        'resolution': r['resolution'],
      });
      pendingRows.remove(original);
      sync();
      return jsonEncode({'status': 'reviewed', 'executed': false});
    }
    if (r['action'] == 'run' || r['action'] == 'verify') {
      requests.add(r);
      effects++;
      if (uncertain) {
        pendingRows = [attempt(9)];
        sync();
        return jsonEncode({
          'status': 'result_uncertain',
          'executed': true,
          'review_required': true,
          'attempt': attempt(9),
        });
      }
      return jsonEncode({'status': 'ok', 'executed': true});
    }
    return super.pluginRequest(text);
  }
}

Future<void> click(WidgetTester t, String text) async {
  final f = find.text(text).last;
  await t.ensureVisible(f);
  await t.tap(f);
  await t.pumpAndSettle();
}

Finder dialogText(String text) =>
    find.descendant(of: find.byType(Dialog), matching: find.text(text));
void main() {
  for (final journalDown in [false, true]) {
    testWidgets(
      'catálogo ocupado conserva bloqueo con diario ${journalDown ? 'no disponible' : 'independiente'}',
      (t) async {
        final b = WebBackend(count: 0);
        await mount(t, b);
        await select(t);
        b.pendingRows = [attempt(9)];
        b.sync();
        b.catalogueError = 'El navegador está ocupado';
        b.failJournal = journalDown;
        await t.tap(find.byIcon(Icons.refresh).first);
        await t.pumpAndSettle();
        expect(
          t
              .widget<FilledButton>(
                find.widgetWithText(FilledButton, 'Ejecutar función'),
              )
              .onPressed,
          isNull,
        );
        expect(find.text('Resultados web pendientes'), findsOneWidget);
        expect(b.effects, 0);
        expect(t.takeException(), isNull);
      },
    );
  }
  testWidgets(
    'otra función independiente permanece disponible sin reenviar el intento',
    (t) async {
      final b = WebBackend();
      b.pendingRows.single['tool'] = 'another_function';
      b.sync();
      await mount(t, b);
      await select(t);
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Ejecutar función'),
            )
            .onPressed,
        isNotNull,
      );
      expect(find.text('Resultados web pendientes'), findsOneWidget);
      expect(b.effects, 0);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'el pendiente bloquea la ficha y cancelar no comprueba ni ejecuta',
    (t) async {
      final b = WebBackend();
      await mount(t, b);
      await select(t);
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Ejecutar función'),
            )
            .onPressed,
        isNull,
      );
      await click(t, 'Revisar resultado');
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Guardar revisión'),
            )
            .onPressed,
        isNull,
      );
      await click(t, 'Cancelar');
      expect(b.requests.where((r) => r['action'] == 'web_reconcile'), isEmpty);
      expect(b.effects, 0);
      expect(b.pendingRows.length, 1);
      expect(t.takeException(), isNull);
    },
  );
  for (final resolution in ['applied', 'not_applied']) {
    testWidgets('comprobación $resolution sin reenvío y con origen exacto', (
      t,
    ) async {
      final b = WebBackend();
      await mount(t, b);
      await select(t);
      await click(t, 'Revisar resultado');
      await click(
        t,
        resolution == 'applied'
            ? 'La acción se aplicó'
            : 'Comprobé que no se aplicó',
      );
      await click(t, 'Guardar revisión');
      expect(find.byType(Dialog), findsNothing);
      expect(b.reviewed.single['resolution'], resolution);
      expect(b.effects, 0);
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Ejecutar función'),
            )
            .onPressed,
        isNotNull,
      );
      expect(t.takeException(), isNull);
    });
  }
  testWidgets('fallo conserva elección y el intento permanece bloqueado', (
    t,
  ) async {
    final b = WebBackend()..failReview = true;
    await mount(t, b);
    await select(t);
    await click(t, 'Revisar resultado');
    await click(t, 'Comprobé que no se aplicó');
    await click(t, 'Guardar revisión');
    expect(
      find.textContaining('No se guardó la comprobación.'),
      findsOneWidget,
    );
    expect(b.pendingRows.length, 1);
    expect(b.effects, 0);
    b.failReview = false;
    await click(t, 'Guardar revisión');
    expect(b.reviewed.single['resolution'], 'not_applied');
    expect(b.effects, 0);
    expect(t.takeException(), isNull);
  });
  testWidgets('operación activa no admite revisión ni otro envío', (t) async {
    final b = WebBackend(active: true);
    await mount(t, b);
    await select(t);
    expect(
      t
          .widget<TextButton>(
            find.widgetWithText(TextButton, 'Revisar resultado'),
          )
          .onPressed,
      isNull,
    );
    expect(
      t
          .widget<FilledButton>(
            find.widgetWithText(FilledButton, 'Ejecutar función'),
          )
          .onPressed,
      isNull,
    );
    expect(b.effects, 0);
  });
  testWidgets('páginas conservan intentos anteriores y cargan sin ejecutar', (
    t,
  ) async {
    final b = WebBackend(count: 5);
    await mount(t, b);
    await select(t);
    await click(t, 'Cargar más intentos');
    await click(t, 'Cargar más intentos');
    expect(find.text('Revisar resultado'), findsNWidgets(5));
    expect(find.text('Cargar más intentos'), findsNothing);
    expect(b.effects, 0);
    expect(
      b.requests
          .where((r) => r['action'] == 'web_attempts')
          .map((r) => r['after']),
      [2, 4],
    );
    expect(t.takeException(), isNull);
  });
  testWidgets(
    'una respuesta incierta bloquea nuevos envíos antes de recargar',
    (t) async {
      final b = WebBackend(count: 0)..uncertain = true;
      await mount(t, b);
      await select(t);
      await t.enterText(find.byKey(const ValueKey('plugin-param-limit')), '1');
      await click(t, 'Ejecutar función');
      expect(b.effects, 1);
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Ejecutar función'),
            )
            .onPressed,
        isNull,
      );
      expect(find.text('Resultados web pendientes'), findsOneWidget);
      await click(t, 'Revisar resultado');
      await click(t, 'La acción se aplicó');
      await click(t, 'Guardar revisión');
      expect(b.effects, 1);
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Ejecutar función'),
            )
            .onPressed,
        isNotNull,
      );
      expect(t.takeException(), isNull);
    },
  );
  for (final brightness in Brightness.values) {
    for (final spec in [
      (const Size(1100, 720), 1.0),
      (const Size(640, 420), 1.0),
      (const Size(640, 420), 1.5),
    ]) {
      testWidgets(
        'revisión web ${brightness.name} ${spec.$1.width} escala ${spec.$2}',
        (t) async {
          await t.runAsync(loadFonts);
          t.platformDispatcher.textScaleFactorTestValue = spec.$2;
          addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
          final b = WebBackend();
          final key = GlobalKey();
          final state = await mount(
            t,
            b,
            size: spec.$1,
            brightness: brightness,
            boundary: key,
          );
          try {
            await select(t);
            await t.ensureVisible(find.text('Resultados web pendientes'));
            await t.pumpAndSettle();
            final contrast = await measureContrast(t, {
              'título': find.text('Resultados web pendientes'),
            }, captureBoundary: find.byKey(key));
            for (final n in contrast.values) {
              expect(n, greaterThanOrEqualTo(4.5));
            }
            await click(t, 'Revisar resultado');
            await click(t, 'Comprobé que no se aplicó');
            expect(
              dialogText('Guardar revisión').hitTestable(),
              findsOneWidget,
            );
            expect(dialogText('Cancelar').hitTestable(), findsOneWidget);
            await t.runAsync(() async {
              final render =
                  key.currentContext!.findRenderObject()!
                      as RenderRepaintBoundary;
              final image = await render.toImage();
              final bytes = await image.toByteData(
                format: ui.ImageByteFormat.png,
              );
              final base =
                  '../.verification/assistant-chat-real/web-attempt-${brightness.name}-${spec.$1.width.toInt()}-${spec.$2}';
              File('$base.png').writeAsBytesSync(bytes!.buffer.asUint8List());
              File(
                '$base-contrast.json',
              ).writeAsStringSync(jsonEncode(contrast));
              image.dispose();
            });
            expect(t.takeException(), isNull);
            expect(b.effects, 0);
          } finally {
            await t.pumpWidget(const SizedBox());
            await t.runAsync(state.close);
            state.dispose();
          }
        },
      );
    }
  }
}
