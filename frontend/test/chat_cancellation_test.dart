import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/backend.dart';
import 'widget_test.dart'
    show FakeBackend, snapshot, mount, composer, draft, appTooltip;
import 'chats_preview_test.dart' show loadFonts;
import 'message_reply_test.dart' show reply, initial;
import 'package:gennie/src/message_quote.dart';
import 'tool_evidence_test.dart' show measureContrast;

class StopBackend extends FakeBackend implements TurnCancellationBackend {
  final stops = <String>[];
  Completer<void>? gate;
  bool fail = false;
  @override
  Future<void> stopResponse(String id) async {
    stops.add(id);
    if (fail) throw StateError('No se pudo detener la respuesta');
    await gate?.future;
  }
}

Snapshot responding(
  String id, {
  bool stopping = false,
  String text = 'Texto que ya llegó',
}) => snapshot(
  busy: true,
  turn: TurnProgress(id: id, stopping: stopping),
  reply: text,
);
Future<void> update(
  WidgetTester t,
  StopBackend b,
  DesktopState state,
  Snapshot value,
) async {
  state.accept(b.value = value);
  await t.pump();
  await t.pump(const Duration(milliseconds: 100));
}

final stop = find.byKey(const ValueKey('stop-response'));

void main() {
  testWidgets(
    'detener envía solo la identidad activa, bloquea doble clic y conserva borrador',
    (t) async {
      final b = StopBackend()..gate = Completer<void>();
      final (_, state) = await mount(t, backend: b);
      await t.enterText(composer(), 'Borrador siguiente');
      await update(t, b, state, responding('first'));
      expect(t.widget<IconButton>(stop).onPressed, isNotNull);
      await t.tap(stop);
      await t.pump();
      await t.tap(stop);
      await t.pump();
      expect(b.stops, ['first']);
      expect(draft(t), 'Borrador siguiente');
      expect(find.text('Deteniendo respuesta…'), findsOneWidget);
      expect(t.widget<IconButton>(stop).onPressed, isNull);
      await update(t, b, state, snapshot(confirmation: 'Respuesta detenida'));
      b.gate!.complete();
      await t.pump();
      expect(stop, findsNothing);
      expect(find.byKey(const ValueKey('send-message')), findsOneWidget);
      expect(draft(t), 'Borrador siguiente');
      expect(b.sent, isEmpty);
    },
  );
  testWidgets(
    'respuesta tardía de detener no sustituye una generación nueva ni roba el foco',
    (t) async {
      final b = StopBackend()..gate = Completer<void>();
      final (_, state) = await mount(t, backend: b);
      await update(t, b, state, responding('old'));
      await t.tap(stop);
      await t.pump();
      await update(t, b, state, responding('new', text: 'Respuesta nueva'));
      await t.enterText(composer(), 'Edición nueva');
      b.gate!.complete();
      await t.pump();
      expect(state.snapshot!.turn!.id, 'new');
      expect(state.snapshot!.reply, 'Respuesta nueva');
      expect(draft(t), 'Edición nueva');
      expect(t.widget<IconButton>(stop).onPressed, isNotNull);
      expect(state.failure, isNull);
      await update(t, b, state, snapshot());
    },
  );
  testWidgets('fallo permite reintentar y no borra el texto recibido', (
    t,
  ) async {
    final b = StopBackend()..fail = true;
    final (_, state) = await mount(t, backend: b);
    await update(t, b, state, responding('current'));
    await t.tap(stop);
    await t.pump();
    expect(state.failure, contains('No se pudo detener'));
    expect(state.snapshot!.reply, 'Texto que ya llegó');
    expect(t.widget<IconButton>(stop).onPressed, isNotNull);
    b.fail = false;
    await t.tap(stop);
    await t.pump();
    expect(b.stops, ['current', 'current']);
    await update(t, b, state, snapshot());
  });
  testWidgets(
    'teclado activa detener y sus etiquetas semánticas son accesibles',
    (t) async {
      final semantics = t.ensureSemantics();
      try {
        final b = StopBackend();
        final (_, state) = await mount(t, backend: b);
        await update(t, b, state, responding('keyboard'));
        await t.tap(composer());
        await t.sendKeyEvent(LogicalKeyboardKey.tab);
        await t.pump();
        await t.sendKeyEvent(LogicalKeyboardKey.enter);
        await t.pump();
        expect(b.stops, ['keyboard']);
        expect(appTooltip('Deteniendo respuesta'), findsOneWidget);
        await update(t, b, state, snapshot());
      } finally {
        semantics.dispose();
      }
    },
  );
  testWidgets(
    'ocupado por conectar no ofrece detener sin identidad de respuesta',
    (t) async {
      final b = StopBackend();
      final (_, state) = await mount(t, backend: b);
      await update(t, b, state, snapshot(busy: true));
      expect(stop, findsNothing);
      expect(b.stops, isEmpty);
      await update(t, b, state, snapshot());
    },
  );
  testWidgets('detener no retira una cita ni borra la edición sin ACK', (
    t,
  ) async {
    final b = StopBackend()..gate = Completer<void>();
    final (_, state) = await mount(
      t,
      backend: b,
      initialSnapshot: snapshot(messages: initial),
    );
    await reply(t, 'Respuesta de Gennie');
    await t.enterText(composer(), 'Texto para responder después');
    await update(
      t,
      b,
      state,
      snapshot(
        busy: true,
        turn: TurnProgress(id: 'quote', stopping: false),
        messages: initial,
      ),
    );
    await t.tap(stop);
    await t.pump();
    expect(find.byType(MessageQuote), findsOneWidget);
    await update(
      t,
      b,
      state,
      snapshot(messages: initial, confirmation: 'Respuesta detenida'),
    );
    b.gate!.complete();
    await t.pump();
    expect(find.byType(MessageQuote), findsOneWidget);
    expect(draft(t), 'Texto para responder después');
    expect(b.sent, isEmpty);
  });
  testWidgets(
    'error tardío al detener no publica un fallo sobre otra generación',
    (t) async {
      final b = StopBackend()..gate = Completer<void>();
      final (_, state) = await mount(t, backend: b);
      await update(t, b, state, responding('old'));
      await t.tap(stop);
      await t.pump();
      await update(t, b, state, responding('new'));
      b.gate!.completeError(StateError('Fallo anterior'));
      await t.pump();
      expect(state.failure, isNull);
      expect(state.snapshot!.turn!.id, 'new');
      expect(t.widget<IconButton>(stop).onPressed, isNotNull);
      await update(t, b, state, snapshot());
    },
  );
  for (final ending in MessageInterruption.values) {
    testWidgets(
      'fragmento sin guardar ${ending.name} conserva texto y estado visible',
      (t) async {
        final b = StopBackend();
        final (_, state) = await mount(t, backend: b);
        await update(
          t,
          b,
          state,
          snapshot(
            messages: const [],
            reply: 'Texto parcial visible',
            replyInterruption: ending,
          ),
        );
        expect(
          find.text(
            ending == MessageInterruption.cancelled
                ? 'Respuesta detenida · sin guardar'
                : 'Respuesta interrumpida · sin guardar',
          ),
          findsOneWidget,
        );
        expect(
          find.textContaining('Texto parcial visible', findRichText: true),
          findsWidgets,
        );
        expect(b.sent, isEmpty);
        expect(b.stops, isEmpty);
      },
    );
  }
  for (final brightness in Brightness.values) {
    for (final spec in [
      (const Size(1100, 720), 1.0),
      (const Size(640, 420), 1.0),
      (const Size(640, 420), 1.5),
    ]) {
      testWidgets('detener visual ${brightness.name} ${spec.$1.width} ${spec.$2}', (
        t,
      ) async {
        await t.runAsync(loadFonts);
        t.platformDispatcher.platformBrightnessTestValue = brightness;
        addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
        t.platformDispatcher.textScaleFactorTestValue = spec.$2;
        addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
        final b = StopBackend()..gate = Completer<void>();
        final (_, state) = await mount(t, backend: b, size: spec.$1);
        await t.enterText(composer(), 'Un borrador que puedo seguir editando.');
        await update(t, b, state, responding('visual'));
        Future<void> capture(String phase) async {
          if (!const bool.fromEnvironment('CAPTURE_UI')) return;
          await t.runAsync(() async {
            final render = t.renderObject<RenderRepaintBoundary>(
              find.byKey(const ValueKey('capture')),
            );
            final image = await render.toImage();
            final data = await image.toByteData(format: ui.ImageByteFormat.png);
            final path =
                '../.verification/assistant-chat-real/stop-$phase-${brightness.name}-${spec.$1.width.toInt()}-${spec.$2}';
            File('$path.png').writeAsBytesSync(data!.buffer.asUint8List());
            image.dispose();
          });
        }

        await capture('active');
        await t.tap(stop);
        await t.pump(const Duration(milliseconds: 100));
        await capture('pending');
        await update(
          t,
          b,
          state,
          snapshot(
            confirmation: 'Respuesta detenida',
            messages: [
              const Message(
                speaker: 'assistant',
                text:
                    'Este es el fragmento recibido antes de detener la respuesta. Conserva el contenido y puede consultarse al volver a abrir Gennie.',
                createdAt: '2026-10-29T15:00:00.000Z',
                interruption: MessageInterruption.cancelled,
              ),
            ],
          ),
        );
        b.gate!.complete();
        await t.pump();
        await t.pump(const Duration(milliseconds: 200));
        expect(find.text('Respuesta detenida'), findsOneWidget);
        expect(t.takeException(), isNull);
        await t.pumpAndSettle();
        final labelRect = t.getRect(find.text('Respuesta detenida'));
        final viewport = t.getRect(find.byKey(const ValueKey('chat-timeline')));
        expect(labelRect.top, greaterThanOrEqualTo(viewport.top));
        expect(labelRect.bottom, lessThanOrEqualTo(viewport.bottom - 47));
        final contrast = await measureContrast(t, {
          'estado': find.text('Respuesta detenida'),
        }, captureBoundary: find.byKey(const ValueKey('capture')));
        expect(contrast['estado'], greaterThanOrEqualTo(4.5));
        await capture('saved');
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          File(
            '../.verification/assistant-chat-real/stop-contrast-${brightness.name}-${spec.$1.width.toInt()}-${spec.$2}.json',
          ).writeAsStringSync(jsonEncode(contrast));
        }
        expect(draft(t), 'Un borrador que puedo seguir editando.');
        expect(b.sent, isEmpty);
      });
    }
  }
}
