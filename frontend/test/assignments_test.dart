import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui' show ImageByteFormat;
import 'package:flutter/services.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/assignments.dart';
import 'package:gennie/src/assignment_logo.dart';
import 'package:gennie/src/sidebar.dart';
import 'widget_test.dart' show FakeBackend, snapshot, mount, rail;

class AssignmentBackend extends FakeBackend {
  Completer<Snapshot>? pending;
  final saved = <(String?, int?, String, Map<String, dynamic>)>[];
  @override
  Future<Snapshot> saveAssignment(
    String? id,
    int? revision,
    String requestKey,
    String document,
  ) {
    saved.add((
      id,
      revision,
      requestKey,
      jsonDecode(document) as Map<String, dynamic>,
    ));
    return pending?.future ?? Future.value(value);
  }
}

Finder field(String label) => find.byWidgetPredicate(
  (w) => w is TextField && w.decoration?.labelText == label,
);
Future<void> show(
  WidgetTester tester,
  DesktopState state, {
  Size size = const Size(1100, 720),
  Brightness brightness = Brightness.light,
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  await state.start();
  await tester.pumpWidget(
    MaterialApp(
      theme: ThemeData(brightness: brightness),
      home: Scaffold(
        body: ListenableBuilder(
          listenable: state,
          builder: (context, _) => RepaintBoundary(
            key: const ValueKey('editor-capture'),
            child: AssignmentsView(state: state),
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
  addTearDown(() async {
    await tester.pumpWidget(const SizedBox());
    state.dispose();
    await (state.backend as AssignmentBackend).events.close();
  });
}

Future<void> revealVisible(WidgetTester tester, Finder finder) async {
  FocusManager.instance.primaryFocus?.unfocus();
  await tester.pumpAndSettle();
  await tester.ensureVisible(finder);
  await tester.pumpAndSettle();
}

Future<void> openEditorSection(WidgetTester tester, String id) async {
  final target = find.byKey(ValueKey('assignment-editor-$id'));
  await revealVisible(tester, target);
  await tester.pumpAndSettle();
  await tester.tap(target);
  await tester.pumpAndSettle();
}

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
  if (const bool.fromEnvironment('CAPTURE_UI')) {
    for (final brightness in Brightness.values) {
      testWidgets('Captura Encargos ${brightness.name}', (tester) async {
        AssignmentLogo.clearMemoryCache();
        addTearDown(AssignmentLogo.clearMemoryCache);
        tester.platformDispatcher.platformBrightnessTestValue = brightness;
        addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);
        final logo = await tester.runAsync(
          () => File(
            '../.verification/event-logos/apple-notes.png',
          ).readAsBytes(),
        );
        const logoChannel = MethodChannel('gennie/assignment-logo');
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(logoChannel, (_) async => logo);
        addTearDown(
          () => TestDefaultBinaryMessengerBinding
              .instance
              .defaultBinaryMessenger
              .setMockMethodCallHandler(logoChannel, null),
        );
        final document = jsonEncode({
          'title': 'Revisar tareas de Notas de Apple',
          'description':
              'Preparar un resumen de las tareas que requieren mi atención.',
          'kind': 'recurring',
          'priority': 'high',
          'status': 'active',
          'timezone': 'America/Bogota',
          'reason': 'Planificar la mañana antes de empezar a trabajar.',
          'recurrence': {
            'frequency': 'daily',
            'local_time': '07:30',
            'weekdays': [],
          },
          'preparation': {
            'interval_minutes': 10,
            'rationale': 'Reunir los cambios antes del resumen diario.',
            'steps': [
              'Reunir tareas de hoy y sin fecha',
              'Observar cierres y reaperturas',
            ],
          },
          'follow_up': {
            'scope':
                'Tareas de hoy, sin fecha y vencidas abiertas. Conservar cierres, reaperturas y cambios aunque no justifiquen un aviso.',
            'signals': [
              '¿Ha cambiado algo que afecte al plan del día?',
              '¿Podemos ayudar a resolver un bloqueo?',
            ],
            'decision_guidance':
                'Valorar cambios, contexto e historial. Preguntar o proponer ayuda cuando aporte valor; registrar en silencio el resto. Adaptar la anticipación a cada situación y respetar respuestas y aplazamientos.',
            'action_scope': null,
          },
          'applications': [
            {
              'name': 'Notas de Apple',
              'kind': 'desktop',
              'url': null,
              'plugin_available': false,
              'manual_required': true,
            },
          ],
        });
        await mount(
          tester,
          initialSnapshot: snapshot(
            plans: [
              AssignmentRecord(
                id: '1',
                revision: 1,
                document: document,
                createdAt: '2026-10-25T13:00:00Z',
                updatedAt: '2026-10-25T13:00:00Z',
              ),
            ],
          ),
        );
        await tester.tap(rail('Encargos'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Revisar tareas de Notas de Apple'));
        await tester.pumpAndSettle();
        // El canal y el codec PNG completan trabajo real fuera del reloj de widgets.
        await tester.runAsync(() => Future<void>.delayed(Duration.zero));
        await tester.pumpAndSettle();
        final avatar = find.descendant(
          of: find.byType(AssignmentLogo),
          matching: find.byType(Image),
        );
        expect(avatar, findsNWidgets(2));
        await tester.runAsync(
          () => precacheImage(
            tester.widget<Image>(avatar.first).image,
            tester.element(find.byType(AssignmentLogo).first),
          ),
        );
        await tester.pumpAndSettle();
        expect(
          find.descendant(
            of: find.byKey(const ValueKey('assignment-delivery')),
            matching: find.text('Criterio para avisarte entre entregas'),
          ),
          findsOneWidget,
        );
        expect(
          find.descendant(
            of: find.byKey(const ValueKey('assignment-delivery')),
            matching: find.byWidgetPredicate(
              (w) =>
                  w is SelectableText &&
                  w.data ==
                      'Valorar cambios, contexto e historial. Preguntar o proponer ayuda cuando aporte valor; registrar en silencio el resto. Adaptar la anticipación a cada situación y respetar respuestas y aplazamientos.',
            ),
          ),
          findsOneWidget,
        );
        final editTop = tester
            .getTopLeft(find.byKey(const ValueKey('assignment-edit')))
            .dy;
        final labelTop = tester
            .getTopLeft(find.byKey(const ValueKey('assignment-profile-label')))
            .dy;
        expect(editTop, labelTop);
        final boundary = tester.renderObject<RenderRepaintBoundary>(
          find.byKey(const ValueKey('capture')),
        );
        await tester.runAsync(() async {
          final image = await boundary.toImage(pixelRatio: 1);
          final bytes = await image.toByteData(format: ImageByteFormat.png);
          final output = Directory('../.verification/flutter-ui');
          await output.create(recursive: true);
          await File(
            '${output.path}/events-${brightness.name}.png',
          ).writeAsBytes(bytes!.buffer.asUint8List());
          image.dispose();
        });
        expect(
          tester.getTopLeft(find.text('Para qué')).dy,
          lessThan(
            tester
                .getTopLeft(find.byKey(const ValueKey('assignment-delivery')))
                .dy,
          ),
        );
        final profileScroll = find
            .descendant(
              of: find.byType(AssignmentProfile),
              matching: find.byType(Scrollable),
            )
            .first;
        await tester.scrollUntilVisible(
          find.text('Detalles del encargo'),
          150,
          scrollable: profileScroll,
        );
        await tester.tap(find.text('Detalles del encargo'));
        await tester.pumpAndSettle();
        await tester.scrollUntilVisible(
          find.text('Aplicaciones'),
          150,
          scrollable: profileScroll,
        );
        await revealVisible(tester, find.text('Aplicaciones'));
        await tester.pumpAndSettle();
        await tester.runAsync(() async {
          final image = await boundary.toImage(pixelRatio: 1);
          final bytes = await image.toByteData(format: ImageByteFormat.png);
          await File(
            '../.verification/flutter-ui/events-details-${brightness.name}.png',
          ).writeAsBytes(bytes!.buffer.asUint8List());
          image.dispose();
        });
        expect(find.text('Pasos de preparación'), findsNothing);
        expect(find.text('Aspectos a valorar'), findsNothing);
        await revealVisible(tester, find.text('Preparación interna'));
        await tester.tap(find.text('Preparación interna'));
        await tester.pumpAndSettle();
        expect(find.text('Pasos de preparación'), findsOneWidget);
        await tester.scrollUntilVisible(
          find.text('Seguimiento'),
          150,
          scrollable: profileScroll,
        );
        await revealVisible(tester, find.text('Seguimiento'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Seguimiento'));
        await tester.pumpAndSettle();
        expect(find.text('Aspectos a valorar'), findsOneWidget);
        tester.state<ScrollableState>(profileScroll).position.jumpTo(0);
        await tester.pumpAndSettle();
        await tester.tap(find.byKey(const ValueKey('assignment-edit')));
        await tester.pumpAndSettle();
        await tester.runAsync(() async {
          final image = await boundary.toImage(pixelRatio: 1);
          final bytes = await image.toByteData(format: ImageByteFormat.png);
          await File(
            '../.verification/flutter-ui/events-editor-${brightness.name}.png',
          ).writeAsBytes(bytes!.buffer.asUint8List());
          image.dispose();
        });
        final scroll = find
            .descendant(
              of: find.byType(AssignmentEditor),
              matching: find.byType(Scrollable),
            )
            .first;
        await revealVisible(tester, find.text('Aplicaciones'));
        await tester.runAsync(() async {
          final image = await boundary.toImage(pixelRatio: 1);
          final bytes = await image.toByteData(format: ImageByteFormat.png);
          await File(
            '../.verification/flutter-ui/events-editor-options-${brightness.name}.png',
          ).writeAsBytes(bytes!.buffer.asUint8List());
          image.dispose();
        });
        await tester.scrollUntilVisible(
          find.text('Seguimiento'),
          220,
          scrollable: scroll,
        );
        await revealVisible(tester, find.text('Seguimiento'));
        await tester.pumpAndSettle();
        await tester.runAsync(() async {
          final image = await boundary.toImage(pixelRatio: 1);
          final bytes = await image.toByteData(format: ImageByteFormat.png);
          await File(
            '../.verification/flutter-ui/events-follow-up-${brightness.name}.png',
          ).writeAsBytes(bytes!.buffer.asUint8List());
          image.dispose();
        });
      });
    }
  }

  testWidgets(
    'Las tres pestañas usan AppSidebar y conservan su ancho al arrastrar',
    (tester) async {
      await mount(tester);
      final sidebar = find.byType(AppSidebar);
      expect(sidebar, findsOneWidget);
      final initial = tester.getSize(sidebar).width;
      await tester.drag(
        find.bySemanticsLabel('Ancho de la sidebar'),
        const Offset(80, 0),
      );
      await tester.pumpAndSettle();
      final historyWidth = tester.getSize(sidebar).width;
      expect(historyWidth, greaterThan(initial));
      expect(find.byType(SidebarSearch), findsOneWidget);
      final historySearch = tester.widget<TextField>(
        find.byKey(const ValueKey('history-search')),
      );
      await tester.tap(rail('Encargos'));
      await tester.pumpAndSettle();
      expect(sidebar, findsOneWidget);
      expect(tester.getSize(sidebar).width, historyWidth);
      final eventSearch = tester.widget<TextField>(
        find.byKey(const ValueKey('assignments-search')),
      );
      expect(eventSearch.decoration!.prefixIcon, isA<Icon>());
      expect(
        (eventSearch.decoration!.prefixIcon as Icon).icon,
        (historySearch.decoration!.prefixIcon as Icon).icon,
      );
      expect(eventSearch.style, historySearch.style);
      expect(find.byType(SidebarEmpty), findsOneWidget);
      expect(find.byType(ContentEmpty), findsOneWidget);
      await tester.enterText(
        find.byKey(const ValueKey('assignments-search')),
        'sin coincidencia',
      );
      await tester.pumpAndSettle();
      expect(find.text('No se encontraron encargos'), findsOneWidget);
      await tester.tap(find.byIcon(Icons.backspace_outlined));
      await tester.pumpAndSettle();
      expect(
        tester
            .widget<TextField>(find.byKey(const ValueKey('assignments-search')))
            .controller!
            .text,
        isEmpty,
      );
      await tester.drag(
        find.bySemanticsLabel('Ancho de la sidebar'),
        const Offset(-48, 0),
      );
      await tester.pumpAndSettle();
      final eventsWidth = tester.getSize(sidebar).width;
      expect(eventsWidth, lessThan(historyWidth));
      final header = tester.getRect(
        find.byKey(const ValueKey('assignment-content-header')),
      );
      expect(header.left, closeTo(60 + eventsWidth, .5));
      await tester.tap(rail('Plugins'));
      await tester.pumpAndSettle();
      expect(sidebar, findsOneWidget);
      expect(tester.getSize(sidebar).width, eventsWidth);
      expect(find.byType(SidebarSearch), findsNothing);
      expect(find.byType(SidebarEmpty), findsOneWidget);
      expect(find.byType(ContentEmpty), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'Guarda aviso y preparación por separado; conserva borrador y clave tras error',
    (tester) async {
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final backend = AssignmentBackend();
      final state = DesktopState(backend);
      await show(tester, state);
      await tester.tap(find.byIcon(Icons.add).first);
      await tester.pumpAndSettle();
      expect(field('Contexto (opcional)'), findsNothing);
      expect(field('Revisar cada cuántos minutos (opcional)'), findsNothing);
      await openEditorSection(tester, 'settings');
      final statusChoice = tester.widget<DropdownButtonFormField<String>>(
        find.byWidgetPredicate(
          (w) =>
              w is DropdownButtonFormField<String> &&
              w.decoration.labelText == 'Estado del encargo',
        ),
      );
      expect(statusChoice.initialValue, 'active');
      final statusDropdown = tester.widget<DropdownButton<String>>(
        find.descendant(
          of: find.byWidget(statusChoice),
          matching: find.byWidgetPredicate((w) => w is DropdownButton<String>),
        ),
      );
      expect(statusDropdown.items!.map((item) => item.value), [
        'active',
        'paused',
        'completed',
        'cancelled',
      ]);
      expect(find.text('Registrado'), findsNothing);
      await revealVisible(tester, field('Título'));
      await tester.enterText(field('Título'), 'Organizar mi correo');
      await tester.enterText(
        field('Qué debe hacer Gennie'),
        'Preparar un resumen de mensajes pendientes',
      );
      await openEditorSection(tester, 'preparation');
      await tester.scrollUntilVisible(
        field('Revisar cada cuántos minutos (opcional)'),
        400,
        scrollable: find
            .descendant(
              of: find.byType(AssignmentEditor),
              matching: find.byType(Scrollable),
            )
            .first,
      );
      await tester.enterText(
        field('Revisar cada cuántos minutos (opcional)'),
        '10',
      );
      await tester.enterText(
        field('Por qué usar esa cadencia'),
        'Reunir novedades antes de las ocho',
      );
      await openEditorSection(tester, 'preparation');
      expect(field('Revisar cada cuántos minutos (opcional)'), findsNothing);
      await tester.scrollUntilVisible(
        find.text('Guardar encargo'),
        400,
        scrollable: find
            .descendant(
              of: find.byType(AssignmentEditor),
              matching: find.byType(Scrollable),
            )
            .first,
      );
      backend.pending = Completer<Snapshot>();
      await revealVisible(tester, find.text('Guardar encargo'));
      await tester.tap(find.text('Guardar encargo'));
      await tester.pump();
      expect(find.text('Guardando…'), findsOneWidget);
      expect(
        tester
            .widget<TextButton>(find.widgetWithText(TextButton, 'Cancelar'))
            .onPressed,
        isNull,
      );
      expect(tester.widget<TextField>(field('Título')).enabled, isFalse);
      expect(
        tester
            .widget<TextButton>(
              find.byKey(const ValueKey('assignment-editor-preparation')),
            )
            .onPressed,
        isNull,
      );
      final payload = backend.saved.single.$4;
      expect((payload['recurrence'] as Map)['local_time'], '08:00');
      expect((payload['preparation'] as Map)['interval_minutes'], 10);
      expect(payload['reason'], isNull);
      expect(payload['status'], 'active');
      backend.pending!.completeError(Exception('No se pudo guardar'));
      await tester.pumpAndSettle();
      expect(find.textContaining('No se pudo guardar'), findsOneWidget);
      backend.pending = null;
      await revealVisible(tester, find.text('Guardar encargo'));
      await revealVisible(tester, find.text('Guardar encargo'));
      await tester.tap(find.text('Guardar encargo'));
      await tester.pumpAndSettle();
      expect(backend.saved.length, 2);
      expect(backend.saved[0].$3, backend.saved[1].$3);
      expect(backend.saved[1].$4['title'], 'Organizar mi correo');
      expect(find.textContaining('Encargo guardado.'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets(
    'Seguimiento contextual guarda criterios libres y conserva el borrador tras error',
    (tester) async {
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final backend = AssignmentBackend();
      final state = DesktopState(backend);
      await show(tester, state);
      await tester.tap(find.byIcon(Icons.add).first);
      await tester.pumpAndSettle();
      await tester.enterText(field('Título'), 'Acompañar mis tareas');
      await tester.enterText(
        field('Qué debe hacer Gennie'),
        'Resumen de hoy y sin fecha; seguimiento de cambios.',
      );
      final scroll = find
          .descendant(
            of: find.byType(AssignmentEditor),
            matching: find.byType(Scrollable),
          )
          .first;
      Future<void> reveal(Finder finder) async {
        await tester.scrollUntilVisible(finder, 220, scrollable: scroll);
        await revealVisible(tester, finder);
        await tester.pumpAndSettle();
      }

      await reveal(field('Hora del aviso'));
      await tester.enterText(field('Hora del aviso'), '07:30');
      await reveal(find.byKey(const ValueKey('follow-up-policy')));
      await revealVisible(
        tester,
        find.byKey(const ValueKey('follow-up-policy')),
      );
      await tester.tap(find.byKey(const ValueKey('follow-up-policy')));
      await tester.pumpAndSettle();
      await reveal(field('Qué observar'));
      await tester.enterText(
        field('Qué observar'),
        'Tareas abiertas, completadas y reabiertas; conservar cambios silenciosos.',
      );
      await openEditorSection(tester, 'actions');
      await reveal(field('Aspectos a valorar (uno por línea)'));
      await tester.enterText(
        field('Aspectos a valorar (uno por línea)'),
        '¿Ha cambiado el plan del día?\n¿Podemos resolver un bloqueo?',
      );
      await reveal(field('Cómo decidir si intervenir'));
      await tester.enterText(
        field('Cómo decidir si intervenir'),
        'Valorar contexto e historial; adaptar el plazo y registrar sin interrumpir cuando no aporte ayuda.',
      );
      await reveal(find.text('Guardar encargo'));
      backend.pending = Completer<Snapshot>();
      await revealVisible(tester, find.text('Guardar encargo'));
      await tester.tap(find.text('Guardar encargo'));
      await tester.pump();
      final first = backend.saved.single.$4;
      expect(first['recurrence']['local_time'], '07:30');
      expect(first['preparation']['interval_minutes'], 10);
      final policy = first['follow_up'] as Map;
      expect(policy['signals'], [
        '¿Ha cambiado el plan del día?',
        '¿Podemos resolver un bloqueo?',
      ]);
      expect(policy['action_scope'], isNull);
      backend.pending!.completeError(
        Exception('Guardado pendiente de reintento'),
      );
      await tester.pumpAndSettle();
      backend.pending = null;
      await revealVisible(tester, find.text('Guardar encargo'));
      await revealVisible(tester, find.text('Guardar encargo'));
      await tester.tap(find.text('Guardar encargo'));
      await tester.pumpAndSettle();
      expect(backend.saved.length, 2);
      expect(backend.saved.last.$4['follow_up'], policy);
      expect(backend.saved.last.$3, backend.saved.first.$3);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('Validación recupera campos plegados sin perder el borrador', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final backend = AssignmentBackend();
    await show(tester, DesktopState(backend));
    await tester.tap(find.byIcon(Icons.add).first);
    await tester.pumpAndSettle();
    final scroll = tester.state<ScrollableState>(
      find
          .descendant(
            of: find.byKey(const ValueKey('assignment-editor-scroll')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    scroll.position.jumpTo(scroll.position.maxScrollExtent);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Guardar encargo'));
    await tester.pumpAndSettle();
    expect(backend.saved, isEmpty);
    expect(field('Título').hitTestable(), findsOneWidget);
    await tester.enterText(field('Título'), 'Mi borrador');
    await revealVisible(tester, field('Qué debe hacer Gennie'));
    await tester.enterText(
      field('Qué debe hacer Gennie'),
      'Revisar mis pendientes.',
    );
    await openEditorSection(tester, 'context');
    await tester.enterText(
      field('Contexto (opcional)'),
      'Conservar este contexto',
    );
    await openEditorSection(tester, 'context');
    expect(field('Contexto (opcional)'), findsNothing);
    await openEditorSection(tester, 'context');
    expect(
      tester.widget<TextField>(field('Contexto (opcional)')).controller!.text,
      'Conservar este contexto',
    );
    await openEditorSection(tester, 'preparation');
    await tester.enterText(
      field('Revisar cada cuántos minutos (opcional)'),
      'abc',
    );
    await openEditorSection(tester, 'preparation');
    expect(field('Revisar cada cuántos minutos (opcional)'), findsNothing);
    await tester.tap(find.text('Guardar encargo'));
    await tester.pumpAndSettle();
    expect(backend.saved, isEmpty);
    expect(find.textContaining('número entero de minutos'), findsOneWidget);
    expect(
      field('Revisar cada cuántos minutos (opcional)').hitTestable(),
      findsOneWidget,
    );
    expect(
      tester.widget<TextField>(field('Título')).controller!.text,
      'Mi borrador',
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('Guardar seguimiento incompleto no envía el plan', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final backend = AssignmentBackend();
    await show(tester, DesktopState(backend));
    await tester.tap(find.byIcon(Icons.add).first);
    await tester.pumpAndSettle();
    await tester.enterText(field('Título'), 'Seguimiento');
    await tester.enterText(
      field('Qué debe hacer Gennie'),
      'Ayudar con mis pendientes.',
    );
    final scroll = find
        .descendant(
          of: find.byType(AssignmentEditor),
          matching: find.byType(Scrollable),
        )
        .first;
    await tester.scrollUntilVisible(
      find.byKey(const ValueKey('follow-up-policy')),
      220,
      scrollable: scroll,
    );
    await revealVisible(tester, find.byKey(const ValueKey('follow-up-policy')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('follow-up-policy')));
    await tester.pumpAndSettle();
    await tester.scrollUntilVisible(
      find.text('Guardar encargo'),
      220,
      scrollable: scroll,
    );
    await revealVisible(tester, find.text('Guardar encargo'));
    await tester.tap(find.text('Guardar encargo'));
    await tester.pumpAndSettle();
    expect(backend.saved, isEmpty);
    expect(find.textContaining('Completa qué observar'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Criterio largo se despliega y contrae sin cambiar el encargo', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final guidance = List.filled(
      8,
      'Valorar cambios y contexto, respetar respuestas y evitar repetir preguntas sin novedades.',
    ).join(' ');
    final backend = AssignmentBackend()
      ..value = snapshot(
        plans: [
          AssignmentRecord(
            id: 'long-policy',
            revision: 1,
            document: jsonEncode({
              'title': 'Revisar pendientes',
              'description': 'Preparar el día.',
              'kind': 'recurring',
              'priority': 'normal',
              'status': 'active',
              'timezone': 'America/Bogota',
              'recurrence': {'frequency': 'daily', 'local_time': '07:30'},
              'follow_up': {'decision_guidance': guidance},
            }),
            createdAt: '',
            updatedAt: '',
          ),
        ],
      );
    await show(tester, DesktopState(backend));
    await tester.tap(find.text('Revisar pendientes'));
    await tester.pumpAndSettle();
    final preview = find.byWidgetPredicate(
      (w) => w is Text && w.data == guidance,
    );
    expect(tester.widget<Text>(preview).maxLines, 3);
    expect(tester.widget<Text>(preview).overflow, TextOverflow.ellipsis);
    await revealVisible(tester, find.text('Ver más'));
    await tester.tap(find.text('Ver más'));
    await tester.pumpAndSettle();
    expect(
      find.byWidgetPredicate((w) => w is SelectableText && w.data == guidance),
      findsOneWidget,
    );
    await revealVisible(tester, find.text('Ver menos'));
    await tester.tap(find.text('Ver menos'));
    await tester.pumpAndSettle();
    expect(tester.widget<Text>(preview).maxLines, 3);
    expect(find.text('Ver más'), findsOneWidget);
    expect(backend.saved, isEmpty);
  });

  testWidgets('Editar encargo conserva su política y permite retirarla', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    const policy = {
      'scope': 'Cambios en mis pendientes',
      'signals': ['¿Aporta valor intervenir ahora?'],
      'decision_guidance':
          'Respetar respuestas y aplazamientos; dejar constancia aunque no avise.',
      'action_scope': 'Preparar borradores; consultar antes de enviar.',
    };
    final backend = AssignmentBackend()
      ..value = snapshot(
        plans: [
          AssignmentRecord(
            id: 'policy-edit',
            revision: 7,
            document: jsonEncode({
              'title': 'Mi seguimiento',
              'description': 'Acompañar mi día',
              'kind': 'recurring',
              'priority': 'normal',
              'status': 'active',
              'timezone': 'America/Bogota',
              'recurrence': {'frequency': 'daily', 'local_time': '07:30'},
              'preparation': {
                'interval_minutes': 20,
                'rationale': 'Mantener contexto entre entregas',
              },
              'follow_up': policy,
            }),
            createdAt: '2026-10-25T12:00:00Z',
            updatedAt: '2026-10-25T12:00:00Z',
          ),
        ],
      );
    await show(tester, DesktopState(backend));
    await tester.tap(find.text('Mi seguimiento'));
    await tester.pumpAndSettle();
    expect(find.byType(AssignmentProfile), findsOneWidget);
    expect(find.byType(AssignmentEditor), findsNothing);
    expect(field('Título'), findsNothing);
    expect(find.byIcon(Icons.arrow_back), findsNothing);
    await revealVisible(tester, find.byKey(const ValueKey('assignment-edit')));
    await tester.tap(find.byKey(const ValueKey('assignment-edit')));
    await tester.pumpAndSettle();
    await tester.enterText(field('Título'), 'Borrador descartado');
    await tester.tap(find.text('Cancelar'));
    await tester.pumpAndSettle();
    expect(find.byType(AssignmentProfile), findsOneWidget);
    expect(find.text('Borrador descartado'), findsNothing);
    expect(backend.saved, isEmpty);
    await revealVisible(tester, find.byKey(const ValueKey('assignment-edit')));
    await tester.tap(find.byKey(const ValueKey('assignment-edit')));
    await tester.pumpAndSettle();
    expect(
      tester.widget<TextField>(field('Título')).controller!.text,
      'Mi seguimiento',
    );
    await openEditorSection(tester, 'followup');
    final scroll = find
        .descendant(
          of: find.byType(AssignmentEditor),
          matching: find.byType(Scrollable),
        )
        .first;
    await tester.scrollUntilVisible(
      find.text('Guardar encargo'),
      240,
      scrollable: scroll,
    );
    backend.pending = Completer<Snapshot>();
    await revealVisible(tester, find.text('Guardar encargo'));
    await tester.tap(find.text('Guardar encargo'));
    await tester.pump();
    expect(backend.saved.single.$4['follow_up'], policy);
    expect(backend.saved.single.$2, 7);
    backend.pending!.completeError(Exception('Conservar borrador'));
    await tester.pumpAndSettle();
    backend.pending = null;
    await tester.scrollUntilVisible(
      find.byKey(const ValueKey('follow-up-policy')),
      -240,
      scrollable: scroll,
    );
    await revealVisible(tester, find.byKey(const ValueKey('follow-up-policy')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('follow-up-policy')));
    await tester.pumpAndSettle();
    await tester.scrollUntilVisible(
      find.text('Guardar encargo'),
      240,
      scrollable: scroll,
    );
    await revealVisible(tester, find.text('Guardar encargo'));
    await tester.tap(find.text('Guardar encargo'));
    await tester.pumpAndSettle();
    expect(backend.saved.last.$4.containsKey('follow_up'), isFalse);
    expect(backend.saved.last.$4['preparation']['interval_minutes'], 20);
    expect(find.byType(AssignmentProfile), findsOneWidget);
    expect(find.byType(AssignmentEditor), findsNothing);
    expect(tester.takeException(), isNull);
  });

  for (final brightness in Brightness.values) {
    testWidgets('Ficha legible al mínimo en ${brightness.name}', (
      tester,
    ) async {
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      const title = 'Revisar Recordatorios de Apple y preparar el día';
      final backend = AssignmentBackend()
        ..value = snapshot(
          plans: [
            AssignmentRecord(
              id: 'profile',
              revision: 1,
              document: jsonEncode({
                'title': title,
                'description':
                    'Preparar un resumen de las tareas previstas y preguntar por las pendientes.',
                'kind': 'recurring',
                'priority': 'normal',
                'status': 'paused',
                'timezone': 'America/Bogota',
                'recurrence': {
                  'frequency': 'weekly',
                  'local_time': '07:30',
                  'weekdays': [1, 3, 7],
                },
                'context': 'Incluir pendientes sin fecha.',
                'applications': [
                  {'name': 'Recordatorios de Apple', 'kind': 'desktop'},
                ],
              }),
              createdAt: '',
              updatedAt: '',
            ),
          ],
        );
      await show(
        tester,
        DesktopState(backend),
        size: const Size(580, 420),
        brightness: brightness,
      );
      await tester.tap(find.text(title));
      await tester.pumpAndSettle();
      expect(find.byType(AssignmentProfile), findsOneWidget);
      expect(find.byType(AssignmentLogo), findsNWidgets(2));
      expect(find.byType(AssignmentEditor), findsNothing);
      expect(find.text('Criterio para avisarte entre entregas'), findsNothing);
      final scroll = find
          .descendant(
            of: find.byType(AssignmentProfile),
            matching: find.byType(Scrollable),
          )
          .first;
      await tester.scrollUntilVisible(
        find.text('Lun, Mié, Dom · 07:30'),
        120,
        scrollable: scroll,
      );
      expect(find.text('Lun, Mié, Dom · 07:30'), findsOneWidget);
      await tester.scrollUntilVisible(
        find.text('Detalles del encargo'),
        180,
        scrollable: scroll,
      );
      await tester.tap(find.text('Detalles del encargo'));
      await tester.pumpAndSettle();
      await tester.scrollUntilVisible(
        find.text('Incluir pendientes sin fecha.'),
        100,
        scrollable: scroll,
      );
      expect(find.text('Incluir pendientes sin fecha.'), findsOneWidget);
      expect(backend.saved, isEmpty);
      expect(tester.takeException(), isNull);
    });
  }

  for (final brightness in Brightness.values) {
    testWidgets(
      'Encargos y formulario utilizables al mínimo en ${brightness.name}',
      (tester) async {
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        final state = DesktopState(AssignmentBackend());
        await show(
          tester,
          state,
          size: const Size(580, 420),
          brightness: brightness,
        );
        await tester.tap(find.byIcon(Icons.add).first);
        await tester.pumpAndSettle();
        expect(field('Título'), findsOneWidget);
        final scroll = find
            .descendant(
              of: find.byType(AssignmentEditor),
              matching: find.byType(Scrollable),
            )
            .first;
        await tester.scrollUntilVisible(
          find.byKey(const ValueKey('follow-up-policy')),
          220,
          scrollable: scroll,
        );
        await revealVisible(
          tester,
          find.byKey(const ValueKey('follow-up-policy')),
        );
        await tester.tap(find.byKey(const ValueKey('follow-up-policy')));
        await tester.pumpAndSettle();
        await tester.scrollUntilVisible(
          field('Cómo decidir si intervenir'),
          220,
          scrollable: scroll,
        );
        expect(field('Cómo decidir si intervenir'), findsOneWidget);
        await tester.scrollUntilVisible(
          find.text('Guardar encargo'),
          400,
          scrollable: find
              .descendant(
                of: find.byType(AssignmentEditor),
                matching: find.byType(Scrollable),
              )
              .first,
        );
        final actions = tester.getRect(
          find.byKey(const ValueKey('assignment-editor-footer')),
        );
        expect(actions.bottom, lessThanOrEqualTo(420));
        expect(find.text('Guardar encargo').hitTestable(), findsOneWidget);
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          final boundary = tester.renderObject<RenderRepaintBoundary>(
            find.byKey(const ValueKey('editor-capture')),
          );
          await tester.runAsync(() async {
            final image = await boundary.toImage(pixelRatio: 1);
            final bytes = await image.toByteData(format: ImageByteFormat.png);
            await File(
              '../.verification/flutter-ui/events-editor-min-${brightness.name}.png',
            ).writeAsBytes(bytes!.buffer.asUint8List());
            image.dispose();
          });
        }
        expect(tester.takeException(), isNull);
      },
    );
  }
}
