import 'dart:convert';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/backend.dart';
import 'widget_test.dart' as fixture;
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show capture, measureContrast;

const reference = ExecutionCall(
  state: 'uncertain',
  assignmentId: '4',
  instanceId: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
  policyId: 'policy-fixture',
  policyRevision: 2,
  runId: '9',
);

class NoticeBackend extends fixture.FakeBackend implements ExecutionBackend {
  NoticeBackend() {
    value = fixture.snapshot(
      messages: const [
        Message(
          speaker: 'user',
          text: 'Revisa únicamente novedades',
          createdAt: '2026-10-29T12:00:00Z',
        ),
        Message(
          speaker: 'assistant',
          text:
              'La función no confirmó el resultado. Revisa el efecto antes de continuar.',
          createdAt: '2026-10-29T13:00:00Z',
        ),
      ],
    );
  }
  final requests = <Map<String, dynamic>>[];
  bool wrongReference = false, failRead = false, failSave = false;
  @override
  Future<ChatPage> loadChatPage(ChatCursor? before) async {
    final page = await super.loadChatPage(before);
    return ChatPage(
      before: page.before,
      messages: [
        for (final e in page.messages)
          ChatEntry(
            id: e.id,
            conversationId: 'origin',
            sessionStartedAt: e.sessionStartedAt,
            startsSession: e.startsSession,
            message: e.message,
            tools: e.tools,
            execution: e.message.speaker == 'assistant' ? reference : null,
          ),
      ],
    );
  }

  @override
  Future<String> executionRequest(String text) async {
    final r = Map<String, dynamic>.from(jsonDecode(text) as Map);
    requests.add(r);
    if (r['action'] == 'reconcile') {
      if (failSave) throw StateError('No se pudo confirmar el recibo');
      return '{}';
    }
    if (failRead) throw StateError('El intento no está disponible');
    return jsonEncode({
      'id': 9,
      'policy_id': 'policy-fixture',
      'policy_revision': 2,
      'state': 'uncertain',
      'scheduled_at': '2026-10-29T13:00:00Z',
      'configuration': {
        'assignment': {
          'id': 4,
          'instance_id': wrongReference
              ? 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'
              : 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
          'document': {
            'authorization': {'scope': 'Solo los mensajes de prueba'},
          },
        },
        'target': {
          'kind': 'mcp',
          'server_id': 'fixture',
          'tool': 'read',
          'revision': 1,
        },
        'arguments': {'query': 'original'},
      },
      'output': {'mcp_attempt_id': 11},
    });
  }
}

Future<NoticeBackend> mount(
  WidgetTester t, {
  NoticeBackend? backend,
  Size size = const Size(1100, 720),
}) async {
  final b = backend ?? NoticeBackend();
  await fixture.mount(t, backend: b, size: size);
  return b;
}

Future<void> press(WidgetTester t, String text) async {
  final f = find.text(text).last;
  await Scrollable.ensureVisible(t.element(f), alignment: .5);
  await t.pumpAndSettle();
  await t.tap(f);
  await t.pumpAndSettle();
}

void main() {
  setUpAll(loadFonts);
  testWidgets(
    'hecho tipado bajo mensaje del bot abre intento exacto; comprobación explícita',
    (t) async {
      final b = await mount(t);
      expect(find.text('Resultado sin confirmar'), findsOneWidget);
      expect(
        find.text('Actualización de una conversación anterior'),
        findsOneWidget,
      );
      await press(t, 'Ver ejecución del encargo');
      expect(b.requests.single, {'action': 'run_details', 'run_id': 9});
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Guardar comprobación'),
            )
            .onPressed,
        isNull,
      );
      await press(t, 'Comprobé que no se aplicó');
      await press(t, 'Guardar comprobación');
      expect(b.requests.last, {
        'action': 'reconcile',
        'run_id': 9,
        'resolution': 'not_applied',
      });
      expect(b.sent, isEmpty);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'identidad reutilizada se rechaza y no ofrece comprobación de otro encargo',
    (t) async {
      final b = await mount(t, backend: NoticeBackend()..wrongReference = true);
      await press(t, 'Ver ejecución del encargo');
      expect(find.textContaining('no coincide').hitTestable(), findsOneWidget);
      expect(find.text('Guardar comprobación'), findsNothing);
      expect(b.requests.length, 1);
      b.wrongReference = false;
      await press(t, 'Reintentar detalle');
      expect(find.text('Revisar resultado'), findsOneWidget);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'error de consulta recuperable y comprobación fallida conserva elección visible',
    (t) async {
      final b = await mount(
        t,
        backend: NoticeBackend()
          ..failRead = true
          ..failSave = true,
      );
      await press(t, 'Ver ejecución del encargo');
      expect(
        find.textContaining('no está disponible').hitTestable(),
        findsOneWidget,
      );
      b.failRead = false;
      await press(t, 'Reintentar detalle');
      await press(t, 'La acción se aplicó');
      await press(t, 'Guardar comprobación');
      expect(
        find.textContaining('No se pudo confirmar el recibo').hitTestable(),
        findsOneWidget,
      );
      expect(
        t
            .widget<RadioGroup<String>>(find.byType(RadioGroup<String>))
            .groupValue,
        'applied',
      );
      expect(b.requests.where((r) => r['action'] == 'reconcile').length, 1);
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
        'aviso de ejecución ${brightness.name} ${size.width} texto $scale',
        (t) async {
          t.platformDispatcher.platformBrightnessTestValue = brightness;
          t.platformDispatcher.textScaleFactorTestValue = scale;
          addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
          addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
          await mount(t, size: size);
          final link = find.text('Ver ejecución del encargo');
          await Scrollable.ensureVisible(t.element(link), alignment: .5);
          await t.pumpAndSettle();
          final name =
              'execution-notice-${brightness.name}-${size.width.toInt()}-$scale';
          final ratios = await measureContrast(t, {
            'estado': find.text('Resultado sin confirmar'),
            'enlace': link,
          });
          await capture(t, '$name-chat');
          await press(t, 'Ver ejecución del encargo');
          await press(t, 'Comprobé que no se aplicó');
          expect(
            find.text('Guardar comprobación').hitTestable(),
            findsOneWidget,
          );
          ratios.addAll(
            await measureContrast(t, {
              'guardar': find.text('Guardar comprobación'),
            }),
          );
          await capture(t, '$name-review');
          if (const bool.fromEnvironment('CAPTURE_UI')) {
            File(
              '../.verification/assistant-chat-real/$name-contrast.json',
            ).writeAsStringSync(jsonEncode(ratios));
          }
          expect(t.takeException(), isNull);
        },
      );
    }
  }
}
