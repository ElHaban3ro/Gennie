import 'dart:io';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/main.dart';

void main() {
  testWidgets(
    'macOS muestra la ventana transparente sin sombra del contenido',
    (tester) async {
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      const window = MethodChannel('window_manager');
      const screen = MethodChannel('dev.leanflutter.plugins/screen_retriever');
      final display = {
        'id': 'test',
        'size': {'width': 1440.0, 'height': 900.0},
        'visiblePosition': {'dx': 0.0, 'dy': 0.0},
      };
      var nativeShadow = false; // Valor inicial del runner Swift.
      bool? shadowWhenShown;
      messenger.setMockMethodCallHandler(window, (call) async {
        switch (call.method) {
          case 'setTitleBarStyle':
            // Comportamiento real de window_manager 0.5.2 en macOS.
            nativeShadow = true;
          case 'setHasShadow':
            nativeShadow = (call.arguments as Map)['hasShadow'] as bool;
          case 'show':
            shadowWhenShown = nativeShadow;
          case 'isFullScreen':
          case 'isMaximized':
          case 'isMinimized':
            return false;
          case 'getBounds':
            return {'x': 0.0, 'y': 0.0, 'width': 1100.0, 'height': 720.0};
        }
        return null;
      });
      messenger.setMockMethodCallHandler(screen, (call) async {
        return switch (call.method) {
          'getPrimaryDisplay' => display,
          'getAllDisplays' => {
            'displays': [display],
          },
          'getCursorScreenPoint' => {'dx': 100.0, 'dy': 100.0},
          _ => null,
        };
      });
      addTearDown(() {
        messenger.setMockMethodCallHandler(window, null);
        messenger.setMockMethodCallHandler(screen, null);
      });
      await configureDesktopWindow();
      expect(
        shadowWhenShown,
        false,
        reason:
            'La barra de título reactiva hasShadow después del runner Swift.',
      );
    },
    skip: !Platform.isMacOS,
  );
}
