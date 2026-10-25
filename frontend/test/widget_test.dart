import 'dart:async';
import 'dart:io';
import 'dart:ui' show PointerDeviceKind, ImageByteFormat;

import 'package:flutter/material.dart';
import 'package:gennie/src/chat_history.dart';
import 'package:gennie/src/chat_message.dart';
import 'package:gennie/src/chat_background.dart';
import 'package:flutter/services.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/main.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/glass.dart';
import 'package:gennie/src/model_controls.dart';

Snapshot snapshot({
  String selected = 'active',
  String? error,
  bool errorRecorded = false,
  int ack = 0,
  String? acknowledged,
  String? confirmation,
  bool busy = false,
  String title = 'Conversación actual',
  ModelState modelState = ModelState.unloaded,
  String? modelDetail,
  List<Message>? messages,
}) => Snapshot(
  models: [
    for (final model in LocalModel.values)
      ModelInfo(
        model: model,
        state: modelState,
        unloadMinutes: 10,
        detail: modelDetail,
      ),
  ],
  conversations: [
    Conversation(
      id: 'active',
      title: title,
      updatedAt: DateTime.now().toUtc().toIso8601String(),
      lastMessage: 'Último mensaje del asistente',
    ),
    Conversation(
      id: 'older',
      title: 'Ideas anteriores',
      updatedAt: DateTime.now()
          .subtract(const Duration(days: 8))
          .toUtc()
          .toIso8601String(),
      lastMessage: 'Última entrada del usuario',
    ),
  ],
  messages:
      messages ??
      [
        Message(
          speaker: 'assistant',
          text: 'Respuesta con **Markdown** y contenido local.',
          createdAt: '',
        ),
      ],
  selectedId: selected,
  activeId: 'active',
  busy: busy,
  connection: 'Conectado',
  reply: '',
  error: error,
  errorRecorded: errorRecorded,
  confirmation: confirmation,
  acknowledgedText: acknowledged,
  acknowledgement: ack,
  dictationStatus: 'Sin cargar',
  voiceStatus: 'Sin cargar',
  reducedMotion: false,
  assistant: AssistantSettings(
    model: 'gpt-test',
    effort: 'low',
    readFiles: true,
    writeFiles: true,
    runCommands: true,
    localActions: true,
  ),
  dictation: DictationSettings(
    enabled: true,
    shortcut: 'Ctrl+Option+Space',
    hold: true,
    unloadMinutes: 10,
    microphone: null,
  ),
  voice: VoiceSettings(
    enabled: true,
    voice: 'ef_dora',
    speedPercent: 100,
    unloadMinutes: 10,
  ),
);

// Ningún método utiliza FFI, SQLite, red, audio ni permisos del sistema.
class FakeBackend implements DesktopBackend {
  Snapshot value = snapshot();
  final events = StreamController<Snapshot>.broadcast();
  Completer<Snapshot>? pendingSend, pendingRename;
  final sent = <String>[];
  final renamed = <(String, String)>[];
  final selected = <String?>[];
  AssistantSettings? assistantSaved;
  VoiceSettings? voicePreviewed;
  bool? motionSaved;
  final modelRequests = <(LocalModel, bool)>[];
  Completer<Snapshot>? pendingModel;
  final memoryRequests = <LocalModel>[];
  Completer<int?>? pendingMemory;
  @override
  Future<int?> modelMemory(LocalModel model) async {
    memoryRequests.add(model);
    return pendingMemory?.future ?? 128 * 1024 * 1024;
  }

  @override
  Future<Snapshot> initialize() async => value;
  @override
  Stream<Snapshot> watch() => events.stream;
  @override
  Future<Snapshot> select(String? id) async {
    selected.add(id);
    return value = snapshot(selected: id!);
  }

  @override
  Future<Snapshot> send(String text) {
    sent.add(text);
    return pendingSend!.future;
  }

  @override
  Future<Snapshot> rename(String id, String title) {
    renamed.add((id, title));
    return pendingRename!.future;
  }

  @override
  Future<Snapshot> saveAssistant(AssistantSettings settings) async {
    assistantSaved = settings;
    return snapshot(confirmation: 'Preferencias guardadas');
  }

  @override
  Future<Snapshot> previewVoice(VoiceSettings settings) async {
    voicePreviewed = settings;
    return value;
  }

  @override
  Future<Snapshot> motion(bool enabled) async {
    motionSaved = enabled;
    return snapshot(confirmation: 'Preferencias guardadas');
  }

