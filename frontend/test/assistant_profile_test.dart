import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/assistant_profile.dart';
import 'package:gennie/src/backend.dart';
import 'widget_test.dart' show FakeBackend, snapshot, mount;
import 'assistant_avatar_test.dart' show AvatarBackend, dialog, nameField;

class ProfileBackend extends FakeBackend implements AssistantChatBackend {
  String? requested;
  Completer<Snapshot>? pending;
  @override
  Future<ChatPage> loadChatPage(ChatCursor? before) async =>
      const ChatPage(messages: []);
  @override
  Future<Snapshot> renameAssistant(String name) {
    requested = name;
    return pending?.future ??
        Future.value(
          value = snapshot(
            profile: AssistantProfile(id: 'gennie', name: name),
          ),
        );
  }
}

void main() {
  for (final (label, value, message) in [
    ('vacío', '', 'Escribe un nombre de entre 1 y 200 caracteres'),
    ('blanco', ' \t ', 'Escribe un nombre de entre 1 y 200 caracteres'),
    (
      'multilínea',
      'Uno\nDos',
      'El nombre del asistente debe ocupar una sola línea',
    ),
    (
      'límite Unicode',
      '👩‍💻' * 67,
      'Escribe un nombre de entre 1 y 200 caracteres',
    ),
  ]) {
    testWidgets('nombre $label se corrige localmente sin llamar al backend', (
      tester,
    ) async {
      final backend = ProfileBackend();
      final (_, state) = await mount(tester, backend: backend);
      await tester.tap(find.byType(PopupMenuButton<String>));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Personalizar asistente'));
      await tester.pumpAndSettle();
      // La edición programática conserva incluso saltos pegados para validar
      // el contrato del guardado, además del filtrado habitual del teclado.
      tester.widget<TextField>(nameField).controller!.text = value;
      await tester.pump();
      await tester.tap(find.text('Guardar'));
      await tester.pumpAndSettle();
      expect(backend.requested, isNull);
      expect(state.failure, isNull);
      expect(find.text(message), findsOneWidget);
      expect(tester.widget<TextField>(nameField).controller!.text, value);
      final editor = tester.widget<EditableText>(
        find.descendant(of: nameField, matching: find.byType(EditableText)),
      );
      expect(editor.focusNode.hasPrimaryFocus, isTrue);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.byType(AssistantProfileDialog), findsNothing);
      expect(tester.takeException(), isNull);
    }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
  }

  for (final brightness in Brightness.values) {
    testWidgets(
      'error de nombre visible al mínimo y 150 % ${brightness.name}',
      (tester) async {
        tester.platformDispatcher.platformBrightnessTestValue = brightness;
        tester.platformDispatcher.textScaleFactorTestValue = 1.5;
        tester.platformDispatcher.accessibilityFeaturesTestValue =
            const FakeAccessibilityFeatures(
              highContrast: true,
              disableAnimations: true,
            );
        addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);
        addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
        addTearDown(
          tester.platformDispatcher.clearAccessibilityFeaturesTestValue,
        );
        final backend = AvatarBackend();
        final (_, state) = await mount(
          tester,
          backend: backend,
          size: const Size(640, 420),
        );
        await dialog(tester, state, () async => null);
        await tester.enterText(nameField, '');
        await tester.tap(find.text('Guardar'));
        await tester.pumpAndSettle();
        expect(backend.saves, isEmpty);
        expect(state.failure, isNull);
        final error = find.text(
          'Escribe un nombre de entre 1 y 200 caracteres',
        );
        expect(error, findsOneWidget);
        final viewport = tester.getRect(
          find.descendant(
            of: find.byType(AssistantProfileDialog),
            matching: find.byType(SingleChildScrollView),
          ),
        );
        expect(tester.getRect(error).top, greaterThanOrEqualTo(viewport.top));
        expect(
          tester.getRect(error).bottom,
          lessThanOrEqualTo(viewport.bottom + 1),
        );
        expect(
          tester.getRect(error).bottom,
          lessThan(tester.getRect(find.text('Guardar')).top),
        );
        expect(tester.takeException(), isNull);
      },
      variant: TargetPlatformVariant.only(TargetPlatform.macOS),
    );
  }

  testWidgets('nombre de 200 escalares Unicode conserva el texto al guardar', (
    tester,
  ) async {
    final backend = ProfileBackend();
    await mount(tester, backend: backend);
    await tester.tap(find.byType(PopupMenuButton<String>));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Personalizar asistente'));
    await tester.pumpAndSettle();
    final value = '${'a' * 199}😀';
    await tester.enterText(nameField, value);
    await tester.tap(find.text('Guardar'));
    await tester.pumpAndSettle();
    expect(backend.requested, value);
    expect(find.byType(AssistantProfileDialog), findsNothing);
  });

  testWidgets('Tab y Mayús–Tab permanecen dentro del perfil', (tester) async {
    final backend = ProfileBackend()
      ..value = snapshot(
        profile: const AssistantProfile(id: 'gennie', name: 'Gennie'),
      );
    await mount(tester, backend: backend);
    await tester.tap(find.byType(PopupMenuButton<String>));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Personalizar asistente'));
    await tester.pumpAndSettle();
    final editor = tester.widget<EditableText>(
      find.descendant(
        of: find.byType(AssistantProfileDialog),
        matching: find.byType(EditableText),
      ),
    );
    final initial = editor.focusNode;
    // Este backend sin imágenes deja nombre, Cancelar y Guardar habilitados.
    for (var i = 0; i < 3; i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
    }
    expect(initial.hasPrimaryFocus, isTrue);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    for (var i = 0; i < 3; i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
    }
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    expect(initial.hasPrimaryFocus, isTrue);
    expect(backend.sent, isEmpty);
    expect(tester.takeException(), isNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  for (final finish in ['Cancelar', 'Guardar', 'Escape']) {
    testWidgets('el perfil devuelve el foco al menú después de $finish', (
      tester,
    ) async {
      final backend = ProfileBackend()
        ..value = snapshot(
          profile: const AssistantProfile(id: 'gennie', name: 'Gennie'),
        );
      await mount(tester, backend: backend);
      final menu = find.byType(PopupMenuButton<String>);
      final focus = Focus.of(
        tester.element(
          find.descendant(of: menu, matching: find.byType(Icon)).first,
        ),
      );
      focus.requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(find.byType(AssistantProfileDialog), findsOneWidget);
      final field = find.descendant(
        of: find.byType(AssistantProfileDialog),
        matching: find.byType(EditableText),
      );
      expect(tester.widget<EditableText>(field).focusNode.hasFocus, isTrue);
      if (finish == 'Escape') {
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      } else {
        // Nombre → Cancelar → Guardar; activar únicamente con teclado.
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        if (finish == 'Guardar') {
          await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        }
        await tester.sendKeyEvent(LogicalKeyboardKey.space);
      }
      await tester.pumpAndSettle();
      expect(find.byType(AssistantProfileDialog), findsNothing);
      expect(focus.hasPrimaryFocus, isTrue);
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await tester.pumpAndSettle();
      expect(find.text('Personalizar asistente'), findsOneWidget);
      expect(backend.requested, finish == 'Guardar' ? 'Gennie' : null);
      expect(tester.takeException(), isNull);
    }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
  }

  testWidgets(
    'Escape no cierra el perfil mientras guarda y permite cancelar tras fallar',
    (tester) async {
      final backend = ProfileBackend()
        ..value = snapshot(
          profile: const AssistantProfile(id: 'gennie', name: 'Gennie'),
        );
      await mount(tester, backend: backend);
      await tester.tap(find.byType(PopupMenuButton<String>));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Personalizar asistente'));
      await tester.pumpAndSettle();
      backend.pending = Completer<Snapshot>();
      await tester.tap(find.text('Guardar'));
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.byType(AssistantProfileDialog), findsOneWidget);
      backend.pending!.completeError(StateError('No se pudo guardar'));
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.byType(AssistantProfileDialog), findsNothing);
      expect(backend.value.assistantProfile!.name, 'Gennie');
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'personalización usa el perfil y conserva borrador al fallar el guardado',
    (tester) async {
      final backend = ProfileBackend()
        ..value = snapshot(
          profile: const AssistantProfile(id: 'gennie', name: 'Mi Gennie'),
        );
      await mount(tester, backend: backend);
      expect(find.text('Mi Gennie'), findsNWidgets(2));
      await tester.tap(find.byType(PopupMenuButton<String>));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Personalizar asistente'));
      await tester.pumpAndSettle();
      final field = find.byWidgetPredicate(
        (w) =>
            w is TextField && w.decoration?.labelText == 'Nombre del asistente',
      );
      await tester.enterText(field, 'Nuevo nombre');
      backend.pending = Completer<Snapshot>();
      await tester.tap(find.text('Guardar'));
      await tester.pump();
      expect(backend.requested, 'Nuevo nombre');
      expect(find.text('Guardando…'), findsOneWidget);
      backend.pending!.completeError('No se pudo guardar');
      await tester.pumpAndSettle();
      expect(tester.widget<TextField>(field).controller!.text, 'Nuevo nombre');
      expect(find.text('Mi Gennie'), findsNWidgets(2));
      backend.pending = null;
      await tester.tap(find.text('Guardar'));
      await tester.pumpAndSettle();
      expect(find.text('Nuevo nombre'), findsNWidgets(2));
      expect(find.text('Personalizar asistente'), findsNothing);
      expect(
        backend.renamed,
        isEmpty,
        reason: 'No renombrar una sesión al personalizar el bot',
      );
      expect(tester.takeException(), isNull);
    },
  );
}
