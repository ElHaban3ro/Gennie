import 'capture_output.dart';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import '../design/chats_preview.dart';

Future<void> loadFonts() async {
  await (FontLoader(
    'MaterialIcons',
  )..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'))).load();
  for (final entry in {
    'Regular': FontWeight.w400,
    'Medium': FontWeight.w500,
    'SemiBold': FontWeight.w600,
    'Bold': FontWeight.w700,
  }.entries) {
    await (FontLoader(
      'Inter',
    )..addFont(rootBundle.load('assets/fonts/Inter-${entry.key}.ttf'))).load();
  }
}

Future<void> mount(
  WidgetTester tester,
  Size size,
  Brightness brightness, {
  double scale = 1,
}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = size;
  await tester.pumpWidget(
    MaterialApp(
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        brightness: brightness,
        fontFamily: 'Inter',
        useMaterial3: true,
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xff656a70),
          brightness: brightness,
        ),
      ),
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(context).copyWith(
          textScaler: TextScaler.linear(scale),
          disableAnimations: true,
        ),
        child: child!,
      ),
      home: const RepaintBoundary(
        key: ValueKey('preview-capture'),
        child: ChatsPreview(),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  setUpAll(loadFonts);
  for (final brightness in Brightness.values) {
    for (final size in [const Size(1100, 720), const Size(640, 420)]) {
      testWidgets('visual ${brightness.name} ${size.width}', (tester) async {
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        await mount(tester, size, brightness);
        expect(tester.takeException(), isNull);
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          final boundary = tester.renderObject<RenderRepaintBoundary>(
            find.byKey(const ValueKey('preview-capture')),
          );
          await tester.runAsync(() async {
            final image = await boundary.toImage();
            final bytes = await image.toByteData(
              format: ui.ImageByteFormat.png,
            );
            captureOutput(
              '../.verification/chats-design',
              '${brightness.name}-${size.width.toInt()}.png',
            ).writeAsBytesSync(bytes!.buffer.asUint8List());
            image.dispose();
          });
        }
      });
    }
  }
  testWidgets(
    'responder, cancelar y nueva conversación conservan historial y borrador',
    (tester) async {
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await mount(tester, const Size(640, 420), Brightness.light);
      await tester.tap(find.byIcon(Icons.reply_rounded).first);
      await tester.pumpAndSettle();
      expect(find.text('Respondiendo a un mensaje'), findsOneWidget);
      await tester.enterText(find.byType(TextField).last, 'Mi borrador');
      await tester.tap(find.byIcon(Icons.close_rounded));
      await tester.pumpAndSettle();
      expect(find.text('Mi borrador'), findsOneWidget);
      await tester.tap(find.byIcon(Icons.add_comment_outlined));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Iniciar conversación'));
      await tester.pumpAndSettle();
      await tester.scrollUntilVisible(
        find.text('Nueva conversación · contexto nuevo'),
        120,
        scrollable: find.descendant(
          of: find.byKey(const ValueKey('preview-history')),
          matching: find.byType(Scrollable),
        ),
      );
      expect(find.text('Nueva conversación · contexto nuevo'), findsOneWidget);
      expect(find.text('Mi borrador'), findsOneWidget);
      expect(find.text('Mi borrador'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets('texto ampliado conserva controles en ventana mínima', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await mount(tester, const Size(640, 420), Brightness.dark, scale: 1.5);
    await tester.ensureVisible(find.byIcon(Icons.reply_rounded).first);
    await tester.pumpAndSettle();
    await tester.tap(find.byIcon(Icons.reply_rounded).first);
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    expect(find.text('Respondiendo a un mensaje'), findsOneWidget);
    expect(find.byIcon(Icons.arrow_upward_rounded), findsOneWidget);
  });
}
