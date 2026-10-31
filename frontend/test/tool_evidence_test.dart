import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/tool_evidence.dart';
import 'package:gennie/src/chat_message.dart';
import 'widget_test.dart' show FakeBackend, snapshot, mount;
import 'chats_preview_test.dart' show loadFonts;

const _captureDirectory = String.fromEnvironment(
  'UI_CAPTURE_DIR',
  defaultValue: '../.verification/assistant-chat-real',
);

ToolCall call(
  String id,
  ToolCallOutcome outcome, {
  String name = 'Correo',
  String server = 'gennie',
  String tool = 'browser_plugin_run',
  String? plugin = 'mail',
  String? detail,
}) => ToolCall(
  callId: id,
  detail: detail,
  server: server,
  tool: tool,
  pluginId: plugin,
  displayName: name,
  functionName: 'list',
  operation: 'run',
  outcome: outcome,
  createdAt: '2026-10-28T00:00:00.000Z',
  updatedAt: '2026-10-28T00:00:01.000Z',
);

class EvidenceBackend extends FakeBackend {
  Map<String, List<ToolCall>> evidence = {};
  @override
  Future<ChatPage> loadChatPage(ChatCursor? before) async {
    final page = await super.loadChatPage(before);
    return ChatPage(
      before: page.before,
      messages: [
        for (final entry in page.messages)
          ChatEntry(
            id: entry.id,
            conversationId: entry.conversationId,
            sessionStartedAt: entry.sessionStartedAt,
            startsSession: entry.startsSession,
            message: entry.message,
            quote: entry.quote,
            tools: evidence[entry.id] ?? const [],
          ),
      ],
    );
  }
}

final messages = [
  const Message(
    speaker: 'user',
    text: 'Consulta el correo',
    createdAt: '2026-10-28T00:00:00.000Z',
  ),
  const Message(
    speaker: 'assistant',
    text: 'Hay información disponible.',
    createdAt: '2026-10-28T00:00:01.000Z',
  ),
];
Future<void> capture(WidgetTester tester, String name) async {
  if (!const bool.fromEnvironment('CAPTURE_UI')) return;
  final boundary = tester.renderObject<RenderRepaintBoundary>(
    find.byKey(const ValueKey('capture')),
  );
  await tester.runAsync(() async {
    final image = await boundary.toImage();
    final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
    final out = Directory(_captureDirectory)..createSync(recursive: true);
    File('${out.path}/$name.png').writeAsBytesSync(bytes!.buffer.asUint8List());
    image.dispose();
  });
}

Future<Map<String, double>> measureContrast(
  WidgetTester tester,
  Map<String, Finder> targets, {
  Finder? captureBoundary,
  Map<String, Finder> backgroundMaterials = const {},
}) async {
  final boundary = tester.renderObject<RenderRepaintBoundary>(
    captureBoundary ?? find.byKey(const ValueKey('capture')),
  );
  final ratios = <String, double>{};
  await tester.runAsync(() async {
    final image = await boundary.toImage();
    try {
      final rgba = (await image.toByteData(
        format: ui.ImageByteFormat.rawRgba,
      ))!;
      for (final target in targets.entries) {
        final rect = tester.getRect(target.value);
        final style = switch (tester.widget(target.value)) {
          Text(:final style) => style,
          SelectableText(:final style) => style,
          EditableText(:final style) => style,
          _ => throw StateError('El contraste requiere un texto visible'),
        };
        final foreground =
            style?.color ??
            DefaultTextStyle.of(tester.element(target.value)).style.color!;
        // Fondo rasterizado junto al texto; evita píxeles suavizados de letras.
        var sample = Offset(rect.left + 2, rect.top - 2);
        final surface = backgroundMaterials[target.key];
        if (surface != null) {
          final control = tester.getRect(surface);
          // En macOS la caja tipográfica puede rebasar el botón compacto.
          // Medir dentro de su Material, fuera de las letras y del borde curvo.
          sample = Offset(control.left + 4, control.center.dy);
          expect(control.contains(sample), isTrue);
          expect(rect.contains(sample), isFalse);
        }
        final point = boundary.globalToLocal(sample);
        final offset = (point.dy.round() * image.width + point.dx.round()) * 4;
        final background = Color.fromARGB(
          255,
          rgba.getUint8(offset),
          rgba.getUint8(offset + 1),
          rgba.getUint8(offset + 2),
        );
        final a = Color.alphaBlend(foreground, background).computeLuminance();
        final b = background.computeLuminance();
        final ratio = (a > b ? a + .05 : b + .05) / (a > b ? b + .05 : a + .05);
        ratios[target.key] = ratio;
        expect(ratio, greaterThanOrEqualTo(4.5), reason: target.key);
      }
    } finally {
      image.dispose();
    }
  });
  return ratios;
}