  @override
  Future<List<String>> microphones() async => ['Micrófono de prueba'];
  @override
  Future<void> shutdown() async {}
  @override
  Future<Snapshot> newChat() async => value;
  @override
  Future<Snapshot> connect() async => value;
  @override
  Future<Snapshot> delete(String id) async => value;
  @override
  Future<Snapshot> saveVoice(VoiceSettings settings) async => value;
  @override
  Future<Snapshot> saveDictation(DictationSettings settings) async => value;
  @override
  Future<Snapshot> setModelLoaded(LocalModel model, bool loaded) async {
    modelRequests.add((model, loaded));
    return pendingModel?.future ?? value;
  }

  @override
  Future<Snapshot> prepare() async => value;
  @override
  Future<Snapshot> release() async => value;
  @override
  Future<Snapshot> authorize() async => value;
}

// FRB mantiene el StreamSink hasta shutdown: cancelar no debe impedir apagar.
class ClosingBackend extends FakeBackend {
  final released = Completer<void>();
  late final retained = StreamController<Snapshot>(
    onCancel: () => released.future,
  );
  bool stopped = false;
  @override
  Stream<Snapshot> watch() => retained.stream;
  @override
  Future<void> shutdown() async {
    stopped = true;
    released.complete();
  }
}

Future<(FakeBackend, DesktopState)> mount(
  WidgetTester tester, {
  Size size = const Size(1100, 720),
  Snapshot? initialSnapshot,
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final fake = FakeBackend();
  if (initialSnapshot != null) fake.value = initialSnapshot;
  final state = DesktopState(fake);
  await state.start();
  await tester.pumpWidget(
    RepaintBoundary(
      key: const ValueKey('capture'),
      child: GennieApp(state: state),
    ),
  );
  await tester.pumpAndSettle();
  addTearDown(() async {
    await tester.pumpWidget(const SizedBox());
    state.dispose();
    await fake.events.close();
  });
  return (fake, state);
}

Finder rail(String label) =>
    find.byWidgetPredicate((w) => w is RailButton && w.label == label);
Finder appTooltip(String label) =>
    find.byWidgetPredicate((w) => w is AppTooltip && w.label == label);
Finder composer() => find.widgetWithText(TextField, 'Escribe a Gennie…');
String draft(WidgetTester tester) =>
    tester.widget<TextField>(composer()).controller!.text;

void main() {
  testWidgets(
    'Modelos al pie: estado real, solicitud pendiente y tooltip detallado',
    (tester) async {
      final (fake, state) = await mount(tester, size: const Size(640, 420));
      expect(fake.memoryRequests, isEmpty);
      final laya = rail('Laya');
      final voice = rail('Voz · Kokoro');
      final dictation = rail('Transcripción · Nemotron');
      expect(tester.getTopLeft(laya).dy, lessThan(tester.getTopLeft(voice).dy));
      expect(
        tester.getTopLeft(voice).dy,
        lessThan(tester.getTopLeft(dictation).dy),
      );
      expect(
        tester.getTopLeft(dictation).dy,
        lessThan(tester.getTopLeft(rail('Configuración')).dy),
      );
      expect(tester.widget<RailButton>(laya).icon, Icons.psychology_outlined);
      expect(
        tester.widget<RailButton>(laya).appearance,
        RailButtonAppearance.modelStatus,
      );
      expect(
        tester.widget<RailButton>(voice).appearance,
        RailButtonAppearance.modelStatus,
      );
      expect(
        tester.widget<RailButton>(dictation).appearance,
        RailButtonAppearance.modelStatus,
      );
      Color statusColor(String label) =>
          (tester
                      .widget<Container>(
                        find.byKey(ValueKey('model-status-$label')),
                      )
                      .decoration!
                  as BoxDecoration)
              .color!;
      for (final label in [
        'Laya',
        'Voz · Kokoro',
        'Transcripción · Nemotron',
      ]) {
        expect(statusColor(label), const Color(0xffe05b56));
      }
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: Offset.zero);
      await mouse.moveTo(tester.getCenter(laya));
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('tooltip-message')), findsNothing);
      expect(find.text('128 MB RAM'), findsOneWidget);
      expect(fake.memoryRequests, [LocalModel.laya]);
      final tooltipContent = find.byType(AppTooltipContent);
      final tooltipGlass = find
          .ancestor(of: tooltipContent, matching: find.byType(Glass))
          .first;
      final card = tester.getRect(tooltipGlass);
      final title = find.byKey(const ValueKey('tooltip-title'));
      final footer = find.byKey(const ValueKey('tooltip-footer'));
      expect(tester.getRect(title).top - card.top, 16);
      expect(card.bottom - tester.getRect(footer).bottom, 16);
      expect(tester.widget<Text>(title).style!.fontWeight, FontWeight.w600);
      expect(tester.widget<Text>(footer).style!.fontWeight, FontWeight.w400);
      expect(tester.widget<RailButton>(laya).asset, 'assets/icons/brain.svg');
      expect(find.text('Apagado'), findsOneWidget);
      expect(
        tester.getRect(find.byKey(const ValueKey('tooltip-status'))).left,
        greaterThan(tester.getRect(title).left),
      );
      state.accept(
        snapshot(
          modelState: ModelState.failed,
          modelDetail: 'detalle técnico oculto',
        ),
      );
      await tester.pumpAndSettle();
      expect(
        find.text(tester.widget<RailButton>(laya).details!.message!),
        findsOneWidget,
      );
      expect(find.text('Fallo'), findsOneWidget);
      expect(find.text('No se pudo cargar el modelo.'), findsOneWidget);
      expect(find.textContaining('detalle técnico'), findsNothing);
      expect(statusColor('Laya'), const Color(0xffd9a34a));
      await tester.pump(const Duration(seconds: 3));
      expect(fake.memoryRequests, [LocalModel.laya]);
      await mouse.moveTo(Offset.zero);
      await mouse.removePointer();
      state.accept(snapshot());
      await tester.pumpAndSettle();
      fake.pendingModel = Completer<Snapshot>();
      await tester.tap(laya);
      await tester.pump();
      expect(fake.modelRequests, [(LocalModel.laya, true)]);
      expect(tester.widget<RailButton>(laya).onPressed, isNull);
      fake.pendingModel!.complete(snapshot(modelState: ModelState.loading));
      await tester.pump();
      await tester.pump();
      expect(tester.widget<RailButton>(laya).busy, isTrue);
      state.accept(snapshot(modelState: ModelState.ready));
      await tester.pumpAndSettle();
      expect(tester.widget<RailButton>(laya).icon, Icons.psychology_outlined);
      expect(tester.widget<RailButton>(laya).toggled, isTrue);
      for (final label in [
        'Laya',
        'Voz · Kokoro',
        'Transcripción · Nemotron',
      ]) {
        expect(statusColor(label), const Color(0xff32b869));
      }
      expect(tester.widget<RailButton>(voice).icon, Icons.volume_up_outlined);
      expect(tester.widget<RailButton>(dictation).icon, Icons.mic_none);
      expect(tester.widget<RailButton>(dictation).details!.message, isNull);
      expect(tester.widget<RailButton>(voice).description, contains('10 min'));
      fake.pendingModel = null;
      await tester.tap(laya);
      await tester.pumpAndSettle();
      expect(fake.modelRequests.last, (LocalModel.laya, false));
      state.accept(snapshot(modelState: ModelState.disabled));
      await tester.pumpAndSettle();
      expect(tester.widget<RailButton>(voice).onPressed, isNull);
      for (final label in [
        'Laya',
        'Voz · Kokoro',
        'Transcripción · Nemotron',
      ]) {
        expect(statusColor(label), const Color(0xff858585));
      }
      expect(
        tester.widget<RailButton>(voice).description,
        contains('Desactivado'),
      );
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('RAM solo al entrar: ignora respuestas de un hover anterior', (
    tester,
  ) async {
    final (fake, _) = await mount(tester);
    expect(fake.memoryRequests, isEmpty);
    final first = Completer<int?>();
    fake.pendingMemory = first;
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(tester.getCenter(rail('Laya')));
    await tester.pumpAndSettle();
    expect(fake.memoryRequests, [LocalModel.laya]);
    expect(find.text('RAM …'), findsOneWidget);
    await mouse.moveTo(Offset.zero);
    await tester.pumpAndSettle();
    final second = Completer<int?>();
    fake.pendingMemory = second;
    await mouse.moveTo(tester.getCenter(rail('Laya')));
    await tester.pumpAndSettle();
    second.complete(64 * 1024 * 1024);
    await tester.pumpAndSettle();
    expect(find.text('64 MB RAM'), findsOneWidget);
    first.complete(900 * 1024 * 1024);
    await tester.pumpAndSettle();
    expect(find.text('64 MB RAM'), findsOneWidget);
    expect(find.text('900 MB RAM'), findsNothing);
    await tester.pump(const Duration(seconds: 3));
    expect(fake.memoryRequests, [LocalModel.laya, LocalModel.laya]);
    await mouse.moveTo(Offset.zero);
    await mouse.removePointer();
    await tester.pumpAndSettle();
    expect(find.byType(AppTooltipContent), findsNothing);
  });

  setUpAll(() async {
    final inter = FontLoader('Inter');
    for (final weight in ['Regular', 'Medium', 'SemiBold', 'Bold']) {
      inter.addFont(rootBundle.load('assets/fonts/Inter-$weight.ttf'));
    }
    await inter.load();
    final icons = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await icons.load();
  });
  for (final initiallyLoaded in [true, false]) {
    testWidgets(
      'Abrir historial largo muestra el último mensaje (inicial: $initiallyLoaded)',
      (tester) async {
        final messages = List.generate(
          80,
          (index) => Message(
            speaker: index.isEven ? 'user' : 'assistant',
            text:
                'Mensaje $index\n${List.filled(index % 7 + 1, 'Contenido de altura variable.').join('\n')}',
            createdAt: '2026-10-25T12:00:00Z',
          ),
        );
        final (_, state) = await mount(
          tester,
          initialSnapshot: initiallyLoaded
              ? snapshot(messages: messages)
              : null,
        );
        if (!initiallyLoaded) {
          state.accept(snapshot(selected: 'older', messages: messages));
          await tester.pumpAndSettle();
        }
        final last = find.byWidgetPredicate(
          (widget) =>
              widget is ChatMessage &&
              widget.message.text == messages.last.text,
        );
        expect(last.hitTestable(), findsOneWidget);
        final chatList = find.ancestor(
          of: last,
          matching: find.byType(ListView),
        );
        final controller = tester.widget<ListView>(chatList).controller!;
        expect(controller.position.extentAfter, lessThan(1));

        // Cambiar entre historiales con igual cantidad también debe ir al final.
        state.accept(snapshot(selected: 'another', messages: messages));
        await tester.pumpAndSettle();
        expect(last.hitTestable(), findsOneWidget);
        expect(controller.position.extentAfter, lessThan(1));

        final withNewMessage = [
          ...messages,
          const Message(
            speaker: 'user',
            text: 'Último mensaje enviado',
            createdAt: '',
          ),
        ];
        state.accept(snapshot(selected: 'another', messages: withNewMessage));
        await tester.pumpAndSettle();
        expect(
          find.text('Último mensaje enviado').hitTestable(),
          findsOneWidget,
        );
        expect(controller.position.extentAfter, lessThan(1));

        // Leer mensajes anteriores no debe perderse por nuevas entradas.
        controller.jumpTo(controller.position.maxScrollExtent / 2);
        await tester.pumpAndSettle();
        final readingOffset = controller.offset;
        state.accept(
          snapshot(
            selected: 'another',
            messages: [
              ...withNewMessage,
              const Message(
                speaker: 'assistant',
                text: 'Nueva respuesta',
                createdAt: '',
              ),
            ],
          ),
        );
        await tester.pumpAndSettle();
        expect(controller.offset, closeTo(readingOffset, 1));
        expect(tester.takeException(), isNull);
      },
    );
  }
  test(
    'Cierre apaga Rust antes de esperar cancelación del stream retenido',
    () async {
      final fake = ClosingBackend();
      final state = DesktopState(fake);
      await state.start();
      try {
        await state.close().timeout(const Duration(seconds: 1));
        expect(fake.stopped, isTrue);
      } finally {
        if (!fake.released.isCompleted) fake.released.complete();
        state.dispose();
        await fake.retained.close();
        await fake.events.close();
      }
    },
  );

  for (final size in [const Size(1400, 720), const Size(640, 420)]) {
    testWidgets('Título de cabecera acotado a una línea a $size', (
      tester,
    ) async {
      final longTitle = List.filled(15, 'Investigación de cohetes').join(' ');
      await mount(
        tester,
        size: size,
        initialSnapshot: snapshot(title: longTitle),
      );
      final title = find.descendant(
        of: find.byKey(const ValueKey('chat-header')),
        matching: find.text(longTitle),
      );
      final text = tester.widget<Text>(title);
      expect(text.maxLines, 1);
      expect(text.overflow, TextOverflow.ellipsis);
      expect(tester.getSize(title).width, lessThanOrEqualTo(320));
      expect(tester.getSize(title).height, lessThan(30));
      expect(tester.takeException(), isNull);
    });
  }

  for (final size in [const Size(1100, 720), const Size(640, 420)]) {
    testWidgets('Escritorio y navegación sin overflow a $size', (tester) async {
      final (fake, _) = await mount(tester, size: size);
      expect(tester.takeException(), isNull);
      expect(find.text('Conversación actual'), findsNWidgets(2));
      final avatar = find.bySemanticsLabel('Gennie, avatar de pingüino');
      expect(avatar, findsOneWidget);
      final header = tester.getRect(find.byKey(const ValueKey('chat-header')));
      final sidebarHeader = tester.getRect(
        find.byKey(const ValueKey('sidebar-header')),
      );
      expect(sidebarHeader.top, header.top);
      expect(sidebarHeader.height, 56);
      expect(sidebarHeader.bottom, header.bottom);
      expect(
        tester.getCenter(find.byIcon(Icons.add)).dy,
        closeTo(header.center.dy, .5),
      );
      expect(
        tester.getCenter(find.text('Historial de chats')).dy,
        closeTo(header.center.dy, .5),
      );
      final resizeHandle = tester.getRect(
        find.bySemanticsLabel('Ancho de la sidebar'),
      );
      expect(header.left, resizeHandle.center.dx);

      expect(find.byType(ChatBackground), findsOneWidget);
      final background = tester.getRect(find.byType(ChatBackground));
      expect(background.left, header.left);
      expect(background.top, header.bottom);
      expect(background.right, header.right);
      expect(background.bottom, lessThanOrEqualTo(size.height));

      await tester.tap(rail('Plugins'));
      await tester.pumpAndSettle();
      expect(composer(), findsNothing);
      expect(find.byType(ChatBackground), findsNothing);
      expect(avatar, findsNothing);
      expect(find.text('No hay contenido disponible'), findsWidgets);
      expect(fake.selected, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.tap(rail('Historial'));
      await tester.pumpAndSettle();
      expect(avatar, findsOneWidget);
      expect(find.byType(ChatBackground), findsOneWidget);
      await tester.tap(find.text('Ideas anteriores'));
      await tester.pumpAndSettle();
      expect(fake.selected, ['older']);
      expect(composer(), findsNothing);
      await tester.tap(find.text('Volver al chat activo'));
      await tester.pumpAndSettle();
      expect(fake.selected, ['older', 'active']);
      expect(composer(), findsOneWidget);
    });
  }

  if (const bool.fromEnvironment('CAPTURE_UI')) {
    testWidgets('Comparación visual de controles de modelos', (tester) async {
      tester.view.physicalSize = const Size(600, 256);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final states = <DesktopState>[];
      for (final _ in Brightness.values) {
        for (final modelState in [ModelState.unloaded, ModelState.ready]) {
          states.add(
            DesktopState(FakeBackend())
              ..accept(snapshot(modelState: modelState)),
          );
        }
      }
      addTearDown(() async {
        for (final state in states) {
          state.dispose();
          await (state.backend as FakeBackend).events.close();
        }
      });
      await tester.pumpWidget(
        MaterialApp(
          debugShowCheckedModeBanner: false,
          home: RepaintBoundary(
            key: const ValueKey('model-comparison'),
            child: Row(
              children: [
                for (var index = 0; index < states.length; index++)
                  Expanded(
                    child: Theme(
                      data: ThemeData(
                        brightness: index < 2
                            ? Brightness.dark
                            : Brightness.light,
                        fontFamily: 'Inter',
                        useMaterial3: true,
                      ),
                      child: Material(
                        color: index < 2
                            ? const Color(0xff45484e)
                            : const Color(0xffe5e7e9),
                        child: Column(
                          children: [
                            const SizedBox(height: 22),
                            Text(
                              index < 2 ? 'Oscuro' : 'Claro',
                              style: const TextStyle(fontSize: 12),
                            ),
                            const SizedBox(height: 5),
                            Text(
                              index.isEven ? 'Sin cargar' : 'Cargado',
                              style: const TextStyle(fontSize: 11),
                            ),
                            const SizedBox(height: 18),
                            ModelControls(state: states[index]),
                          ],
                        ),
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      final boundary = tester.renderObject<RenderRepaintBoundary>(
        find.byKey(const ValueKey('model-comparison')),
      );
      await tester.runAsync(() async {
        final image = await boundary.toImage(pixelRatio: 2);
        final bytes = await image.toByteData(format: ImageByteFormat.png);
        final output = Directory('../.verification/flutter-ui');
        await output.create(recursive: true);
        await File(
          '${output.path}/model-controls-comparison.png',
        ).writeAsBytes(bytes!.buffer.asUint8List());
        image.dispose();
      });
    });
    for (final brightness in Brightness.values) {
      testWidgets('Captura software ${brightness.name}', (tester) async {
        tester.platformDispatcher.platformBrightnessTestValue = brightness;
        addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);
        await mount(tester);
        final boundary = tester.renderObject<RenderRepaintBoundary>(
          find.byKey(const ValueKey('capture')),
        );
        await tester.runAsync(() async {
          final image = await boundary.toImage(pixelRatio: 1);
          final bytes = await image.toByteData(format: ImageByteFormat.png);
          final output = Directory('../.verification/flutter-ui');
          await output.create(recursive: true);
          await File(
            '${output.path}/desktop-${brightness.name}.png',
          ).writeAsBytes(bytes!.buffer.asUint8List());
          image.dispose();
        });
      });
    }
  }

  for (final brightness in Brightness.values) {
    testWidgets(
      'Chat: separadores por día y menú del sistema ${brightness.name}',
      (tester) async {
        tester.platformDispatcher.platformBrightnessTestValue = brightness;
        addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);
        final (_, state) = await mount(tester);
        final now = DateTime.now();
        final today = DateTime(now.year, now.month, now.day, 9, 5);
        final yesterday = DateTime(now.year, now.month, now.day - 1, 18, 30);
        state.accept(
          snapshot(
            messages: [
              Message(
                speaker: 'user',
                text: 'Quiero organizar las ideas de esta semana.',
                createdAt: yesterday.toUtc().toIso8601String(),
              ),
              Message(
                speaker: 'assistant',
                text:
                    'Podemos empezar por las **tres prioridades** y dejar espacio para lo pendiente.',
                createdAt: yesterday
                    .add(const Duration(minutes: 1))
                    .toUtc()
                    .toIso8601String(),
              ),
              Message(
                speaker: 'user',
                text: 'Perfecto. Hoy empezamos con el diseño del chat.',
                createdAt: today.toUtc().toIso8601String(),
              ),
            ],
          ),
        );
        await tester.pumpAndSettle();
        expect(find.byType(ChatDayBadge), findsNWidgets(2));
        expect(
          find.descendant(
            of: find.byType(ChatDayBadge),
            matching: find.text('Ayer'),
          ),
          findsOneWidget,
        );
        expect(
          find.descendant(
            of: find.byType(ChatDayBadge),
            matching: find.text('Hoy'),
          ),
          findsOneWidget,
        );
        expect(find.text('09:05'), findsOneWidget);
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          final boundary = tester.renderObject<RenderRepaintBoundary>(
            find.byKey(const ValueKey('capture')),
          );
          await tester.runAsync(() async {
            final image = await boundary.toImage(pixelRatio: 1);
            final bytes = await image.toByteData(format: ImageByteFormat.png);
            final output = Directory('../.verification/flutter-ui');
            await output.create(recursive: true);
            await File(
              '${output.path}/chat-${brightness.name}.png',
            ).writeAsBytes(bytes!.buffer.asUint8List());
            image.dispose();
          });
        }
        await tester.tap(find.byIcon(Icons.more_horiz));
        await tester.pumpAndSettle();
        expect(find.text('Conectado'), findsOneWidget);
        await tester.tap(find.text('Configuración'));
        await tester.pumpAndSettle();
        expect(find.text('Asistente'), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );
  }

  testWidgets(
    'Búsqueda transparente filtra título y vista previa sin seleccionar',
    (tester) async {
      final (fake, _) = await mount(tester, size: const Size(640, 420));
      final search = find.byKey(const ValueKey('history-search'));
      expect(tester.widget<TextField>(search).decoration!.filled, isFalse);
      expect(tester.getSize(search).height, 40);
      expect(find.byIcon(Icons.backspace_outlined), findsNothing);
      final header = tester.getRect(
        find.byKey(const ValueKey('sidebar-header')),
      );
      expect(tester.getRect(search).top, header.bottom + 12);
      await tester.enterText(search, '  IDEAS  ');
      await tester.pumpAndSettle();
      expect(find.byType(ChatHistoryTile), findsOneWidget);
      expect(find.text('Ideas anteriores'), findsOneWidget);
      await tester.enterText(search, 'asistente');
      await tester.pumpAndSettle();
      expect(find.byType(ChatHistoryTile), findsOneWidget);
      expect(find.text('Ideas anteriores'), findsNothing);
      await tester.enterText(search, 'sin coincidencias');
      await tester.pumpAndSettle();
      expect(find.text('No se encontraron chats'), findsOneWidget);
      expect(find.byType(ChatHistoryTile), findsNothing);
      await tester.tap(find.byIcon(Icons.backspace_outlined));
      await tester.pumpAndSettle();
      expect(tester.widget<TextField>(search).controller!.text, isEmpty);
      expect(find.byIcon(Icons.backspace_outlined), findsNothing);
      expect(find.byType(ChatHistoryTile), findsNWidgets(2));
      expect(fake.selected, isEmpty);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'Los errores aparecen como mensajes de sistema sin banner duplicado',
    (tester) async {
      final (_, state) = await mount(tester);
      const error = 'El proveedor rechazó la operación (código -32600)';
      state.accept(
        snapshot(
          error: error,
          errorRecorded: true,
          messages: [
            Message(
              speaker: 'system',
              text: 'Error: $error',
              createdAt: DateTime.now().toUtc().toIso8601String(),
            ),
          ],
        ),
      );
      await tester.pumpAndSettle();
      expect(find.byType(ChatMessage), findsOneWidget);
      expect(find.text('Sistema'), findsOneWidget);
      expect(find.text('Error: $error'), findsOneWidget);
      expect(find.text(error), findsNothing);
      state.accept(snapshot(error: 'SQLite no disponible'));
      await tester.pumpAndSettle();
      expect(find.text('Sistema · sin guardar'), findsOneWidget);
      expect(find.text('Error: SQLite no disponible'), findsOneWidget);
      expect(find.byType(ChatMessage), findsNWidgets(2));
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('Tooltips inmediatos usan Glass y desaparecen al salir', (
    tester,
  ) async {
    await mount(tester);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    for (final label in [
      'Historial',
      'Plugins',
      'Configuración',
      'Nuevo chat',
      'Enviar mensaje',
    ]) {
      await mouse.moveTo(tester.getCenter(appTooltip(label)));
      await tester.pump();
      expect(find.text(label), findsOneWidget);
      expect(
        find.ancestor(of: find.text(label), matching: find.byType(Glass)),
        findsOneWidget,
      );
      await mouse.moveTo(const Offset(1090, 10));
      await tester.pump();
      expect(find.text(label), findsNothing);
    }
    await mouse.removePointer();
  });

  testWidgets('macOS presenta el tooltip en el compositor nativo y lo oculta', (
    tester,
  ) async {
    const channel = MethodChannel('gennie/tooltip');
    final calls = <MethodCall>[];
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(channel, (
      call,
    ) async {
      calls.add(call);
      return null;
    });
    addTearDown(
      () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        channel,
        null,
      ),
    );
    final (_, state) = await mount(tester);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    for (final label in ['Historial', 'Nuevo chat', 'Enviar mensaje']) {
      await mouse.moveTo(tester.getCenter(appTooltip(label).first));
      await tester.pump();
      expect(calls.last.method, 'show');
      expect(calls.last.arguments['label'], label);
      expect(calls.last.arguments['preferRight'], label == 'Historial');
      expect(find.text(label), findsNothing);
      await mouse.moveTo(const Offset(1090, 10));
      await tester.pump();
      expect(calls.last.method, 'hide');
    }
    await mouse.moveTo(tester.getCenter(rail('Laya')));
    await tester.pump();
    final details = calls.last.arguments['details'] as Map;
    expect(details['title'], 'Laya');
    expect(details['status'], 'Apagado');
    expect(details['statusColor'], 0xffe05b56);
    expect(details.containsKey('description'), isFalse);
    expect(details['memory'], isNotEmpty);
    expect(details['message'], isNull);
    expect(details['footer'], contains('10 min'));
    state.accept(snapshot(modelState: ModelState.ready));
    await tester.pump();
    await tester.pump();
    expect(calls.last.arguments['details']['status'], 'Encendido');
    expect(calls.last.arguments['details']['statusColor'], 0xff32b869);
    await mouse.removePointer();
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets('Envío conserva borrador hasta ack, error y edición posterior', (
    tester,
  ) async {
    final (fake, state) = await mount(tester);
    fake.pendingSend = Completer<Snapshot>();
    await tester.enterText(composer(), 'Borrador original');
    await tester.tap(appTooltip('Enviar mensaje'));
    await tester.pump();
    expect(fake.sent, ['Borrador original']);
    expect(draft(tester), 'Borrador original');
    fake.pendingSend!.completeError(StateError('Guardado rechazado'));
    await tester.pumpAndSettle();
    expect(draft(tester), 'Borrador original');
    expect(find.textContaining('Guardado rechazado'), findsOneWidget);
    fake.pendingSend = Completer<Snapshot>();
    await tester.tap(appTooltip('Enviar mensaje'));
    await tester.pump();
    await tester.enterText(composer(), 'Nueva edición');
    fake.pendingSend!.complete(
      snapshot(ack: 1, acknowledged: 'Borrador original'),
    );
    await tester.pumpAndSettle();
    expect(draft(tester), 'Nueva edición');
    state.accept(snapshot(ack: 2, acknowledged: 'Nueva edición'));
    await tester.pump();
    expect(draft(tester), isEmpty);
  });

  testWidgets('Intro ocupado conserva borrador sin enviar otra solicitud', (
    tester,
  ) async {
    final (fake, state) = await mount(tester);
    fake.pendingSend = Completer<Snapshot>();
    state.accept(snapshot(busy: true));
    await tester.pump();
    await tester.enterText(composer(), 'Esperar respuesta');
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(fake.sent, isEmpty);
    expect(draft(tester), 'Esperar respuesta');
  });

  testWidgets('Renombrar no selecciona fila y conserva borrador ante fallo', (
    tester,
  ) async {
    final (fake, _) = await mount(tester);
    fake.pendingRename = Completer<Snapshot>();
    await tester.tap(find.byType(ChatHistoryTile).last, buttons: 2);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Cambiar nombre'));
    await tester.pumpAndSettle();
    final field = find.widgetWithText(TextField, 'Nombre del chat');
    await tester.enterText(field, 'Nombre editado');
    await tester.tap(find.text('Guardar'));
    await tester.pump();
    expect(fake.selected, isEmpty);
    expect(fake.renamed, [('older', 'Nombre editado')]);
    expect(tester.widget<TextField>(field).enabled, isFalse);
    fake.pendingRename!.completeError(StateError('No se pudo guardar'));
    await tester.pumpAndSettle();
    expect(tester.widget<TextField>(field).controller!.text, 'Nombre editado');
    expect(find.textContaining('No se pudo guardar'), findsWidgets);
    await tester.tap(find.text('Cancelar'));
    await tester.pumpAndSettle();
    expect(field, findsNothing);
  });

  testWidgets('Ajustes permanecen utilizables en ventana mínima', (
    tester,
  ) async {
    await mount(tester, size: const Size(640, 420));
    await tester.tap(rail('Configuración'));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    await tester.tap(find.text('General'));
    await tester.pumpAndSettle();
    expect(find.text('Guardar cambios').hitTestable(), findsOneWidget);
    expect(appTooltip('Cerrar configuración').hitTestable(), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Ajustes guardan payload y confirman, vista previa no guarda', (
    tester,
  ) async {
    final (fake, _) = await mount(tester);
    await tester.tap(rail('Configuración'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.widgetWithText(TextField, 'Modelo'),
      'modelo-editado',
    );
    await tester.tap(find.text('Guardar cambios'));
    await tester.pumpAndSettle();
    expect(fake.assistantSaved!.model, 'modelo-editado');
    expect(find.text('Preferencias guardadas'), findsWidgets);
    await tester.tap(find.text('Voz de salida'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Probar voz sin guardar'));
    await tester.tap(find.text('Probar voz sin guardar'));
    await tester.pumpAndSettle();
    expect(fake.voicePreviewed!.voice, 'ef_dora');
    await tester.tap(find.text('General'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Reducir movimiento'));
    await tester.tap(find.text('Guardar cambios'));
    await tester.pumpAndSettle();
    expect(fake.motionSaved, isTrue);
    expect(tester.takeException(), isNull);
  });
}
