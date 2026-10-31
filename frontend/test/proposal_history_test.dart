import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/main.dart';
import 'package:gennie/src/assignment_proposals.dart';
import 'package:gennie/src/backend.dart';
import 'assignment_proposals_test.dart' show mount;
import 'proposal_refinement_test.dart' show RefinementBackend;
import 'assignment_execution_test.dart' show press;
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show capture, measureContrast;
import 'widget_test.dart' as fixture;

class HistoryBackend extends RefinementBackend {
  final archivedFamilies = <String>{};
  bool failArchive = false, failMore = false;
  Completer<String>? pendingMore;
  Completer<void>? pendingArchive;
  Map<String, dynamic> row(int n, {bool archived = false}) {
    final r = super.review();
    r['id'] = 'review-$n';
    r['archived'] = archived;
    r['current'] = !archived;
    r['proposals'][0]['id'] = n;
    r['proposals'][0]['candidate']['title'] = 'Documento propuesto $n';
    return r;
  }

  String page(bool more) => jsonEncode({
    'reviews': more
        ? [row(3), row(4)]
        : [
            for (final n in [1, 2])
              if (!archivedFamilies.contains('review-$n')) row(n),
          ],
    'total': 4 - archivedFamilies.length,
    'next_cursor': more ? null : {'page': 2},
  });
  @override
  Future<String> proposalRequest(String text) async {
    final r = Map<String, dynamic>.from(jsonDecode(text) as Map);
    if (r['action'] == 'list') {
      requests.add(r);
      if (r['archived'] == true) {
        return jsonEncode({
          'reviews': [
            for (final n in [1, 2])
              if (archivedFamilies.contains('review-$n'))
                row(n, archived: true),
          ],
          'total': archivedFamilies.length,
        });
      }
      if (r['before'] != null) {
        if (failMore) throw StateError('No se pudo cargar');
        if (pendingMore != null) return pendingMore!.future;
        return page(true);
      }
      return page(false);
    }
    if (r['action'] == 'archive') {
      requests.add(r);
      if (failArchive) throw StateError('No se pudo guardar');
      if (pendingArchive != null) await pendingArchive!.future;
      archivedFamilies.add(r['review_id'] as String);
      return '{}';
    }
    return super.proposalRequest(text);
  }
}

class SelectionHistoryBackend extends HistoryBackend {
  @override
  Future<String> proposalRequest(String text) async {
    final request = jsonDecode(text) as Map;
    if (request['action'] == 'list' && request['assignment_id'] == 2) {
      final r = row(100);
      r['scope']['assignments'][0]['id'] = 2;
      r['proposals'][0]['candidate']['assignment_id'] = 2;
      return jsonEncode({
        'reviews': [r],
        'total': 1,
      });
    }
    return super.proposalRequest(text);
  }
}

