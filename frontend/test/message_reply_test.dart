import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/message_quote.dart';
import 'widget_test.dart'
    show FakeBackend, snapshot, mount, composer, draft, appTooltip;
import 'chats_preview_test.dart' show loadFonts;

final initial = [
  const Message(
    speaker: 'user',
    text: 'Mensaje propio',
    createdAt: '2026-10-28T00:00:00.000Z',
  ),
  const Message(
    speaker: 'assistant',
    text: 'Respuesta de Gennie',
    createdAt: '2026-10-28T00:01:00.000Z',
  ),
];
Future<void> reply(WidgetTester tester, String text) async {
  await tester.ensureVisible(find.text(text).first);
  await tester.pumpAndSettle();
  await tester.tap(find.text(text).first, buttons: 2);
  await tester.pumpAndSettle();
  await tester.tap(find.text('Responder'));
  await tester.pumpAndSettle();
}

class QuoteBackend extends FakeBackend {
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
            conversationId: int.parse(entry.id) < 70 ? 'older' : 'active',
            sessionStartedAt: entry.sessionStartedAt,
            startsSession: entry.startsSession,
            message: entry.message,
            quote: entry.id == '140'
                ? const ReplyQuote(
                    sourceMessageId: '1',
                    speaker: 'user',
                    text: 'Original 0',
                    createdAt: '2026-10-28T00:00:00.000Z',
                    truncated: false,
                  )
                : null,
          ),
      ],
    );
  }
}

