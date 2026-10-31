import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/assistant_avatar.dart';
import 'package:gennie/src/assistant_profile.dart';
import 'package:gennie/src/backend.dart';
import 'widget_test.dart' show FakeBackend, snapshot, mount, appTooltip;
import 'chats_preview_test.dart' show loadFonts;

final image = base64Decode(
  'iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGPQyNvyH4QBEc8Ek57FNWUAAAAASUVORK5CYII=',
);

class AvatarBackend extends FakeBackend implements AssistantProfileBackend {
  final saves = <(String, Uint8List?)>[];
  final reads = <String>[];
  Completer<Snapshot>? pending;
  String? prepareError;
  bool readError = false;
  final pendingReads = <String, Completer<Uint8List>>{};
  @override
  Future<Uint8List> loadAvatar(String revision) async {
    reads.add(revision);
    if (pendingReads.containsKey(revision)) {
      return pendingReads[revision]!.future;
    }
    if (readError) throw StateError('Lectura fallida');
    return image;
  }

  @override
  Future<Uint8List> prepareAvatar(Uint8List input) async {
    if (prepareError != null) throw StateError(prepareError!);
    return image;
  }

  @override
  Future<Snapshot> saveProfile(String name, Uint8List? bytes) async {
    saves.add((name, bytes));
    if (pending != null) return pending!.future;
    return value = snapshot(
      profile: AssistantProfile(
        id: 'gennie',
        name: name,
        avatarRevision: bytes?.isEmpty == true ? null : '1',
      ),
    );
  }
}

