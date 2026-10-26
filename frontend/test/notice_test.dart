import 'dart:io';
import 'dart:ui' show ImageByteFormat;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/glass.dart';
import 'package:gennie/src/notice.dart';

void main() {
  setUpAll(() async {
    final inter = FontLoader('Inter');
    for (final weight in ['Regular', 'Medium', 'SemiBold']) {
      inter.addFont(rootBundle.load('assets/fonts/Inter-$weight.ttf'));
    }
    await inter.load();
    final icons = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await icons.load();
  });

  for (final brightness in Brightness.values) {
    testWidgets('Aviso compacto: reemplazo y cierre en ${brightness.name}', (
      tester,
    ) async {
      tester.view.physicalSize = const Size(640, 420);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      late BuildContext noticeContext;
      await tester.pumpWidget(
        MaterialApp(
          theme: ThemeData(brightness: brightness, fontFamily: 'Inter'),
          home: RepaintBoundary(
            key: const ValueKey('notice-capture'),
            child: GlassCanvas(
              child: Scaffold(
                backgroundColor: Colors.transparent,
                body: Builder(
                  builder: (context) {
                    noticeContext = context;
                    return const Center(child: Text('Encargos'));
                  },
                ),
              ),
            ),
          ),
        ),
      );
      showAppNotice(noticeContext, 'Encargo guardado.');
      await tester.pumpAndSettle();
      expect(find.text('Encargo guardado.'), findsOneWidget);
      final rect = tester.getRect(find.byType(AppNotice));
      expect(rect.width, lessThan(640 - 32));
      expect(rect.left, greaterThan(16));
      expect(rect.bottom, lessThan(420));
      expect(tester.takeException(), isNull);
      if (const bool.fromEnvironment('CAPTURE_UI')) {
        final boundary = tester.renderObject<RenderRepaintBoundary>(
          find.byKey(const ValueKey('notice-capture')),
        );
        await tester.runAsync(() async {
          final image = await boundary.toImage(pixelRatio: 1);
          final bytes = await image.toByteData(format: ImageByteFormat.png);
          await Directory(
            '../.verification/flutter-ui',
          ).create(recursive: true);
          await File(
            '../.verification/flutter-ui/notice-${brightness.name}.png',
          ).writeAsBytes(bytes!.buffer.asUint8List());
          image.dispose();
        });
      }
      showAppNotice(
        noticeContext,
        'Los cambios del encargo se guardaron correctamente.',
      );
      await tester.pumpAndSettle();
      expect(find.byType(AppNotice), findsOneWidget);
      expect(find.text('Encargo guardado.'), findsNothing);
      expect(
        find.text('Los cambios del encargo se guardaron correctamente.'),
        findsOneWidget,
      );
      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();
      expect(find.byType(AppNotice), findsNothing);
      expect(tester.takeException(), isNull);
    });
  }
}
