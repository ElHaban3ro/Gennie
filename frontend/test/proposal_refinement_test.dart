import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'assignment_proposals_test.dart' show ProposalBackend, mount;
import 'assignment_execution_test.dart' show press;
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show capture, measureContrast;

class RefinementBackend extends ProposalBackend {
  Completer<String>? pendingGet;
  bool superseded = false,
      verified = true,
      originCurrent = true,
      failGet = false;
  @override
  Map<String, dynamic> review() {
    final r = super.review();
    r['current'] = !superseded;
    r['origin_current'] = originCurrent;
    r['bot_available'] = botAvailable;
    r['version'] = {
      'round': 1,
      'next_review_id': superseded ? 'next-review' : null,
      'input_verified': verified,
    };
    return r;
  }

  Map<String, dynamic> newer() {
    final r = super.review();
    r['id'] = 'next-review';
    r['version'] = {'round': 2, 'next_review_id': null, 'input_verified': true};
    r['proposals'][0]['id'] = 88;
    r['proposals'][0]['candidate']['questions'] = [];
    r['proposals'][0]['candidate']['title'] = 'Resumen acordado';
    return r;
  }

  @override
  Future<String> proposalRequest(String text) async {
    final r = Map<String, dynamic>.from(jsonDecode(text) as Map);
    if (r['action'] == 'get') {
      requests.add(r);
      if (failGet) throw StateError('No se pudo leer');
      if (pendingGet != null) return pendingGet!.future;
      return jsonEncode(newer());
    }
    return super.proposalRequest(text);
  }
}

void main() {
  setUpAll(loadFonts);
  testWidgets('responder abre el chat sin enviar texto ni aplicar cambios', (
    t,
  ) async {
    final b = RefinementBackend()
      ..showProposal = true
      ..question = true;
    await mount(t, backend: b);
    await press(t, 'Agrupar las novedades');
    await press(t, 'Responder en el chat');
    expect(
      find.text('Por aclarar con Gennie: ¿Qué carpeta quieres revisar?'),
      findsNothing,
    );
    expect(b.sent, isEmpty);
    expect(b.requests.where((r) => r['action'] == 'resolve'), isEmpty);
    expect(t.takeException(), isNull);
  });
  testWidgets(
    'versión anterior impide resolver y permite consultar la posterior',
    (t) async {
      final b = RefinementBackend()
        ..showProposal = true
        ..superseded = true;
      await mount(t, backend: b);
      await press(t, 'Agrupar las novedades');
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
      await press(t, 'Ver versión posterior');
      expect(find.text('Resumen acordado'), findsOneWidget);
      await press(t, 'Aplicar cambios');
      expect(
        b.requests.singleWhere((r) => r['action'] == 'resolve')['proposal_id'],
        88,
      );
      expect(t.takeException(), isNull);
    },
  );
  for (final legacy in [false, true]) {
    testWidgets(
      'contexto ${legacy ? 'sin origen verificable' : 'de otra conversación'} bloquea responder',
      (t) async {
        final b = RefinementBackend()
          ..showProposal = true
          ..question = true
          ..verified = !legacy
          ..originCurrent = legacy;
        await mount(t, backend: b);
        await press(t, 'Agrupar las novedades');
        expect(
          t
              .widget<TextButton>(
                find.widgetWithText(TextButton, 'Responder en el chat'),
              )
              .onPressed,
          isNull,
        );
        expect(b.sent, isEmpty);
        expect(t.takeException(), isNull);
      },
    );
  }
  testWidgets(
    'consulta pendiente impide abrir otros borradores hasta recibir la versión',
    (t) async {
      final response = Completer<String>();
      final b = RefinementBackend()
        ..showProposal = true
        ..superseded = true
        ..pendingGet = response;
      await mount(t, backend: b);
      await press(t, 'Agrupar las novedades');
      await t.tap(find.text('Ver versión posterior'));
      await t.pump();
      await t.pump(const Duration(milliseconds: 300));
      expect(find.byType(Dialog), findsNothing);
      expect(
        t
            .widget<TextButton>(
              find.widgetWithText(TextButton, 'Proponer mejoras'),
            )
            .onPressed,
        isNull,
      );
      expect(
        t
            .widget<ListTile>(
              find.ancestor(
                of: find.text('Agrupar las novedades'),
                matching: find.byType(ListTile),
              ),
            )
            .onTap,
        isNull,
      );
      response.complete(jsonEncode(b.newer()));
      await t.pumpAndSettle();
      expect(find.text('Resumen acordado'), findsOneWidget);
      expect(find.byType(Dialog), findsOneWidget);
      expect(b.requests.where((r) => r['action'] == 'get'), hasLength(1));
      expect(b.requests.where((r) => r['action'] == 'resolve'), isEmpty);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'fallo al consultar la nueva versión queda visible sin aplicar el anterior',
    (t) async {
      final b = RefinementBackend()
        ..showProposal = true
        ..superseded = true
        ..failGet = true;
      await mount(t, backend: b);
      await press(t, 'Agrupar las novedades');
      await press(t, 'Ver versión posterior');
      expect(
        find.textContaining('No se pudo consultar la versión nueva'),
        findsOneWidget,
      );
      expect(b.requests.where((r) => r['action'] == 'resolve'), isEmpty);
    },
  );
  for (final brightness in Brightness.values) {
    for (final (size, scale) in [
      (const Size(1100, 720), 1.0),
      (const Size(640, 420), 1.0),
      (const Size(640, 420), 1.5),
    ]) {
      testWidgets('versiones ${brightness.name} ${size.width} texto $scale', (
        t,
      ) async {
        t.platformDispatcher.platformBrightnessTestValue = brightness;
        t.platformDispatcher.textScaleFactorTestValue = scale;
        addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
        addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
        final b = RefinementBackend()
          ..showProposal = true
          ..question = true;
        await mount(t, size: size, backend: b);
        await press(t, 'Agrupar las novedades');
        final name =
            'refinement-${brightness.name}-${size.width.toInt()}-$scale';
        await Scrollable.ensureVisible(
          t.element(find.text('Responder en el chat')),
          alignment: .5,
        );
        await t.pumpAndSettle();
        await capture(t, '$name-question');
        final ratios = await measureContrast(t, {
          'responder': find.text('Responder en el chat'),
          'cerrar': find.text('Cerrar'),
        });
        await press(t, 'Cerrar');
        b.question = false;
        b.superseded = true;
        await press(t, 'Actualizar propuestas');
        await press(t, 'Agrupar las novedades');
        await Scrollable.ensureVisible(
          t.element(find.text('Ver versión posterior')),
          alignment: .5,
        );
        await t.pumpAndSettle();
        await capture(t, '$name-previous');
        ratios.addAll(
          await measureContrast(t, {
            'posterior': find.text('Ver versión posterior'),
          }),
        );
        await press(t, 'Ver versión posterior');
        expect(find.text('Aplicar cambios').hitTestable(), findsOneWidget);
        await capture(t, '$name-current');
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