void main() {
  testWidgets(
    'el detalle devuelve el foco al enlace dentro de las acciones del mensaje',
    (tester) async {
      final messageFocus = FocusNode();
      addTearDown(messageFocus.dispose);
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: ChatMessage(
              message: messages.last,
              focusNode: messageFocus,
              onReply: () {},
              tools: [call('one', ToolCallOutcome.unknown)],
            ),
          ),
        ),
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      messageFocus.requestFocus();
      await tester.pumpAndSettle();
      final link = find.textContaining('Correo · sin confirmar');
      final linkFocus = Focus.of(tester.element(link));
      linkFocus.requestFocus();
      await tester.pumpAndSettle();
      expect(linkFocus.hasPrimaryFocus, isTrue);
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await tester.pumpAndSettle();
      expect(find.text('Uso de Correo'), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.text('Uso de Correo'), findsNothing);
      expect(linkFocus.hasPrimaryFocus, isTrue);
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await tester.pumpAndSettle();
      expect(find.text('Uso de Correo'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );
  testWidgets(
    'el detalle de evidencia se abre por teclado y Escape restituye el enlace',
    (tester) async {
      final opened = <String>[];
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: ToolEvidence(
              calls: [call('one', ToolCallOutcome.unknown)],
              onOpenPlugin: opened.add,
            ),
          ),
        ),
      );
      final label = find.textContaining('Correo · sin confirmar');
      final focus = Focus.of(tester.element(label));
      focus.requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await tester.pumpAndSettle();
      expect(find.text('Uso de Correo'), findsOneWidget);
      expect(
        find.text('No se recibió un resultado confirmado para esta llamada.'),
        findsOneWidget,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.text('Uso de Correo'), findsNothing);
      expect(focus.hasPrimaryFocus, isTrue);
      expect(opened, isEmpty);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );

  testWidgets('mencionar un plugin en texto no inventa evidencia de su uso', (
    tester,
  ) async {
    await mount(
      tester,
      initialSnapshot: snapshot(
        messages: const [
          Message(
            speaker: 'assistant',
            text: 'Usé Correo para consultar.',
            createdAt: '',
          ),
        ],
      ),
    );
    expect(find.text('Usé Correo para consultar.'), findsOneWidget);
    expect(find.byType(ToolEvidence), findsNothing);
  });
  testWidgets(
    'evidencia pasa de solicitud a respuesta sin activar otra sesión',
    (tester) async {
      final fake = EvidenceBackend()
        ..value = snapshot(messages: messages)
        ..evidence = {
          '1': [call('one', ToolCallOutcome.unknown)],
        };
      final (_, state) = await mount(tester, backend: fake);
      expect(find.text('Actividad del asistente'), findsOneWidget);
      fake.evidence = {
        '2': [call('one', ToolCallOutcome.completed)],
      };
      state.accept(fake.value = snapshot(messages: [...messages]));
      await tester.pumpAndSettle();
      expect(find.text('Actividad del asistente'), findsNothing);
      expect(
        find.descendant(
          of: find.byKey(const ValueKey('2')),
          matching: find.text('Usó: Correo'),
        ),
        findsOneWidget,
      );
      expect(fake.selected, isEmpty);
      expect(fake.sent, isEmpty);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets('estados distintos se leen sin presentar intentos como éxito', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ToolEvidence(
            onRequest: true,
            calls: [
              call(
                'a',
                ToolCallOutcome.completed,
                name: 'Lectura',
                plugin: 'read',
              ),
              call(
                'b',
                ToolCallOutcome.failed,
                name: 'Fallida',
                plugin: 'failed',
              ),
              call(
                'c',
                ToolCallOutcome.unknown,
                name: 'Incierta',
                plugin: 'unknown',
              ),
              call(
                'd',
                ToolCallOutcome.running,
                name: 'En proceso',
                plugin: 'running',
              ),
            ],
          ),
        ),
      ),
    );
    expect(find.text('Usó: Lectura'), findsOneWidget);
    expect(find.text('Intentó: Fallida · falló'), findsOneWidget);
    expect(find.text('Incierta · sin confirmar'), findsOneWidget);
    expect(find.text('Usando: En proceso'), findsOneWidget);
    expect(find.text('Actividad del asistente'), findsOneWidget);
  });
  testWidgets(
    'teclado abre detalles y el enlace usa solo el plugin local identificado',
    (tester) async {
      final opened = <String>[];
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 400,
              child: ToolEvidence(
                calls: [
                  call('one', ToolCallOutcome.completed),
                  call('two', ToolCallOutcome.failed),
                  call('three', ToolCallOutcome.unknown),
                ],
                onOpenPlugin: opened.add,
              ),
            ),
          ),
        ),
      );
      final label = find.text('Usó: Correo · 1 fallo · 1 sin confirmar');
      Focus.of(tester.element(label)).requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(find.text('Uso de Correo'), findsOneWidget);
      expect(find.text('Finalizada'), findsOneWidget);
      expect(find.text('Falló'), findsOneWidget);
      expect(find.text('Sin confirmar'), findsOneWidget);
      expect(find.text('Función: list'), findsNWidgets(3));
      await tester.tap(find.text('Abrir plugin'));
      await tester.pumpAndSettle();
      expect(opened, ['mail']);
      expect(find.byType(Dialog), findsNothing);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets('un MCP externo no abre por coincidencia un plugin web local', (
    tester,
  ) async {
    final opened = <String>[];
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ToolEvidence(
            calls: [
              call(
                'one',
                ToolCallOutcome.completed,
                server: 'external',
                tool: 'list_messages',
              ),
            ],
            onOpenPlugin: opened.add,
          ),
        ),
      ),
    );
    await tester.tap(find.text('Usó: Correo'));
    await tester.pumpAndSettle();
    expect(find.text('Servidor: external'), findsOneWidget);
    expect(find.text('Abrir plugin'), findsNothing);
    await tester.tap(find.text('Listo'));
    await tester.pumpAndSettle();
    expect(opened, isEmpty);
  });
  testWidgets(
    'evidencia MCP abre su fuente y conserva la identidad del servidor',
    (tester) async {
      final opened = <String>[];
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: ToolEvidence(
              calls: [
                call(
                  'remote',
                  ToolCallOutcome.completed,
                  server: 'mcp:notes',
                  tool: 'echo',
                  plugin: null,
                ),
              ],
              onOpenPlugin: opened.add,
            ),
          ),
        ),
      );
      await tester.tap(find.text('Usó: Correo'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Abrir plugin'));
      await tester.pumpAndSettle();
      expect(opened, ['mcp:notes']);
      expect(find.byType(Dialog), findsNothing);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets('un intento pendiente indica revisión y permite abrir su fuente', (
    tester,
  ) async {
    final opened = <String>[];
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ToolEvidence(
            calls: [
              call(
                'pending',
                ToolCallOutcome.unknown,
                server: 'mcp:notes',
                tool: 'echo',
                plugin: null,
                detail: 'result_uncertain',
              ),
            ],
            onOpenPlugin: opened.add,
          ),
        ),
      ),
    );
    await tester.tap(find.text('Correo · sin confirmar'));
    await tester.pumpAndSettle();
    expect(
      find.text(
        'Este intento requirió revisión; consulta su estado en Plugins antes de repetir.',
      ),
      findsOneWidget,
    );
    await tester.tap(find.text('Abrir plugin'));
    await tester.pumpAndSettle();
    expect(opened, ['mcp:notes']);
  });
  for (final brightness in Brightness.values) {
    for (final (size, scale) in [
      (const Size(1100, 720), 1.0),
      (const Size(640, 420), 1.0),
      (const Size(640, 420), 1.5),
    ]) {
      testWidgets(
        'uso de plugins ${brightness.name} ${size.width} escala $scale',
        (tester) async {
          tester.platformDispatcher.platformBrightnessTestValue = brightness;
          tester.platformDispatcher.textScaleFactorTestValue = scale;
          addTearDown(
            tester.platformDispatcher.clearPlatformBrightnessTestValue,
          );
          addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
          await loadFonts();
          final name = scale > 1
              ? 'Correo de nombre muy largo '.padRight(200, 'á')
              : 'Correo';
          final fake = EvidenceBackend()
            ..value = snapshot(messages: messages)
            ..evidence = {
              '2': [
                call('one', ToolCallOutcome.completed, name: name),
                call('two', ToolCallOutcome.failed, name: name),
                call('three', ToolCallOutcome.unknown, name: name),
                call(
                  'calendar',
                  ToolCallOutcome.completed,
                  name: 'Calendario',
                  server: 'calendar',
                  tool: 'events',
                  plugin: null,
                ),
              ],
            };
          await mount(tester, backend: fake, size: size);
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          await capture(
            tester,
            'tool-usage-${brightness.name}-${size.width.toInt()}-$scale',
          );
          final button = find.textContaining('Usó: $name').first;
          await tester.ensureVisible(button);
          await tester.pumpAndSettle();
          final contrasts = await measureContrast(tester, {'enlace': button});
          await tester.tap(button);
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          expect(find.text('Listo').hitTestable(), findsOneWidget);
          contrasts.addAll(
            await measureContrast(tester, {
              'título': find.text('Uso de $name'),
              'Abrir plugin': find.text('Abrir plugin'),
              'Listo': find.text('Listo'),
            }),
          );
          if (const bool.fromEnvironment('CAPTURE_UI')) {
            File(
              '$_captureDirectory/tool-contrast-${brightness.name}-${size.width.toInt()}-$scale.json',
            ).writeAsStringSync(jsonEncode(contrasts));
          }
          await capture(
            tester,
            'tool-details-${brightness.name}-${size.width.toInt()}-$scale',
          );
          await tester.tap(find.text('Listo'));
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          expect(fake.selected, isEmpty);
          expect(fake.sent, isEmpty);
        },
        variant: TargetPlatformVariant.only(TargetPlatform.macOS),
      );
    }
  }
}
