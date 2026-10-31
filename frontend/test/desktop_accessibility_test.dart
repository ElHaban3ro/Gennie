import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/glass.dart';
import 'package:gennie/src/sidebar.dart';
import 'package:gennie/src/backend.dart';
import 'chats_preview_test.dart' show loadFonts;
import 'widget_test.dart'
    show mount, snapshot, appTooltip, composer, rail, draft;
import 'chat_cancellation_test.dart' show StopBackend, update, responding;
import 'message_reply_test.dart' show initial, reply;

Finder iconAction(String tooltip) =>
    find.descendant(of: appTooltip(tooltip), matching: find.byType(IconButton));

void expectNamedButton(WidgetTester t, String label, {bool enabled = true}) {
  final data = t
      .getSemantics(find.bySemanticsLabel(label).first)
      .getSemanticsData();
  expect(data.label, label);
  expect(data.flagsCollection.isButton, isTrue);
  expect(data.hasAction(ui.SemanticsAction.tap), enabled);
}

void main() {
  testWidgets(
    'el movimiento reducido del sistema y de la app detienen el indicador real',
    (t) async {
      t.platformDispatcher.accessibilityFeaturesTestValue =
          const FakeAccessibilityFeatures(disableAnimations: true);
      addTearDown(t.platformDispatcher.clearAccessibilityFeaturesTestValue);
      final (backend, _) = await mount(t);
      final indicator = find.descendant(
        of: rail('Laya'),
        matching: find.byType(CircularProgressIndicator),
      );
      Future<void> showLoading({bool reducedMotion = false}) async {
        backend.events.add(
          snapshot(
            modelState: ModelState.loading,
            reducedMotion: reducedMotion,
          ),
        );
        await t.pump();
        await t.pump(const Duration(milliseconds: 100));
      }

      await showLoading();
      expect(t.widget<CircularProgressIndicator>(indicator).value, .75);

      t.platformDispatcher.accessibilityFeaturesTestValue =
          const FakeAccessibilityFeatures();
      await t.pump();
      expect(t.widget<CircularProgressIndicator>(indicator).value, isNull);

      await showLoading(reducedMotion: true);
      expect(t.widget<CircularProgressIndicator>(indicator).value, .75);

      t.platformDispatcher.accessibilityFeaturesTestValue =
          const FakeAccessibilityFeatures(disableAnimations: true);
      await t.pump();
      await showLoading();
      expect(t.widget<CircularProgressIndicator>(indicator).value, .75);

      t.platformDispatcher.accessibilityFeaturesTestValue =
          const FakeAccessibilityFeatures();
      await t.pump();
      expect(t.widget<CircularProgressIndicator>(indicator).value, isNull);
      expect(backend.motionSaved, isNull);
      expect(backend.modelRequests, isEmpty);
      expect(t.takeException(), isNull);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );
  testWidgets(
    'el buscador estrecho conserva una línea y su nombre completo con texto ampliado',
    (t) async {
      await loadFonts();
      t.platformDispatcher.textScaleFactorTestValue = 1.5;
      addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
      final semantics = t.ensureSemantics();
      final controller = TextEditingController();
      var changes = 0;
      const hint = 'Buscar encargos…';
      try {
        await t.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: Align(
                alignment: Alignment.topLeft,
                child: SizedBox(
                  width: 216,
                  child: SidebarSearch(
                    controller: controller,
                    hint: hint,
                    onChanged: (_) => changes++,
                  ),
                ),
              ),
            ),
          ),
        );
        await t.pumpAndSettle();
        final paragraph = t.renderObject<RenderParagraph>(find.text(hint));
        final lines = paragraph
            .getBoxesForSelection(
              TextSelection(baseOffset: 0, extentOffset: hint.length),
            )
            .map((box) => box.top.round())
            .toSet();
        expect(lines, hasLength(1));
        expect(
          t.getSemantics(find.bySemanticsLabel(hint)).getSemanticsData().label,
          hint,
        );
        expect(changes, 0);
        expect(t.takeException(), isNull);
      } finally {
        await t.pumpWidget(const SizedBox());
        controller.dispose();
        semantics.dispose();
      }
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );
  testWidgets(
    'la navegación une nombre, estado y acción en el mismo botón accesible',
    (t) async {
      final handle = t.ensureSemantics();
      try {
        await mount(t);
        for (final label in ['Encargos', 'Plugins', 'Chats']) {
          final node = t.getSemantics(find.bySemanticsLabel(label).first);
          final data = node.getSemanticsData();
          expect(data.label, label);
          expect(data.flagsCollection.isButton, isTrue);
          expect(data.hasAction(ui.SemanticsAction.tap), isTrue);
          node.owner!.performAction(node.id, ui.SemanticsAction.tap);
          await t.pumpAndSettle();
          expect(
            t
                .getSemantics(find.bySemanticsLabel(label).first)
                .getSemanticsData()
                .flagsCollection
                .isSelected,
            ui.Tristate.isTrue,
          );
        }
        final settings = t.getSemantics(
          find.bySemanticsLabel('Configuración').first,
        );
        expect(settings.getSemanticsData().label, 'Configuración');
        expect(
          settings.getSemanticsData().hasAction(ui.SemanticsAction.tap),
          isTrue,
        );
        settings.owner!.performAction(settings.id, ui.SemanticsAction.tap);
        await t.pumpAndSettle();
        expectNamedButton(t, 'Cerrar configuración');
      } finally {
        handle.dispose();
      }
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );

  testWidgets(
    'los controles de modelos conservan estado y bloqueo accesibles',
    (t) async {
      final handle = t.ensureSemantics();
      var calls = 0;
      try {
        for (final (enabled, toggled, busy) in [
          (true, false, false),
          (true, true, false),
          (false, true, true),
          (false, false, false),
        ]) {
          await t.pumpWidget(
            MaterialApp(
              home: Material(
                child: RailButton(
                  label: 'Control de prueba',
                  icon: Icons.psychology_outlined,
                  appearance: RailButtonAppearance.modelStatus,
                  toggled: toggled,
                  busy: busy,
                  onPressed: enabled ? () => calls++ : null,
                ),
              ),
            ),
          );
          await t.pump();
          final node = t.getSemantics(
            find.bySemanticsLabel('Control de prueba').first,
          );
          final data = node.getSemanticsData();
          expect(data.flagsCollection.isButton, isTrue);
          expect(
            data.flagsCollection.isEnabled,
            enabled ? ui.Tristate.isTrue : ui.Tristate.isFalse,
          );
          expect(
            data.flagsCollection.isToggled,
            toggled ? ui.Tristate.isTrue : ui.Tristate.isFalse,
          );
          expect(data.flagsCollection.isSelected, ui.Tristate.none);
          expect(data.hasAction(ui.SemanticsAction.tap), enabled);
          if (enabled) {
            node.owner!.performAction(node.id, ui.SemanticsAction.tap);
            await t.pump();
          }
        }
        expect(calls, 2);
      } finally {
        handle.dispose();
      }
    },
  );

  testWidgets('el botón unido conserva foco y una pulsación por activación', (
    t,
  ) async {
    final handle = t.ensureSemantics();
    var calls = 0;
    try {
      await t.pumpWidget(
        MaterialApp(
          home: Material(
            child: RailButton(
              label: 'Acción de prueba',
              icon: Icons.history,
              onPressed: () => calls++,
            ),
          ),
        ),
      );
      await t.pumpAndSettle();
      await t.sendKeyEvent(LogicalKeyboardKey.tab);
      await t.pumpAndSettle();
      final node = t.getSemantics(
        find.bySemanticsLabel('Acción de prueba').first,
      );
      expect(
        node.getSemanticsData().flagsCollection.isFocused,
        ui.Tristate.isTrue,
      );
      await t.sendKeyEvent(LogicalKeyboardKey.space);
      await t.pumpAndSettle();
      expect(calls, 1);
      node.owner!.performAction(node.id, ui.SemanticsAction.tap);
      await t.pumpAndSettle();
      expect(calls, 2);
    } finally {
      handle.dispose();
    }
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  for (final enter in [
    LogicalKeyboardKey.enter,
    LogicalKeyboardKey.numpadEnter,
  ]) {
    testWidgets(
      'Mayús–${enter.debugName} inserta una línea en la selección sin enviar',
      (t) async {
        final (backend, _) = await mount(t);
        await t.enterText(composer(), 'Hola 👋 mundo');
        final editor = t.widget<TextField>(composer()).controller!;
        editor.selection = const TextSelection(baseOffset: 5, extentOffset: 7);
        await t.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
        await t.sendKeyEvent(enter);
        await t.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
        await t.pump();
        expect(draft(t), 'Hola \n mundo');
        expect(editor.selection, const TextSelection.collapsed(offset: 6));
        expect(backend.sent, isEmpty);
        await t.sendKeyEvent(enter);
        await t.pump();
        expect(backend.sent, ['Hola \n mundo']);
      },
      variant: TargetPlatformVariant.only(TargetPlatform.macOS),
    );
  }
  testWidgets(
    'las acciones de icono exponen nombre en el botón y no solo ayuda',
    (t) async {
      final handle = t.ensureSemantics();
      try {
        await mount(t);
        expectNamedButton(t, 'Nueva conversación');
        expectNamedButton(t, 'Opciones del sistema');
        expectNamedButton(t, 'Abrir Gennie Web');
        await t.enterText(composer(), 'Borrador');
        expectNamedButton(t, 'Enviar mensaje');
        await t.tap(rail('Configuración'));
        await t.pumpAndSettle();
        expectNamedButton(t, 'Cerrar configuración');
      } finally {
        handle.dispose();
      }
    },
  );

  testWidgets(
    'campos editados y limpiar búsqueda mantienen nombres accesibles',
    (t) async {
      final handle = t.ensureSemantics();
      try {
        await mount(t);
        final search = find.byKey(const ValueKey('history-search'));
        expect(
          t.getSemantics(search).getSemanticsData().label,
          'Buscar asistentes…',
        );
        await t.enterText(search, 'Conversación');
        await t.pumpAndSettle();
        expect(
          t.getSemantics(search).getSemanticsData().label,
          'Buscar asistentes…',
        );
        expectNamedButton(t, 'Limpiar búsqueda');
        await t.tap(iconAction('Limpiar búsqueda'));
        await t.pumpAndSettle();
        expect(
          t.getSemantics(search).getSemanticsData().label,
          'Buscar asistentes…',
        );
        await t.enterText(composer(), 'Borrador distinto del nombre');
        await t.pumpAndSettle();
        final field = t.getSemantics(composer()).getSemanticsData();
        expect(field.label, 'Mensaje para Conversación actual');
        expect(field.value, 'Borrador distinto del nombre');
        await t.enterText(composer(), '');
        await t.pumpAndSettle();
        expect(
          t.getSemantics(composer()).getSemanticsData().label,
          'Mensaje para Conversación actual',
        );
      } finally {
        handle.dispose();
      }
    },
  );

  testWidgets(
    'detener, esperar y cancelar cita conservan nombre y acción reales',
    (t) async {
      final handle = t.ensureSemantics();
      try {
        final b = StopBackend();
        final (_, state) = await mount(
          t,
          backend: b,
          initialSnapshot: snapshot(messages: initial),
        );
        await reply(t, 'Respuesta de Gennie');
        expectNamedButton(t, 'Cancelar respuesta');
        await update(t, b, state, responding('a11y'));
        expectNamedButton(t, 'Detener respuesta');
        await update(t, b, state, responding('a11y', stopping: true));
        expectNamedButton(t, 'Deteniendo respuesta', enabled: false);
        await update(t, b, state, snapshot());
      } finally {
        handle.dispose();
      }
    },
  );
}
