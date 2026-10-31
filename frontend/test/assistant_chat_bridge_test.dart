import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:gennie/main.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/message_quote.dart';
import 'package:gennie/src/tool_evidence.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:gennie/src/rust/api.dart' as api;
import 'package:gennie/src/rust/frb_generated.dart';

bool libraryReady = false;
// Comparar datos, no identidad de las listas que decodifica FRB en cada lectura.
List<Object?> entryValue(api.ChatEntry entry) => [
  entry.id,
  entry.conversationId,
  entry.sessionStartedAt,
  entry.startsSession,
  entry.message,
  entry.quote,
  entry.tools,
];
Future<void> ensureLibrary(String library) async {
  if (!libraryReady) {
    await RustLib.init(externalLibrary: ExternalLibrary.open(library));
    libraryReady = true;
  }
}

class BridgeBackend extends RustBackend {
  BridgeBackend(this.env, this.helper);
  final String env, helper;
  int reads = 0;
  @override
  Future<Snapshot> initialize() =>
      api.initialize(envFile: env, helperPath: helper);
  @override
  Future<ChatPage> loadChatPage(ChatCursor? before) async {
    final page = await super.loadChatPage(before);
    reads++;
    return page;
  }
}

Future<void> pumpUntil(WidgetTester tester, bool Function() done) async {
  for (var attempt = 0; attempt < 100; attempt++) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 20)),
    );
    await tester.pump();
    if (done()) return;
  }
  throw TimeoutException('El lector no recibió su página');
}

Future<void> capture(WidgetTester tester, String name) async {
  final boundary = tester.renderObject<RenderRepaintBoundary>(
    find.byKey(const ValueKey('real-chat-capture')),
  );
  await tester.runAsync(() async {
    final image = await boundary.toImage();
    final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
    final output = Directory('../.verification/assistant-chat-real')
      ..createSync(recursive: true);
    File(
      '${output.path}/$name.png',
    ).writeAsBytesSync(bytes!.buffer.asUint8List());
    image.dispose();
  });
}

