import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/chat_message.dart';
import 'widget_test.dart' show FakeBackend, snapshot, mount;

List<Message> messages(int count) => List.generate(
  count,
  (i) => Message(
    speaker: i.isEven ? 'user' : 'assistant',
    text: 'Entrada $i · contenido para leer',
    createdAt: '2026-10-28T12:00:00.000Z',
  ),
);
ScrollController controller(WidgetTester tester) => tester
    .widget<ListView>(find.byKey(const ValueKey('chat-timeline')))
    .controller!;

class DelayedBackend extends FakeBackend {
  Completer<ChatPage>? pending;
  bool failOlder = false;
  @override
  Future<ChatPage> loadChatPage(ChatCursor? before) {
    if (pending != null) {
      pageRequests.add(before);
      return pending!.future;
    }
    if (before != null && failOlder) {
      pageRequests.add(before);
      return Future.error(StateError('Fallo temporal de historial'));
    }
    return super.loadChatPage(before);
  }
}

class HistoricalBackend extends FakeBackend {
  @override
  Future<ChatPage> loadChatPage(ChatCursor? before) async {
    final page = await super.loadChatPage(before);
    return ChatPage(
      before: page.before,
      messages: [
        for (final entry in page.messages)
          ChatEntry(
            tools: const [],
            id: entry.id,
            conversationId: 'older',
            sessionStartedAt: entry.sessionStartedAt,
            startsSession: entry.startsSession,
            message: entry.message,
          ),
      ],
    );
  }
}

void main() {
  testWidgets(
    'carga inicial acotada y desplazamiento recuperan anteriores sin seleccionar sesiones',
    (tester) async {
      final (fake, _) = await mount(
        tester,
        initialSnapshot: snapshot(messages: messages(80)),
      );
      expect(fake.pageRequests, [null]);
      expect(
        find.text('Entrada 79 · contenido para leer').hitTestable(),
        findsOneWidget,
      );
      final scroll = controller(tester);
      scroll.jumpTo(scroll.position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(fake.pageRequests.length, 2);
      expect(fake.pageRequests.last?.messageId, '21');
      scroll.jumpTo(scroll.position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(
        find.text('Entrada 0 · contenido para leer').hitTestable(),
        findsOneWidget,
      );
      expect(fake.selected, isEmpty);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets(
    'una llegada nueva conserva la posición del mensaje visible al leer anteriores',
    (tester) async {
      final (fake, state) = await mount(
        tester,
        initialSnapshot: snapshot(messages: messages(100)),
      );
      final scroll = controller(tester);
      scroll.jumpTo(scroll.position.maxScrollExtent / 2);
      await tester.pumpAndSettle();
      final visible =
          find.byType(ChatMessage).hitTestable().evaluate().first.widget
              as ChatMessage;
      final text = visible.message.text;
      final finder = find.byWidgetPredicate(
        (w) => w is ChatMessage && w.message.text == text,
      );
      final before = tester.getTopLeft(finder).dy;
      state.accept(fake.value = snapshot(messages: messages(101)));
      await tester.pumpAndSettle();
      expect(tester.getTopLeft(finder).dy, closeTo(before, 1));
      expect(fake.selected, isEmpty);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets(
    'una ráfaga mayor que una página conserva el tramo y el mensaje visible',
    (tester) async {
      final (fake, state) = await mount(
        tester,
        initialSnapshot: snapshot(messages: messages(100)),
      );
      final scroll = controller(tester);
      scroll.jumpTo(scroll.position.maxScrollExtent / 2);
      await tester.pumpAndSettle();
      final visible =
          find.byType(ChatMessage).hitTestable().evaluate().first.widget
              as ChatMessage;
      final text = visible.message.text;
      final finder = find.byWidgetPredicate(
        (w) => w is ChatMessage && w.message.text == text,
      );
      final before = tester.getTopLeft(finder).dy;
      state.accept(fake.value = snapshot(messages: messages(201)));
      await tester.pumpAndSettle();
      expect(tester.getTopLeft(finder).dy, closeTo(before, 1));
      expect(fake.selected, isEmpty);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets(
    'error al cargar anteriores conserva contenido y ofrece recuperación',
    (tester) async {
      final fake = DelayedBackend()..value = snapshot(messages: messages(80));
      await mount(tester, backend: fake);
      fake.failOlder = true;
      final scroll = controller(tester);
      scroll.jumpTo(scroll.position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(
        find.textContaining('Fallo temporal de historial'),
        findsOneWidget,
      );
      fake.failOlder = false;
      await tester.tap(find.text('Reintentar historial'));
      await tester.pumpAndSettle();
      expect(find.textContaining('Fallo temporal de historial'), findsNothing);
      scroll.jumpTo(0);
      await tester.pumpAndSettle();
      expect(
        find.text('Entrada 79 · contenido para leer').hitTestable(),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets(
    'respuesta tardía tras desmontar el lector no modifica widgets retirados',
    (tester) async {
      final fake = DelayedBackend()..value = snapshot(messages: messages(80));
      final (_, state) = await mount(tester, backend: fake);
      fake.pending = Completer<ChatPage>();
      state.accept(fake.value = snapshot(messages: messages(81)));
      await tester.pump();
      await tester.pumpWidget(const SizedBox());
      fake.pending!.complete(const ChatPage(messages: []));
      await tester.pump();
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets(
    'sesión activa vacía muestra su separador y conserva historial y borrador',
    (tester) async {
      final previous = messages(5);
      final fake = HistoricalBackend()..value = snapshot(messages: previous);
      final (_, state) = await mount(tester, backend: fake);
      await tester.enterText(
        find.widgetWithText(TextField, 'Escribe a Gennie…'),
        'Borrador conservado',
      );
      final started = DateTime.now().toUtc().toIso8601String();
      state.accept(
        fake.value = snapshot(
          messages: previous,
          emptyActive: true,
          sessionStartedAt: started,
        ),
      );
      await tester.pumpAndSettle();
      expect(
        find
            .text('Nueva conversación · Hoy ${messageTimeLabel(started)}')
            .hitTestable(),
        findsOneWidget,
      );
      expect(
        find.text('Entrada 4 · contenido para leer').hitTestable(),
        findsOneWidget,
      );
      expect(find.text('Borrador conservado'), findsOneWidget);
      expect(fake.selected, isEmpty);
      expect(tester.takeException(), isNull);
    },
  );
}