void main() {
  setUpAll(loadFonts);
  testWidgets(
    'una actualización del chat no habilita otro archivo durante el guardado',
    (t) async {
      final response = Completer<void>();
      final b = HistoryBackend()..pendingArchive = response;
      await mount(t, backend: b);
      await press(t, 'Archivar revisión');
      await t.tap(find.text('Archivar'));
      await t.pump();
      await t.pump(const Duration(milliseconds: 300));
      final state = t.widget<GennieApp>(find.byType(GennieApp)).state;
      state.accept(fixture.snapshot(plans: b.value.assignments, messages: []));
      await t.pump();
      expect(
        t
            .widget<TextButton>(
              find.widgetWithText(TextButton, 'Archivar revisión').first,
            )
            .onPressed,
        isNull,
      );
      expect(
        t
            .widget<TextButton>(
              find.widgetWithText(TextButton, 'Actualizar propuestas'),
            )
            .onPressed,
        isNull,
      );
      response.complete();
      await t.pumpAndSettle();
      expect(b.requests.where((r) => r['action'] == 'archive'), hasLength(1));
      expect(b.archivedFamilies, contains('review-2'));
      expect(find.text('Documento propuesto 2'), findsNothing);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets('respuesta de una página anterior no contamina otra selección', (
    t,
  ) async {
    final response = Completer<String>();
    final b = SelectionHistoryBackend()..pendingMore = response;
    final state = DesktopState(b);
    await state.start();
    addTearDown(state.dispose);
    final selected = ValueNotifier(b.value.assignments.first);
    addTearDown(selected.dispose);
    await t.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SingleChildScrollView(
            child: ValueListenableBuilder<AssignmentRecord>(
              valueListenable: selected,
              builder: (_, record, _) => AssignmentProposals(
                state: state,
                record: record,
                onOpenChat: () {},
              ),
            ),
          ),
        ),
      ),
    );
    await t.pumpAndSettle();
    await t.ensureVisible(find.text('Cargar revisiones anteriores'));
    await t.pumpAndSettle();
    await t.tap(find.text('Cargar revisiones anteriores'));
    await t.pump();
    selected.value = AssignmentRecord(
      id: '2',
      instanceId: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
      revision: 1,
      createdAt: '',
      updatedAt: '',
      document: b.value.assignments.first.document,
    );
    await t.pumpAndSettle();
    response.complete(b.page(true));
    await t.pumpAndSettle();
    expect(find.text('Documento propuesto 100'), findsOneWidget);
    expect(find.text('1 de 1 revisiones'), findsOneWidget);
    expect(find.text('Documento propuesto 4'), findsNothing);
    expect(find.text('4 de 4 revisiones'), findsNothing);
    expect(t.takeException(), isNull);
  });
  testWidgets(
    'páginas conservan filas y cursor ante fallo y permiten reintentar',
    (t) async {
      final b = HistoryBackend()..failMore = true;
      await mount(t, backend: b);
      await press(t, 'Cargar revisiones anteriores');
      expect(find.text('Documento propuesto 1'), findsOneWidget);
      expect(
        find.textContaining('No se pudieron cargar las propuestas'),
        findsOneWidget,
      );
      b.failMore = false;
      await press(t, 'Cargar revisiones anteriores');
      expect(find.text('Documento propuesto 1'), findsOneWidget);
      expect(find.text('Documento propuesto 4'), findsOneWidget);
      expect(find.text('4 de 4 revisiones'), findsOneWidget);
      expect(find.text('Cargar revisiones anteriores'), findsNothing);
      expect(b.requests.where((r) => r['before'] != null), hasLength(2));
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'cancelar archivo no escribe y confirmar conserva consulta sin resolución',
    (t) async {
      final b = HistoryBackend();
      await mount(t, backend: b);
      await press(t, 'Archivar revisión');
      await press(t, 'Conservar revisión');
      expect(b.requests.where((r) => r['action'] == 'archive'), isEmpty);
      await press(t, 'Archivar revisión');
      await press(t, 'Archivar');
      expect(b.requests.where((r) => r['action'] == 'archive'), hasLength(1));
      await press(t, 'Revisiones archivadas');
      await press(t, 'Documento propuesto 2');
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
            .widget<TextButton>(find.widgetWithText(TextButton, 'Descartar'))
            .onPressed,
        isNull,
      );
      expect(
        find.textContaining('Los encargos cambiaron desde esta revisión'),
        findsNothing,
      );
      expect(b.requests.where((r) => r['action'] == 'resolve'), isEmpty);
      expect(b.sent, isEmpty);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'fallo al archivar conserva borradores y muestra error recuperable',
    (t) async {
      final b = HistoryBackend()..failArchive = true;
      await mount(t, backend: b);
      await press(t, 'Archivar revisión');
      await press(t, 'Archivar');
      expect(
        find.textContaining('No se pudo archivar la revisión'),
        findsOneWidget,
      );
      expect(find.text('Documento propuesto 1'), findsOneWidget);
      b.failArchive = false;
      await press(t, 'Archivar revisión');
      await press(t, 'Archivar');
      expect(b.archivedFamilies, isNotEmpty);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets('consulta pendiente bloquea doble página y cambio de modo', (
    t,
  ) async {
    final response = Completer<String>();
    final b = HistoryBackend()..pendingMore = response;
    await mount(t, backend: b);
    final button = find.text('Cargar revisiones anteriores');
    await Scrollable.ensureVisible(t.element(button));
    await t.pumpAndSettle();
    await t.tap(button);
    await t.pump();
    expect(
      t
          .widget<TextButton>(
            find.widgetWithText(TextButton, 'Cargar revisiones anteriores'),
          )
          .onPressed,
      isNull,
    );
    expect(
      t
          .widget<TextButton>(
            find.widgetWithText(TextButton, 'Revisiones archivadas'),
          )
          .onPressed,
      isNull,
    );
    response.complete(b.page(true));
    await t.pumpAndSettle();
    expect(find.text('Documento propuesto 4'), findsOneWidget);
    expect(b.requests.where((r) => r['before'] != null), hasLength(1));
    expect(t.takeException(), isNull);
  });
  testWidgets(
    'actualizaciones de conversación conservan las páginas ya leídas',
    (t) async {
      final b = HistoryBackend();
      await mount(t, backend: b);
      await press(t, 'Cargar revisiones anteriores');
      final state = t.widget<GennieApp>(find.byType(GennieApp)).state;
      state.accept(fixture.snapshot(plans: b.value.assignments, messages: []));
      await t.pumpAndSettle();
      expect(find.text('Documento propuesto 4'), findsOneWidget);
      expect(
        find.textContaining('Puede haber propuestas nuevas'),
        findsOneWidget,
      );
      await press(t, 'Actualizar propuestas');
      expect(find.text('Documento propuesto 4'), findsNothing);
      expect(t.takeException(), isNull);
    },
  );
  for (final brightness in Brightness.values) {
    for (final (size, scale) in [
      (const Size(1100, 720), 1.0),
      (const Size(640, 420), 1.0),
      (const Size(640, 420), 1.5),
    ]) {
      testWidgets('archivo ${brightness.name} ${size.width} texto $scale', (
        t,
      ) async {
        t.platformDispatcher.platformBrightnessTestValue = brightness;
        t.platformDispatcher.textScaleFactorTestValue = scale;
        addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
        addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
        final b = HistoryBackend();
        await mount(t, size: size, backend: b);
        await press(t, 'Archivar revisión');
        final name =
            'proposal-history-${brightness.name}-${size.width.toInt()}-$scale';
        await capture(t, '$name-confirm');
        final ratios = await measureContrast(t, {
          'conservar': find.text('Conservar revisión'),
          'archivar': find.text('Archivar'),
        });
        await press(t, 'Archivar');
        await press(t, 'Revisiones archivadas');
        await press(t, 'Documento propuesto 2');
        await capture(t, '$name-detail');
        ratios.addAll(
          await measureContrast(t, {'cerrar': find.text('Cerrar')}),
        );
        expect(t.takeException(), isNull);
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          File(
            '../.verification/assistant-chat-real/$name-contrast.json',
          ).writeAsStringSync(jsonEncode(ratios));
        }
      });
    }
  }
}