// Sonda optativa con SQLite temporal preparado por scripts/probes/assistant_chat_bridge.py.
// Sin modelos, navegador, permisos del sistema ni acceso a cuentas.
void main() {
  const library = String.fromEnvironment('GENNIE_BRIDGE_SMOKE_LIBRARY');
  const env = String.fromEnvironment('GENNIE_BRIDGE_SMOKE_ENV');
  const helper = String.fromEnvironment('GENNIE_BRIDGE_SMOKE_HELPER');
  test(
    'FRB real: identidad, páginas, cursor inválido y reapertura',
    () async {
      await ensureLibrary(library);
      bool running = false;
      StreamSubscription<api.Snapshot>? events;
      try {
        final initial = await api.initialize(envFile: env, helperPath: helper);
        running = true;
        expect(initial.assistant, isNull);
        expect(initial.assistantProfile?.id, 'gennie');
        expect(initial.assistantProfile?.name, 'Gennie');
        expect(initial.assistantProfile?.avatarRevision, isNull);
        expect(initial.activeId, '2');
        final watched = Completer<api.Snapshot>();
        events = api.watch().listen((value) {
          if (!watched.isCompleted) watched.complete(value);
        });
        expect(
          (await watched.future.timeout(
            const Duration(seconds: 5),
          )).assistantProfile,
          initial.assistantProfile,
        );
        final latest = await api.loadChatPage(before: null);
        expect(latest.messages.length, 60);
        expect(latest.before, isNotNull);
        final quoted = latest.messages.firstWhere((entry) => entry.id == '76');
        expect(quoted.quote?.sourceMessageId, '2');
        expect(quoted.quote?.text, 'Mensaje de ejemplo 2 · verificación');
        expect(latest.messages.last.id, '77');
        expect(latest.messages.last.tools.map((u) => u.outcome), [
          api.ToolCallOutcome.completed,
          api.ToolCallOutcome.failed,
          api.ToolCallOutcome.unknown,
        ]);
        final older = await api.loadChatPage(before: latest.before);
        expect(older.messages.length, 17);
        expect(older.before, isNull);
        final all = [...older.messages, ...latest.messages];
        expect(all.map((m) => m.id).toSet().length, 77);
        expect(all.map((m) => m.conversationId).toSet(), {'1', '2'});
        expect(all.first.message.text, 'Mensaje de ejemplo 1 · verificación');
        await expectLater(
          api.loadChatPage(
            before: const api.ChatCursor(
              messageId: 'otro',
              createdAt: '2026-10-28T00:00:00.000Z',
            ),
          ),
          throwsA(anything),
        );
        await expectLater(
          api.loadChatPage(
            before: const api.ChatCursor(
              messageId: '9999',
              createdAt: '2026-10-28T00:00:00.000Z',
            ),
          ),
          throwsA(anything),
        );
        expect(
          (await api.loadChatPage(before: null)).messages.map(entryValue),
          latest.messages.map(entryValue),
        );
        final renamed = await api.renameAssistant(name: 'Mi Gennie · personal');
        expect(renamed.assistantProfile?.name, 'Mi Gennie · personal');
        expect(renamed.conversations, initial.conversations);
        expect(renamed.activeId, initial.activeId);
        expect(renamed.historyRevision, initial.historyRevision);
        expect(
          renamed.error,
          isNull,
          reason: 'Una consulta fallida no publica avisos de sistema',
        );
        final image = await File(
          '${File(env).parent.path}/avatar.png',
        ).readAsBytes();
        final prepared = await api.prepareAssistantAvatar(image: image);
        final saved = await api.saveAssistantProfile(
          name: 'Mi Gennie · personal',
          avatar: prepared,
        );
        expect(saved.assistantProfile?.avatarRevision, '1');
        expect(saved.activeId, initial.activeId);
        expect(saved.conversations, initial.conversations);
        expect(await api.loadAssistantAvatar(revision: '1'), prepared);
        await expectLater(
          api.loadAssistantAvatar(revision: 'no válida'),
          throwsA(anything),
        );
        await expectLater(
          api.loadAssistantAvatar(revision: '2'),
          throwsA(anything),
        );
        expect(
          (await api.loadChatPage(before: null)).messages.map(entryValue),
          latest.messages.map(entryValue),
        );
        await api.shutdown();
        running = false;
        await events.cancel();
        events = null;
        final reopened = await api.initialize(envFile: env, helperPath: helper);
        running = true;
        expect(reopened.assistantProfile?.name, 'Mi Gennie · personal');
        expect(reopened.assistantProfile?.avatarRevision, '1');
        expect(reopened.error, isNull);
        expect(await api.loadAssistantAvatar(revision: '1'), prepared);
        expect(reopened.activeId, initial.activeId);
        expect(
          (await api.loadChatPage(before: null)).messages.map(entryValue),
          latest.messages.map(entryValue),
        );
      } finally {
        if (running) await api.shutdown();
        await events?.cancel();
      }
    },
    skip: library.isEmpty || env.isEmpty || helper.isEmpty,
    timeout: const Timeout(Duration(seconds: 30)),
  );
  for (final brightness in Brightness.values) {
    for (final size in [const Size(1100, 720), const Size(640, 420)]) {
      testWidgets(
        'lector Flutter con FRB real ${brightness.name} ${size.width}',
        (tester) async {
          tester.view.physicalSize = size;
          tester.view.devicePixelRatio = 1;
          tester.platformDispatcher.platformBrightnessTestValue = brightness;
          addTearDown(tester.view.resetPhysicalSize);
          addTearDown(tester.view.resetDevicePixelRatio);
          addTearDown(
            tester.platformDispatcher.clearPlatformBrightnessTestValue,
          );
          final fonts = FontLoader('Inter');
          for (final weight in ['Regular', 'Medium', 'SemiBold', 'Bold']) {
            fonts.addFont(rootBundle.load('assets/fonts/Inter-$weight.ttf'));
          }
          final icons = FontLoader('MaterialIcons')
            ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
          await tester.runAsync(() async {
            await ensureLibrary(library);
            await fonts.load();
            await icons.load();
          });
          final backend = BridgeBackend(env, helper);
          final state = DesktopState(backend);
          try {
            await tester.runAsync(state.start);
            expect(state.failure, isNull);
            await tester.pumpWidget(
              RepaintBoundary(
                key: const ValueKey('real-chat-capture'),
                child: GennieApp(state: state),
              ),
            );
            await tester.pump();
            await pumpUntil(tester, () => backend.reads == 1);
            await tester.pumpAndSettle();
            expect(backend.reads, 1);
            expect(find.text('Mi Gennie · personal'), findsNWidgets(2));
            await pumpUntil(
              tester,
              () => find.byType(Image).evaluate().length == 2,
            );
            await tester.pumpAndSettle();
            expect(find.text('Sesión 1'), findsNothing);
            expect(
              find
                  .descendant(
                    of: find.byKey(const ValueKey('chat-timeline')),
                    matching: find.text('Resultado de la consulta de ejemplo'),
                  )
                  .hitTestable(),
              findsOneWidget,
            );
            expect(tester.takeException(), isNull);
            await capture(tester, '${brightness.name}-${size.width.toInt()}');
            final evidence = find.byType(ToolEvidence);
            expect(evidence, findsOneWidget);
            final evidenceLabel = find.text(
              'Usó: Correo de prueba · 1 fallo · 1 sin confirmar',
            );
            await tester.ensureVisible(evidenceLabel);
            await tester.pumpAndSettle();
            await tester.tap(evidenceLabel);
            await tester.pumpAndSettle();
            expect(find.text('Finalizada'), findsOneWidget);
            expect(find.text('Falló'), findsOneWidget);
            expect(find.text('Sin confirmar'), findsOneWidget);
            await capture(
              tester,
              'real-tool-details-${brightness.name}-${size.width.toInt()}',
            );
            await tester.tap(find.text('Listo'));
            await tester.pumpAndSettle();
            final scroll = tester
                .widget<ListView>(find.byKey(const ValueKey('chat-timeline')))
                .controller!;
            await tester.ensureVisible(find.byType(MessageQuote));
            await tester.pumpAndSettle();
            await tester.tap(find.byType(MessageQuote));
            await tester.pump();
            await pumpUntil(tester, () => backend.reads == 2);
            await tester.pumpAndSettle();
            expect(backend.reads, 2);
            expect(
              find
                  .descendant(
                    of: find.byKey(const ValueKey('2')),
                    matching: find.text('Mensaje de ejemplo 2 · verificación'),
                  )
                  .hitTestable(),
              findsOneWidget,
            );
            await tester.tap(
              find.descendant(
                of: find.byKey(const ValueKey('2')),
                matching: find.text('Mensaje de ejemplo 2 · verificación'),
              ),
              buttons: 2,
            );
            await tester.pumpAndSettle();
            await tester.tap(find.text('Responder'));
            await tester.pumpAndSettle();
            final composer = find.widgetWithText(
              TextField,
              'Escribe a Gennie…',
            );
            await tester.enterText(composer, 'Borrador de respuesta');
            await tester.pumpAndSettle();
            expect(tester.takeException(), isNull);
            await capture(
              tester,
              'reply-preview-${brightness.name}-${size.width.toInt()}',
            );
            await tester.tap(find.byIcon(Icons.close));
            await tester.pumpAndSettle();
            expect(
              tester.widget<TextField>(composer).controller!.text,
              'Borrador de respuesta',
            );
            scroll.jumpTo(scroll.position.maxScrollExtent);
            await tester.pumpAndSettle();
            expect(
              find.text('Mensaje de ejemplo 1 · verificación').hitTestable(),
              findsOneWidget,
            );
            expect(state.snapshot?.activeId, '2');
            expect(tester.takeException(), isNull);
          } finally {
            await tester.pumpWidget(const SizedBox());
            await tester.runAsync(state.close);
            state.dispose();
          }
        },
        skip: library.isEmpty || env.isEmpty || helper.isEmpty,
      );
    }
  }
  testWidgets(
    'borrar el original actualiza la cita visible y conserva su fragmento',
    (tester) async {
      final backend = BridgeBackend(env, helper);
      final state = DesktopState(backend);
      try {
        await tester.runAsync(() async {
          await ensureLibrary(library);
          await state.start();
        });
        await tester.pumpWidget(GennieApp(state: state));
        await tester.pump();
        await pumpUntil(tester, () => backend.reads == 1);
        await tester.pumpAndSettle();
        expect(
          tester
              .widget<MessageQuote>(find.byType(MessageQuote))
              .quote
              .sourceMessageId,
          '2',
        );
        await tester.runAsync(
          () => state.perform(() => api.deleteConversation(id: '1')),
        );
        await tester.pump();
        await pumpUntil(tester, () => backend.reads == 2);
        await tester.pumpAndSettle();
        final quote = tester.widget<MessageQuote>(find.byType(MessageQuote));
        expect(quote.quote.sourceMessageId, isNull);
        expect(quote.quote.text, 'Mensaje de ejemplo 2 · verificación');
        expect(find.text('Mensaje original no disponible'), findsOneWidget);
        expect(state.snapshot?.activeId, '2');
        expect(tester.takeException(), isNull);
      } finally {
        await tester.pumpWidget(const SizedBox());
        await tester.runAsync(state.close);
        state.dispose();
      }
    },
    skip: library.isEmpty || env.isEmpty || helper.isEmpty,
  );
}