void main() {
  for (final brightness in Brightness.values) {
    testWidgets(
      'cita al mínimo con nombre largo, texto ampliado y alto contraste ${brightness.name}',
      (tester) async {
        tester.platformDispatcher.textScaleFactorTestValue = 1.5;
        tester.platformDispatcher.accessibilityFeaturesTestValue =
            const FakeAccessibilityFeatures(highContrast: true);
        tester.platformDispatcher.platformBrightnessTestValue = brightness;
        addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
        addTearDown(
          tester.platformDispatcher.clearAccessibilityFeaturesTestValue,
        );
        addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);
        await loadFonts();
        await mount(
          tester,
          size: const Size(640, 420),
          initialSnapshot: snapshot(
            title: 'Asistente con nombre muy largo '.padRight(200, 'á'),
            messages: initial,
          ),
        );
        await reply(tester, 'Respuesta de Gennie');
        await tester.enterText(composer(), 'Borrador legible');
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        expect(appTooltip('Cancelar respuesta').hitTestable(), findsOneWidget);
        expect(composer().hitTestable(), findsOneWidget);
        expect(appTooltip('Enviar mensaje').hitTestable(), findsOneWidget);
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          final boundary = tester.renderObject<RenderRepaintBoundary>(
            find.byKey(const ValueKey('capture')),
          );
          await tester.runAsync(() async {
            final image = await boundary.toImage();
            final bytes = await image.toByteData(
              format: ui.ImageByteFormat.png,
            );
            final directory = Directory('../.verification/assistant-chat-real')
              ..createSync(recursive: true);
            File(
              '${directory.path}/quote-accessibility-${brightness.name}.png',
            ).writeAsBytesSync(bytes!.buffer.asUint8List());
            image.dispose();
          });
        }
        await tester.tap(appTooltip('Cancelar respuesta'));
        await tester.pumpAndSettle();
        expect(draft(tester), 'Borrador legible');
        expect(
          tester.widget<TextField>(composer()).focusNode!.hasFocus,
          isTrue,
        );
      },
    );
  }
  testWidgets(
    'responder al asistente envía referencia y conserva cita/borrador hasta el ack',
    (tester) async {
      final (fake, state) = await mount(
        tester,
        initialSnapshot: snapshot(messages: initial),
      );
      await reply(tester, 'Respuesta de Gennie');
      expect(tester.widget<TextField>(composer()).focusNode!.hasFocus, isTrue);
      expect(
        tester
            .widget<MessageQuote>(find.byType(MessageQuote))
            .quote
            .sourceMessageId,
        '2',
      );
      await tester.enterText(composer(), 'Mi respuesta literal');
      fake.pendingSend = Completer<Snapshot>();
      await tester.tap(appTooltip('Enviar mensaje'));
      await tester.pump();
      expect(fake.replied, [('Mi respuesta literal', '2')]);
      expect(fake.sent, ['Mi respuesta literal']);
      expect(draft(tester), 'Mi respuesta literal');
      expect(find.byType(MessageQuote), findsOneWidget);
      state.accept(
        fake.value = snapshot(
          messages: initial,
          ack: 1,
          acknowledged: 'Mi respuesta literal',
        ),
      );
      fake.pendingSend!.complete(fake.value);
      await tester.pumpAndSettle();
      expect(draft(tester), isEmpty);
      expect(find.byType(MessageQuote), findsNothing);
      expect(fake.selected, isEmpty);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets('cancelar cita conserva el texto y el foco del compositor', (
    tester,
  ) async {
    await mount(
      tester,
      initialSnapshot: snapshot(messages: initial),
      size: const Size(640, 420),
    );
    await reply(tester, 'Respuesta de Gennie');
    await tester.enterText(composer(), 'Borrador');
    await tester.tap(appTooltip('Cancelar respuesta'));
    await tester.pumpAndSettle();
    expect(find.byType(MessageQuote), findsNothing);
    expect(draft(tester), 'Borrador');
    expect(tester.widget<TextField>(composer()).focusNode!.hasFocus, isTrue);
    expect(tester.takeException(), isNull);
  });
  testWidgets(
    'cambiar de cita mientras se guarda protege la nueva selección ante un ack anterior',
    (tester) async {
      final (fake, state) = await mount(
        tester,
        initialSnapshot: snapshot(messages: initial),
      );
      await reply(tester, 'Mensaje propio');
      await tester.enterText(composer(), 'Mismo texto');
      fake.pendingSend = Completer<Snapshot>();
      await tester.tap(appTooltip('Enviar mensaje'));
      await tester.pump();
      await reply(tester, 'Respuesta de Gennie');
      state.accept(
        fake.value = snapshot(
          messages: initial,
          ack: 1,
          acknowledged: 'Mismo texto',
        ),
      );
      fake.pendingSend!.complete(fake.value);
      await tester.pumpAndSettle();
      expect(fake.replied, [('Mismo texto', '1')]);
      expect(draft(tester), 'Mismo texto');
      expect(
        tester
            .widget<MessageQuote>(find.byType(MessageQuote))
            .quote
            .sourceMessageId,
        '2',
      );
    },
  );
  testWidgets('un fallo de guardado no pierde el texto ni la cita', (
    tester,
  ) async {
    final (fake, _) = await mount(
      tester,
      initialSnapshot: snapshot(messages: initial),
    );
    await reply(tester, 'Respuesta de Gennie');
    await tester.enterText(composer(), 'Conservar');
    fake.pendingSend = Completer<Snapshot>();
    await tester.tap(appTooltip('Enviar mensaje'));
    await tester.pump();
    fake.pendingSend!.completeError(StateError('No se pudo guardar'));
    await tester.pumpAndSettle();
    expect(draft(tester), 'Conservar');
    expect(find.byType(MessageQuote), findsOneWidget);
    expect(find.textContaining('No se pudo guardar'), findsWidgets);
  });
  testWidgets('Shift+F10 abre Responder para un mensaje propio', (
    tester,
  ) async {
    await mount(tester, initialSnapshot: snapshot(messages: initial));
    Focus.of(tester.element(find.text('Mensaje propio'))).requestFocus();
    await tester.pump();
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.f10);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Responder'));
    await tester.pumpAndSettle();
    expect(
      tester.widget<MessageQuote>(find.byType(MessageQuote)).quote.speaker,
      'user',
    );
  });
  testWidgets(
    'abrir cita carga páginas anteriores y localiza el original sin activar su sesión',
    (tester) async {
      final fake = QuoteBackend()
        ..value = snapshot(
          messages: List.generate(
            140,
            (i) => Message(
              speaker: 'user',
              text: i == 0 ? 'Original 0' : 'Entrada $i',
              createdAt: '2026-10-28T00:00:00.000Z',
            ),
          ),
        );
      await mount(tester, backend: fake);
      expect(fake.pageRequests.length, 1);
      await tester.tap(find.byType(MessageQuote));
      await tester.pumpAndSettle();
      expect(fake.pageRequests.length, greaterThan(1));
      expect(
        find
            .descendant(
              of: find.byKey(const ValueKey('1')),
              matching: find.text('Original 0'),
            )
            .hitTestable(),
        findsOneWidget,
      );
      expect(fake.selected, isEmpty);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets(
    'abrir una cita por teclado permite responder al original sin enfocar otra sesión',
    (tester) async {
      final fake = QuoteBackend()
        ..value = snapshot(
          messages: List.generate(
            140,
            (i) => Message(
              speaker: 'user',
              text: i == 0 ? 'Original 0' : 'Entrada $i',
              createdAt: '2026-10-28T00:00:00.000Z',
            ),
          ),
        );
      await mount(tester, backend: fake);
      final quote = find.byType(MessageQuote);
      Focus.of(
        tester.element(
          find.descendant(of: quote, matching: find.byType(Text)).first,
        ),
      ).requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await tester.pumpAndSettle();
      expect(fake.pageRequests.length, greaterThan(1));
      if (const bool.fromEnvironment('CAPTURE_UI')) {
        final boundary = tester.renderObject<RenderRepaintBoundary>(
          find.byKey(const ValueKey('capture')),
        );
        await tester.runAsync(() async {
          final image = await boundary.toImage();
          final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
          File(
            '../.verification/assistant-chat-real/quote-original-focus-macos.png',
          ).writeAsBytesSync(bytes!.buffer.asUint8List());
          image.dispose();
        });
      }
      await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.f10);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
      await tester.pumpAndSettle();
      expect(find.text('Responder'), findsOneWidget);
      await tester.tap(find.text('Responder'));
      await tester.pumpAndSettle();
      final preview = tester
          .widgetList<MessageQuote>(find.byType(MessageQuote))
          .where((w) => w.onOpen == null)
          .single;
      expect(preview.quote.sourceMessageId, '1');
      expect(preview.quote.text, 'Original 0');
      expect(fake.selected, isEmpty);
      expect(fake.sent, isEmpty);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );

  testWidgets(
    'eliminar el historial libera el foco del original y mantiene editable el compositor',
    (tester) async {
      final fake = QuoteBackend()
        ..value = snapshot(
          messages: List.generate(
            140,
            (i) => Message(
              speaker: 'user',
              text: i == 0 ? 'Original 0' : 'Entrada $i',
              createdAt: '2026-10-28T00:00:00.000Z',
            ),
          ),
        );
      final (_, state) = await mount(tester, backend: fake);
      await tester.tap(find.byType(MessageQuote));
      await tester.pumpAndSettle();
      state.accept(fake.value = snapshot(messages: []));
      await tester.pumpAndSettle();
      expect(find.text('Original 0'), findsNothing);
      await tester.enterText(composer(), 'Sigo escribiendo');
      expect(draft(tester), 'Sigo escribiendo');
      expect(fake.sent, isEmpty);
      expect(fake.selected, isEmpty);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'una cita cuyo original fue eliminado conserva el fragmento legible',
    (tester) async {
      await tester.pumpWidget(
        const MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              child: MessageQuote(
                quote: ReplyQuote(
                  speaker: 'assistant',
                  text: 'Fragmento conservado',
                  createdAt: '2026-10-28T00:00:00.000Z',
                  truncated: false,
                ),
              ),
            ),
          ),
        ),
      );
      expect(find.text('Fragmento conservado'), findsOneWidget);
      expect(find.text('Mensaje original no disponible'), findsOneWidget);
      expect(find.byType(InkWell), findsNothing);
      expect(tester.takeException(), isNull);
    },
  );
}
