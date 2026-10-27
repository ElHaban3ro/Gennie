import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/chat_message.dart';
import 'widget_test.dart' as fixtures;

class PluginFake extends fixtures.FakeBackend implements PluginBackend {
  final requests = <Map<String, dynamic>>[];
  String status = 'waiting_user';
  bool fail = false;
  bool failConnect = false;
  bool hasNewJob = false;
  String summary = 'Inicia sesión y pulsa Continuar.';
  @override
  Future<String> pluginRequest(String request) async {
    final value = Map<String, dynamic>.from(jsonDecode(request) as Map);
    requests.add(value);
    if (value['action'] == 'connect' && failConnect) {
      throw StateError('fallo al abrir');
    }
    if (value['action'] == 'build') hasNewJob = true;
    if (value['action'] == 'resume') {
      if (fail) throw StateError('fallo');
      status = 'queued';
    }
    return jsonEncode({
      'jobs': [
        {
          'id': 1,
          'plugin_id': 'mail',
          'objective': 'Leer correos',
          'max_attempts': 4,
          'max_seconds': 600,
          'origin': 'https://mail.example',
          'trigger_conversation_id': 7,
          'state': status,
          'summary': status == 'cancelled'
              ? 'Construcción cancelada. La versión publicada se conserva.'
              : summary,
        },
        if (hasNewJob)
          {
            'id': 2,
            'plugin_id': 'mail',
            'state': 'queued',
            'trigger_conversation_id': 7,
          },
      ],
    });
  }
}

void main() {
  testWidgets('aviso vigente antiguo dirige al botón del mensaje', (
    tester,
  ) async {
    final backend = PluginFake()
      ..summary =
          'Inicia sesión en Gennie Web y pulsa Continuar en Plugins para comprobar la lectura.';
    final state = DesktopState(backend);
    addTearDown(state.dispose);
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ChatMessage(
            state: state,
            conversationId: '7',
            message: const Message(
              speaker: 'system',
              text:
                  'Plugin · mail.example: Inicia sesión en Gennie Web y pulsa Continuar en Plugins para comprobar la lectura.',
              createdAt: '2026-10-26T12:00:00Z',
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(
      find.text(
        'Inicia sesión en Gennie Web y pulsa Continuar en este mensaje para comprobar la lectura.',
      ),
      findsOneWidget,
    );
    expect(find.text('Abrir Gennie Web'), findsOneWidget);
    backend.status = 'cancelled';
    state.notifyListeners();
    await tester.pumpAndSettle();
    expect(
      find.text(
        'Inicia sesión en Gennie Web y pulsa Continuar en Plugins para comprobar la lectura.',
      ),
      findsOneWidget,
    );
    expect(find.text('Abrir Gennie Web'), findsNothing);
  });

  testWidgets('mensaje cancelado permite volver a generar y evita duplicados', (
    tester,
  ) async {
    final backend = PluginFake()..status = 'cancelled';
    final state = DesktopState(backend);
    addTearDown(state.dispose);
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ChatMessage(
            state: state,
            conversationId: '7',
            message: const Message(
              speaker: 'system',
              text:
                  'Plugin · mail.example: Construcción cancelada. La versión publicada se conserva.',
              createdAt: '2026-10-26T12:00:00Z',
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Volver a generar'), findsOneWidget);
    await tester.tap(find.text('Volver a generar'));
    await tester.pumpAndSettle();
    final request = backend.requests.singleWhere((r) => r['action'] == 'build');
    expect(request['trigger_conversation_id'], 7);
    expect(request['request']['plugin_id'], 'mail');
    expect(request['request']['objective'], 'Leer correos');
    expect(find.text('Volver a generar'), findsNothing);
  });

  testWidgets('acciones del trabajo vigente y errores en el chat', (
    tester,
  ) async {
    final backend = PluginFake();
    final state = DesktopState(backend);
    addTearDown(state.dispose);
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ChatMessage(
            state: state,
            conversationId: '7',
            message: const Message(
              speaker: 'system',
              text: 'Plugin · trabajo 1: Inicia sesión y pulsa Continuar.',
              createdAt: '2026-10-26T12:00:00Z',
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Abrir Gennie Web'), findsOneWidget);
    expect(find.text('Continuar'), findsNothing);
    await tester.tap(find.text('Abrir Gennie Web'));
    await tester.pumpAndSettle();
    expect(find.text('Abrir Gennie Web'), findsNothing);
    expect(find.text('Continuar'), findsOneWidget);
    expect(
      backend.requests.any((r) => r['action'] == 'connect' && r['job_id'] == 1),
      isTrue,
    );
    backend.fail = true;
    await tester.tap(find.text('Continuar'));
    await tester.pumpAndSettle();
    expect(
      find.textContaining('No se pudo realizar la acción'),
      findsOneWidget,
    );
    backend.fail = false;
    await tester.tap(find.text('Continuar'));
    await tester.pumpAndSettle();
    expect(find.text('Continuar'), findsNothing);
  });
  testWidgets('un fallo al abrir conserva el primer paso', (tester) async {
    final backend = PluginFake()..failConnect = true;
    final state = DesktopState(backend);
    addTearDown(state.dispose);
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ChatMessage(
            state: state,
            conversationId: '7',
            message: const Message(
              speaker: 'system',
              text: 'Plugin · trabajo 1: Inicia sesión y pulsa Continuar.',
              createdAt: '2026-10-26T12:00:00Z',
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Abrir Gennie Web'));
    await tester.pumpAndSettle();
    expect(find.text('Abrir Gennie Web'), findsOneWidget);
    expect(find.text('Continuar'), findsNothing);
    expect(
      find.textContaining('No se pudo realizar la acción'),
      findsOneWidget,
    );
  });
  testWidgets('un trabajo interrumpido puede continuar directamente', (
    tester,
  ) async {
    final backend = PluginFake()..status = 'interrupted';
    final state = DesktopState(backend);
    addTearDown(state.dispose);
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ChatMessage(
            state: state,
            conversationId: '7',
            message: const Message(
              speaker: 'system',
              text: 'Plugin · trabajo 1: Inicia sesión y pulsa Continuar.',
              createdAt: '2026-10-26T12:00:00Z',
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Continuar'), findsOneWidget);
    expect(find.text('Abrir Gennie Web'), findsNothing);
  });
  testWidgets('no ofrece acciones en otro chat', (tester) async {
    final state = DesktopState(PluginFake());
    addTearDown(state.dispose);
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ChatMessage(
            state: state,
            conversationId: '8',
            message: const Message(
              speaker: 'system',
              text: 'Plugin · mail.example: Inicia sesión y pulsa Continuar.',
              createdAt: '2026-10-26T12:00:00Z',
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Continuar'), findsNothing);
  });
}
