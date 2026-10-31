import 'dart:io';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/backend.dart';
import 'widget_test.dart' as fixture;
import 'plugins_test.dart' show PluginsBackend;
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show capture, measureContrast;

class BotNoticeBackend extends PluginsBackend {
  String sequence = '103';
  String message =
      'Para seguir con la integración, abre Gennie Web e inicia sesión en la cuenta que quieres utilizar.';
  BotNoticeBackend() {
    jobs = [
      {
        'id': 7,
        'plugin_id': 'gmail_test',
        'origin': 'https://example.com',
        'objective': 'Preparar consultas de prueba',
        'trigger_conversation_id': 1,
        'state': 'waiting_user',
        'summary': 'PRIVATE_HELPER_TEXT que no coincide con el mensaje del bot',
        'events': [
          {'id': 103, 'message': 'PRIVATE_HELPER_TEXT'},
        ],
      },
    ];
  }
  bool hideCatalogueJob = false;
  @override
  Future<String> pluginRequest(String text) async {
    final request = Map<String, dynamic>.from(jsonDecode(text) as Map);
    if (request['action'] == 'get_job') {
      requests.add(request);
      return jsonEncode({
        'job': jobs.firstWhere((j) => j['id'] == request['job_id']),
        'has_live_successor': false,
      });
    }
    if (request['action'] == 'list' && hideCatalogueJob) {
      requests.add(request);
      return jsonEncode({'plugins': records, 'jobs': []});
    }
    return super.pluginRequest(text);
  }

  @override
  Future<ChatPage> loadChatPage(ChatCursor? before) async => ChatPage(
    messages: [
      ChatEntry(
        id: '321',
        conversationId: '1',
        sessionStartedAt: '2026-10-29T10:00:00Z',
        startsSession: true,
        tools: const [],
        delegation: DelegationCall(
          jobId: '7',
          sourceSequence: sequence,
          pluginId: 'gmail_test',
        ),
        message: Message(
          speaker: 'assistant',
          text: message,
          createdAt: '2026-10-29T10:05:00Z',
        ),
      ),
    ],
  );
}

Future<BotNoticeBackend> mount(
  WidgetTester t, {
  BotNoticeBackend? backend,
  Size size = const Size(1100, 720),
}) async {
  final b = backend ?? BotNoticeBackend();
  await fixture.mount(t, backend: b, size: size);
  await t.pumpAndSettle();
  return b;
}

void main() {
  testWidgets(
    'el aviso del bot usa origen/evento persistidos y conserva abrir/continuar',
    (t) async {
      final b = await mount(t);
      expect(find.text('Abrir Gennie Web'), findsOneWidget);
      expect(find.textContaining('PRIVATE_HELPER_TEXT'), findsNothing);
      await t.tap(find.text('Abrir Gennie Web'));
      await t.pumpAndSettle();
      expect(
        b.requests.where((r) => r['action'] == 'connect').single['job_id'],
        7,
      );
      expect(
        find.text('Actualización de una conversación anterior'),
        findsOneWidget,
      );
      expect(find.text('Continuar'), findsOneWidget);
      await t.tap(find.text('Continuar'));
      await t.pumpAndSettle();
      expect(
        b.requests.where((r) => r['action'] == 'resume').single['job_id'],
        7,
      );
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'una actualización anterior no habilita acciones por similitud de texto',
    (t) async {
      await mount(t, backend: BotNoticeBackend()..sequence = '102');
      expect(find.text('Abrir Gennie Web'), findsNothing);
      expect(find.text('Continuar'), findsNothing);
    },
  );
  testWidgets('otro origen o plugin no recibe acciones del trabajo', (t) async {
    final b = BotNoticeBackend();
    b.jobs.single['trigger_conversation_id'] = 2;
    await mount(t, backend: b);
    expect(find.text('Abrir Gennie Web'), findsNothing);
  });
  testWidgets(
    'el resultado del bot enlaza el plugin por identidad comprobada',
    (t) async {
      final b = BotNoticeBackend();
      b.jobs.single['state'] = 'completed';
      b.message =
          'La integración quedó preparada y sus funciones se verificaron.';
      await mount(t, backend: b);
      expect(find.text('Ver plugin'), findsOneWidget);
      await t.tap(find.text('Ver plugin'));
      await t.pumpAndSettle();
      expect(find.text('Correo de prueba'), findsWidgets);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'una referencia antigua se consulta directamente aunque el catálogo omita el trabajo',
    (t) async {
      final b = await mount(
        t,
        backend: BotNoticeBackend()..hideCatalogueJob = true,
      );
      expect(find.text('Abrir Gennie Web'), findsOneWidget);
      expect(
        b.requests.where((r) => r['action'] == 'get_job').isNotEmpty,
        isTrue,
      );
    },
  );
  for (final brightness in Brightness.values) {
    for (final (size, scale) in [
      (const Size(1100, 720), 1.0),
      (const Size(640, 420), 1.0),
      (const Size(640, 420), 1.5),
    ]) {
      testWidgets('aviso bot ${brightness.name} ${size.width} texto $scale', (
        t,
      ) async {
        t.platformDispatcher.platformBrightnessTestValue = brightness;
        t.platformDispatcher.textScaleFactorTestValue = scale;
        addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
        addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
        await loadFonts();
        await mount(t, size: size);
        expect(find.text('Abrir Gennie Web').hitTestable(), findsOneWidget);
        final name =
            'bot-notice-${brightness.name}-${size.width.toInt()}-$scale';
        final ratios = await measureContrast(t, {
          'abrir': find.text('Abrir Gennie Web'),
          'origen': find.text('Actualización de una conversación anterior'),
        });
        await capture(t, name);
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          File(
            '../.verification/assistant-chat-real/$name-contrast.json',
          ).writeAsStringSync(jsonEncode(ratios));
        }
        expect(t.takeException(), isNull);
      });
    }
  }
}