Finder get nameField => find.byWidgetPredicate(
  (w) => w is TextField && w.decoration?.labelText == 'Nombre del asistente',
);
Future<void> dialog(
  WidgetTester tester,
  DesktopState state,
  Future<Uint8List?> Function() pick,
) async {
  unawaited(
    showDialog<void>(
      context: tester.element(find.byType(AssistantAvatar).first),
      barrierDismissible: false,
      builder: (_) => AssistantProfileDialog(
        state: state,
        profile: state.snapshot!.assistantProfile!,
        pickImage: pick,
      ),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  testWidgets(
    'Escape espera al selector de imagen y cancela sin guardar al regresar',
    (tester) async {
      final fake = AvatarBackend();
      final (_, state) = await mount(tester, backend: fake);
      final initial = state.snapshot!.assistantProfile;
      final selection = Completer<Uint8List?>();
      await dialog(tester, state, () => selection.future);
      await tester.tap(find.text('Elegir imagen…'));
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.byType(AssistantProfileDialog), findsOneWidget);
      expect(find.text('Preparando imagen…'), findsOneWidget);
      selection.complete(null);
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.byType(AssistantProfileDialog), findsNothing);
      expect(fake.saves, isEmpty);
      expect(state.snapshot!.assistantProfile, same(initial));
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'una imagen tardía de otra revisión no reemplaza el perfil actual',
    (tester) async {
      final fake = AvatarBackend()
        ..value = snapshot(
          profile: const AssistantProfile(
            id: 'gennie',
            name: 'Gennie',
            avatarRevision: '1',
          ),
        );
      fake.pendingReads['1'] = Completer<Uint8List>();
      fake.pendingReads['2'] = Completer<Uint8List>();
      final (_, state) = await mount(tester, backend: fake);
      state.accept(
        snapshot(
          profile: const AssistantProfile(
            id: 'gennie',
            name: 'Gennie',
            avatarRevision: '2',
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(fake.reads, ['1', '2']);
      fake.pendingReads['1']!.complete(image);
      await tester.pumpAndSettle();
      expect(find.byType(Image), findsNothing);
      fake.pendingReads['2']!.complete(image);
      await tester.pumpAndSettle();
      expect(find.byType(Image), findsNWidgets(2));
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets(
    'cancelar selector o diálogo conserva nombre e imagen guardados',
    (tester) async {
      final fake = AvatarBackend()
        ..value = snapshot(
          profile: const AssistantProfile(
            id: 'gennie',
            name: 'Gennie',
            avatarRevision: '1',
          ),
        );
      final (_, state) = await mount(tester, backend: fake);
      expect(fake.reads, [
        '1',
      ], reason: 'Cabecera y fila comparten una lectura');
      await dialog(tester, state, () async => null);
      await tester.ensureVisible(find.text('Elegir imagen…'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Elegir imagen…'));
      await tester.pumpAndSettle();
      await tester.enterText(nameField, 'Borrador');
      await tester.tap(find.text('Cancelar'));
      await tester.pumpAndSettle();
      expect(state.snapshot!.assistantProfile!.name, 'Gennie');
      expect(state.snapshot!.assistantProfile!.avatarRevision, '1');
      expect(fake.saves, isEmpty);
      await dialog(tester, state, () async => image);
      await tester.ensureVisible(find.text('Elegir imagen…'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Elegir imagen…'));
      await tester.pumpAndSettle();
      expect(find.byType(Image), findsNWidgets(3));
      await tester.tap(find.text('Cancelar'));
      await tester.pumpAndSettle();
      expect(fake.saves, isEmpty);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets(
    'guardar confirma nombre e imagen juntos y un fallo conserva el borrador',
    (tester) async {
      final fake = AvatarBackend();
      final (_, state) = await mount(tester, backend: fake);
      await dialog(tester, state, () async => image);
      await tester.ensureVisible(find.text('Elegir imagen…'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Elegir imagen…'));
      await tester.pumpAndSettle();
      await tester.enterText(nameField, 'Mi asistente');
      fake.pending = Completer<Snapshot>();
      await tester.tap(find.text('Guardar'));
      await tester.pump();
      expect(fake.saves.single.$1, 'Mi asistente');
      expect(fake.saves.single.$2, image);
      expect(find.text('Guardando…'), findsOneWidget);
      fake.pending!.completeError(StateError('No se pudo guardar'));
      await tester.pumpAndSettle();
      expect(
        tester.widget<TextField>(nameField).controller!.text,
        'Mi asistente',
      );
      expect(
        find.byType(Image),
        findsOneWidget,
        reason: 'La vista previa sigue en el diálogo',
      );
      expect(find.textContaining('No se pudo guardar'), findsWidgets);
      fake.pending = null;
      await tester.tap(find.text('Guardar'));
      await tester.pumpAndSettle();
      expect(find.byType(AssistantProfileDialog), findsNothing);
      expect(find.text('Mi asistente'), findsNWidgets(2));
      expect(find.byType(Image), findsNWidgets(2));
      expect(fake.reads, ['1']);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets(
    'imagen inválida conserva la selección anterior y el nombre editado',
    (tester) async {
      final fake = AvatarBackend()..prepareError = 'Imagen inválida';
      final (_, state) = await mount(tester, backend: fake);
      await dialog(tester, state, () async => Uint8List.fromList([0]));
      await tester.enterText(nameField, 'Mi borrador');
      await tester.ensureVisible(find.text('Elegir imagen…'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Elegir imagen…'));
      await tester.pumpAndSettle();
      expect(find.textContaining('Imagen inválida'), findsOneWidget);
      expect(
        tester.widget<TextField>(nameField).controller!.text,
        'Mi borrador',
      );
      expect(find.byType(Image), findsNothing);
      expect(fake.saves, isEmpty);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets('restaurar pingüino se aplica solo después de guardar', (
    tester,
  ) async {
    final fake = AvatarBackend()
      ..value = snapshot(
        profile: const AssistantProfile(
          id: 'gennie',
          name: 'Gennie',
          avatarRevision: '1',
        ),
      );
    final (_, state) = await mount(tester, backend: fake);
    await dialog(tester, state, () async => null);
    await tester.tap(find.text('Usar pingüino'));
    await tester.pumpAndSettle();
    expect(find.byType(Image), findsNWidgets(2));
    expect(state.snapshot!.assistantProfile!.avatarRevision, '1');
    await tester.tap(find.text('Guardar'));
    await tester.pumpAndSettle();
    expect(fake.saves.single.$2, isEmpty);
    expect(state.snapshot!.assistantProfile!.avatarRevision, isNull);
    expect(find.byType(Image), findsNothing);
  });
  testWidgets(
    'fallo al leer imagen ofrece reintento compartido sin mensajes de sistema',
    (tester) async {
      final fake = AvatarBackend()
        ..readError = true
        ..value = snapshot(
          profile: const AssistantProfile(
            id: 'gennie',
            name: 'Gennie',
            avatarRevision: '1',
          ),
        );
      final (_, state) = await mount(tester, backend: fake);
      expect(
        appTooltip('No se pudo leer la imagen. Abre Personalizar asistente'),
        findsNWidgets(2),
      );
      expect(fake.reads, ['1']);
      await dialog(tester, state, () async => null);
      expect(
        appTooltip('No se pudo leer la imagen. Reintentar'),
        findsOneWidget,
      );
      await tester.tap(appTooltip('No se pudo leer la imagen. Reintentar'));
      await tester.pumpAndSettle();
      expect(
        tester.takeException(),
        isNull,
        reason: 'Un reintento fallido se presenta en el diálogo',
      );
      expect(fake.reads, ['1', '1']);
      fake.readError = false;
      await tester.tap(
        appTooltip('No se pudo leer la imagen. Reintentar').last,
      );
      await tester.pumpAndSettle();
      expect(fake.reads, [
        '1',
        '1',
        '1',
      ], reason: 'El botón dispara una lectura nueva');
      expect(find.byType(Image), findsNWidgets(3));
      expect(fake.reads, ['1', '1', '1']);
      expect(state.failure, isNull);
    },
  );
  for (final brightness in Brightness.values) {
    for (final (size, scale) in [
      (const Size(1100, 720), 1.0),
      (const Size(640, 420), 1.0),
      (const Size(640, 420), 1.5),
    ]) {
      testWidgets('perfil con imagen ${brightness.name} ${size.width} escala $scale', (
        tester,
      ) async {
        tester.platformDispatcher.platformBrightnessTestValue = brightness;
        tester.platformDispatcher.textScaleFactorTestValue = scale;
        addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);
        addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
        await loadFonts();
        final (_, state) = await mount(
          tester,
          backend: AvatarBackend(),
          size: size,
        );
        await dialog(tester, state, () async => image);
        await tester.ensureVisible(find.text('Elegir imagen…'));
        await tester.pumpAndSettle();
        await tester.tap(
          find.text('Elegir imagen…'),
          kind: ui.PointerDeviceKind.mouse,
        );
        await tester.pumpAndSettle();
        await tester.runAsync(
          () => precacheImage(
            MemoryImage(image),
            tester.element(find.byType(AssistantProfileDialog)),
          ),
        );
        await tester.pumpAndSettle();
        expect(
          tester
              .widget<RawImage>(
                find.descendant(
                  of: find.byType(AssistantProfileDialog),
                  matching: find.byType(RawImage),
                ),
              )
              .image,
          isNotNull,
        );
        expect(tester.takeException(), isNull);
        final save = find.text('Guardar');
        await tester.ensureVisible(save);
        await tester.pumpAndSettle();
        expect(save.hitTestable(), findsOneWidget);
        final contrasts = <String, double>{};
        // Medir el texto real contra el fondo rasterizado del control, sin usar píxeles suavizados de letras.
        final boundary = tester.renderObject<RenderRepaintBoundary>(
          find.byKey(const ValueKey('capture')),
        );
        await tester.runAsync(() async {
          final capture = await boundary.toImage();
          final rgba = await capture.toByteData(
            format: ui.ImageByteFormat.rawRgba,
          );
          if (const bool.fromEnvironment('CAPTURE_UI')) {
            final bytes = await capture.toByteData(
              format: ui.ImageByteFormat.png,
            );
            final out = Directory('../.verification/profile-keyboard')
              ..createSync(recursive: true);
            File(
              '${out.path}/measurement-${brightness.name}-${size.width.toInt()}-$scale.png',
            ).writeAsBytesSync(bytes!.buffer.asUint8List());
          }
          for (final label in [
            'Elegir imagen…',
            'Usar pingüino',
            'Cancelar',
            'Guardar',
          ]) {
            final target = find.text(label);
            final rect = tester.getRect(target);
            final text = tester.widget<Text>(target);
            final foreground =
                text.style?.color ??
                DefaultTextStyle.of(tester.element(target)).style.color!;
            final control = tester.getRect(
              find.ancestor(of: target, matching: find.byType(Material)).first,
            );
            // El botón compacto macOS puede empezar por debajo del borde
            // superior de la caja tipográfica. Medir dentro de su Material,
            // junto al borde central y fuera de las letras, no sobre el diálogo.
            final sample = Offset(control.left + 4, control.center.dy);
            expect(control.contains(sample), isTrue);
            expect(rect.contains(sample), isFalse);
            final point = boundary.globalToLocal(sample);
            final offset =
                (point.dy.round() * capture.width + point.dx.round()) * 4;
            final background = Color.fromARGB(
              255,
              rgba!.getUint8(offset),
              rgba.getUint8(offset + 1),
              rgba.getUint8(offset + 2),
            );
            final composed = Color.alphaBlend(foreground, background);
            final a = composed.computeLuminance(),
                b = background.computeLuminance();
            final contrast =
                (a > b ? a + .05 : b + .05) / (a > b ? b + .05 : a + .05);
            contrasts[label] = contrast;
            expect(
              contrast,
              greaterThanOrEqualTo(4.5),
              reason: '$label ${brightness.name}: $contrast',
            );
          }
          capture.dispose();
        });
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          final boundary = tester.renderObject<RenderRepaintBoundary>(
            find.byKey(const ValueKey('capture')),
          );
          await tester.runAsync(() async {
            final capture = await boundary.toImage();
            final bytes = await capture.toByteData(
              format: ui.ImageByteFormat.png,
            );
            final out = Directory('../.verification/assistant-chat-real')
              ..createSync(recursive: true);
            File(
              '${out.path}/avatar-profile-${brightness.name}-${size.width.toInt()}-$scale.png',
            ).writeAsBytesSync(bytes!.buffer.asUint8List());
            File(
              '${out.path}/avatar-contrast-${brightness.name}-${size.width.toInt()}-$scale.json',
            ).writeAsStringSync(jsonEncode(contrasts));
            capture.dispose();
          });
        }
      }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
    }
  }
}
