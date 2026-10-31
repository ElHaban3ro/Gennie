import 'dart:convert';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/main.dart';
import 'assignment_proposals_test.dart' show mount;
import 'proposal_refinement_test.dart' show RefinementBackend;
import 'assignment_execution_test.dart' show press;
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show capture, measureContrast;
import 'widget_test.dart' as fixture;

class ChatRequestBackend extends RefinementBackend {
  bool longRequest = false, continued = false;
  @override
  Map<String, dynamic> review() {
    final r = super.review();
    r['chat_request'] = {
      'text': longRequest
          ? 'Revisa los encargos. ${'Conserva el alcance acordado y agrupa novedades. ' * 70}'
          : 'Genera propuestas para este encargo y evita avisos repetidos.',
      'truncated': longRequest,
      'message_id': 100,
    };
    if (continued) {
      r['version'] = {
        'round': 2,
        'input_verified': true,
        'response': {
          'text': 'A las nueve, conservando los destinatarios.',
          'truncated': false,
        },
      };
    }
    return r;
  }
}

void main() {
  setUpAll(loadFonts);
  testWidgets(
    'solicitud literal y respuesta se consultan sin enviar ni aplicar',
    (t) async {
      final b = ChatRequestBackend()
        ..showProposal = true
        ..continued = true;
      await mount(t, backend: b);
      await press(t, 'Agrupar las novedades');
      expect(find.text('Solicitud del chat'), findsOneWidget);
      expect(
        find.text(
          'Genera propuestas para este encargo y evita avisos repetidos.',
        ),
        findsOneWidget,
      );
      expect(find.text('Respuesta para esta versión'), findsOneWidget);
      expect(
        find.text('A las nueve, conservando los destinatarios.'),
        findsOneWidget,
      );
      await press(t, 'Cerrar');
      expect(b.sent, isEmpty);
      expect(b.requests.where((r) => r['action'] == 'resolve'), isEmpty);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'revisión nueva se actualiza por metadatos sin mensaje añadido al historial',
    (t) async {
      final b = ChatRequestBackend();
      await mount(t, backend: b);
      final state = t.widget<GennieApp>(find.byType(GennieApp)).state;
      state.accept(b.value);
      await t.pumpAndSettle();
      final history = state.snapshot!.historyRevision;
      b.showProposal = true;
      state.accept(
        fixture.snapshot(plans: b.value.assignments, executionRevision: '1'),
      );
      await t.pumpAndSettle();
      expect(state.snapshot!.historyRevision, history);
      expect(find.text('Solicitada desde el chat'), findsOneWidget);
      expect(find.text('Agrupar las novedades'), findsOneWidget);
      expect(b.sent, isEmpty);
      expect(t.takeException(), isNull);
    },
  );
  for (final brightness in Brightness.values) {
    for (final (size, scale) in [
      (const Size(1100, 720), 1.0),
      (const Size(640, 420), 1.0),
      (const Size(640, 420), 1.5),
    ]) {
      testWidgets(
        'solicitud del chat ${brightness.name} ${size.width} texto $scale',
        (t) async {
          t.platformDispatcher.platformBrightnessTestValue = brightness;
          t.platformDispatcher.textScaleFactorTestValue = scale;
          addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
          addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
          final b = ChatRequestBackend()
            ..showProposal = true
            ..longRequest = true;
          await mount(t, size: size, backend: b);
          await press(t, 'Agrupar las novedades');
          final name =
              'chat-review-${brightness.name}-${size.width.toInt()}-$scale';
          await capture(t, '$name-request');
          final ratios = await measureContrast(t, {
            'cerrar': find.text('Cerrar'),
            'aplicar': find.text('Aplicar cambios'),
            'contexto': find.text('Solicitud del chat'),
          });
          expect(find.text('Cambios propuestos'), findsOneWidget);
          await press(t, 'Solicitud del chat');
          await Scrollable.ensureVisible(
            t.element(
              find.text('Se muestra un extracto de la solicitud original.'),
            ),
            alignment: .5,
          );
          await t.pumpAndSettle();
          await capture(t, '$name-excerpt');
          expect(find.text('Aplicar cambios').hitTestable(), findsOneWidget);
          expect(find.text('Cerrar').hitTestable(), findsOneWidget);
          expect(t.takeException(), isNull);
          if (const bool.fromEnvironment('CAPTURE_UI')) {
            File(
              '../.verification/assistant-chat-real/$name-contrast.json',
            ).writeAsStringSync(jsonEncode(ratios));
          }
        },
      );
    }
  }
}
