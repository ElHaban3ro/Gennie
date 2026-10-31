import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/rendering.dart';
import 'package:gennie/src/assignments.dart';
import 'chats_preview_test.dart' show loadFonts;
import 'assignments_test.dart' show AssignmentBackend, field, openEditorSection;
import 'widget_test.dart' show mount, rail;

void main() {
  for (final brightness in Brightness.values) {
    testWidgets('todas las etiquetas desplegadas caben al mínimo $brightness', (
      t,
    ) async {
      await loadFonts();
      t.platformDispatcher.textScaleFactorTestValue = 1.5;
      t.platformDispatcher.platformBrightnessTestValue = brightness;
      addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
      addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
      final backend = AssignmentBackend();
      await mount(t, backend: backend, size: const Size(640, 420));
      await t.tap(rail('Encargos'));
      await t.pumpAndSettle();
      await t.tap(find.byIcon(Icons.add).first);
      await t.pumpAndSettle();
      for (final section in ['method', 'context', 'preparation', 'settings']) {
        await openEditorSection(t, section);
      }
      final followUp = find.byKey(const ValueKey('follow-up-policy'));
      await t.ensureVisible(followUp);
      await t.pumpAndSettle();
      await t.tap(followUp);
      await t.pumpAndSettle();
      await openEditorSection(t, 'actions');
      final addApp = find.text('Añadir aplicación');
      await t.ensureVisible(addApp);
      await t.pumpAndSettle();
      await t.tap(addApp);
      await t.pumpAndSettle();
      final clipped = <String>[];
      final labels = <String>[];
      for (final element
          in find
              .descendant(
                of: find.byType(AssignmentEditor),
                matching: find.byType(InputDecorator),
              )
              .evaluate()) {
        final label = (element.widget as InputDecorator).decoration.labelText;
        if (label == null) continue;
        labels.add(label);
        final exact = find.byElementPredicate((e) => identical(e, element));
        final text = find.descendant(of: exact, matching: find.text(label));
        if (t.renderObject<RenderParagraph>(text).didExceedMaxLines) {
          clipped.add(label);
        }
      }
      debugPrint('LABEL_AUDIT ${brightness.name}: ${labels.join(' | ')}');
      expect(labels.length, greaterThanOrEqualTo(20));
      expect(find.text('Un paso por línea.'), findsOneWidget);
      expect(
        t
            .renderObject<RenderParagraph>(find.text('Un paso por línea.'))
            .didExceedMaxLines,
        isFalse,
      );
      expect(clipped, isEmpty, reason: 'Etiquetas elididas: $clipped');
      expect(backend.saved, isEmpty);
      expect(t.takeException(), isNull);
    }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

    testWidgets('etiqueta de cadencia completa y asociada $brightness', (
      t,
    ) async {
      await loadFonts();
      t.platformDispatcher.textScaleFactorTestValue = 1.5;
      t.platformDispatcher.platformBrightnessTestValue = brightness;
      addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
      addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
      final semantics = t.ensureSemantics();
      try {
        final backend = AssignmentBackend();
        await mount(t, backend: backend, size: const Size(640, 420));
        await t.tap(rail('Encargos'));
        await t.pumpAndSettle();
        await t.tap(find.byIcon(Icons.add).first);
        await t.pumpAndSettle();
        await openEditorSection(t, 'preparation');
        const label = 'Intervalo de revisión';
        await t.scrollUntilVisible(
          field(label),
          160,
          scrollable: find
              .descendant(
                of: find.byType(AssignmentEditor),
                matching: find.byType(Scrollable),
              )
              .first,
        );
        await t.pumpAndSettle();
        final paragraph = t.renderObject<RenderParagraph>(find.text(label));
        expect(paragraph.didExceedMaxLines, isFalse);
        const helper = 'Minutos entre revisiones. Opcional.';
        expect(
          t.widget<TextField>(field(label)).decoration?.helperText,
          helper,
        );
        expect(
          t.renderObject<RenderParagraph>(find.text(helper)).didExceedMaxLines,
          isFalse,
        );
        final data = t.getSemantics(field(label)).getSemanticsData();
        expect(data.flagsCollection.isTextField, isTrue);
        expect(data.label, contains(label));
        expect(data.value, isEmpty);
        await t.enterText(field(label), '15');
        await t.pumpAndSettle();
        final edited = t.getSemantics(field(label)).getSemanticsData();
        expect(edited.label, contains(label));
        expect(edited.value, '15');
        FocusManager.instance.primaryFocus?.unfocus();
        final followUp = find.byKey(const ValueKey('follow-up-policy'));
        await t.ensureVisible(followUp);
        await t.pumpAndSettle();
        await t.tap(followUp);
        await t.pumpAndSettle();
        expect(
          t.widget<TextField>(field(label)).decoration?.helperText,
          'Minutos entre revisiones.',
        );
        expect(t.widget<TextField>(field(label)).controller?.text, '15');
        expect(backend.saved, isEmpty);
        expect(t.takeException(), isNull);
      } finally {
        await t.pumpWidget(const SizedBox());
        semantics.dispose();
      }
    }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

    testWidgets('acciones completas en editor mínimo al 150% $brightness', (
      t,
    ) async {
      t.platformDispatcher.textScaleFactorTestValue = 1.5;
      t.platformDispatcher.platformBrightnessTestValue = brightness;
      addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
      addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
      final backend = AssignmentBackend();
      await mount(t, backend: backend, size: const Size(640, 420));
      await t.tap(rail('Encargos'));
      await t.pumpAndSettle();
      await t.tap(find.byIcon(Icons.add).first);
      await t.pumpAndSettle();
      expect(t.takeException(), isNull);
      void checkActions() {
        final footer = t.getRect(
          find.byKey(const ValueKey('assignment-editor-footer')),
        );
        for (final action in [
          find.widgetWithText(TextButton, 'Cancelar'),
          find.widgetWithText(FilledButton, 'Guardar encargo'),
        ]) {
          expect(action.hitTestable(), findsOneWidget);
          final rect = t.getRect(action);
          expect(rect.left, greaterThanOrEqualTo(footer.left));
          expect(rect.right, lessThanOrEqualTo(footer.right));
          expect(rect.bottom, lessThanOrEqualTo(420));
        }
      }

      checkActions();
      await t.tap(find.text('Guardar encargo'));
      await t.pumpAndSettle();
      expect(t.takeException(), isNull);
      checkActions();
      expect(find.text('Completa este campo').hitTestable(), findsWidgets);
      final title = t.widget<EditableText>(
        find.descendant(
          of: field('Título'),
          matching: find.byType(EditableText),
        ),
      );
      expect(title.focusNode.hasFocus, isTrue);
      await t.enterText(field('Título'), 'Borrador QA');
      await t.tap(find.text('Guardar encargo'));
      await t.pumpAndSettle();
      final description = t.widget<EditableText>(
        find.descendant(
          of: field('Qué debe hacer Gennie'),
          matching: find.byType(EditableText),
        ),
      );
      expect(description.focusNode.hasFocus, isTrue);
      checkActions();
      expect(backend.saved, isEmpty);
    }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
  }
  testWidgets('el primer campo inválido recibe teclado en editor amplio', (
    t,
  ) async {
    final backend = AssignmentBackend();
    await mount(t, backend: backend);
    await t.tap(rail('Encargos'));
    await t.pumpAndSettle();
    await t.tap(find.byIcon(Icons.add).first);
    await t.pumpAndSettle();
    await t.tap(find.text('Guardar encargo'));
    await t.pumpAndSettle();
    final title = t.widget<EditableText>(
      find.descendant(of: field('Título'), matching: find.byType(EditableText)),
    );
    expect(title.focusNode.hasFocus, isTrue);
    expect(backend.saved, isEmpty);
  });
}
