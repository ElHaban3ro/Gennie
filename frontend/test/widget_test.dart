import 'dart:async';
import 'dart:io';
import 'dart:ui' show PointerDeviceKind, ImageByteFormat;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/main.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/glass.dart';

Snapshot snapshot({
  String selected = 'active',
  String? error,
  int ack = 0,
  String? acknowledged,
  String? confirmation,
  bool busy = false,
}) => Snapshot(
  conversations: [
    Conversation(id: 'active', title: 'Conversación actual', updatedAt: ''),
    Conversation(id: 'older', title: 'Ideas anteriores', updatedAt: ''),
  ],
  messages: [
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
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final fake = FakeBackend();
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
Finder composer() => find.widgetWithText(TextField, 'Escribe a Gennie…');
String draft(WidgetTester tester) =>
    tester.widget<TextField>(composer()).controller!.text;

void main() {
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

  for (final size in [const Size(1100, 720), const Size(640, 420)]) {
    testWidgets('Escritorio y navegación sin overflow a $size', (tester) async {
      final (fake, _) = await mount(tester, size: size);
      expect(tester.takeException(), isNull);
      expect(find.text('Conversación actual'), findsNWidgets(2));
      await tester.tap(rail('Plugins'));
      await tester.pumpAndSettle();
      expect(composer(), findsNothing);
      expect(find.text('No hay contenido disponible'), findsWidgets);
      expect(fake.selected, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.tap(rail('Historial'));
      await tester.pumpAndSettle();
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

  testWidgets('Tooltips inmediatos usan Glass y desaparecen al salir', (
    tester,
  ) async {
    await mount(tester);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    for (final label in ['Historial', 'Plugins', 'Configuración']) {
      await mouse.moveTo(tester.getCenter(rail(label)));
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

  testWidgets('Envío conserva borrador hasta ack, error y edición posterior', (
    tester,
  ) async {
    final (fake, state) = await mount(tester);
    fake.pendingSend = Completer<Snapshot>();
    await tester.enterText(composer(), 'Borrador original');
    await tester.tap(find.byTooltip('Enviar mensaje'));
    await tester.pump();
    expect(fake.sent, ['Borrador original']);
    expect(draft(tester), 'Borrador original');
    fake.pendingSend!.completeError(StateError('Guardado rechazado'));
    await tester.pumpAndSettle();
    expect(draft(tester), 'Borrador original');
    expect(find.textContaining('Guardado rechazado'), findsOneWidget);
    fake.pendingSend = Completer<Snapshot>();
    await tester.tap(find.byTooltip('Enviar mensaje'));
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
    await tester.tap(find.byTooltip('Opciones del chat').last);
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
    expect(
      find.byTooltip('Cerrar configuración').hitTestable(),
      findsOneWidget,
    );
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
