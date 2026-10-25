import 'dart:io';
import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';
import 'src/backend.dart';
import 'src/desktop.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  if (Platform.isMacOS || Platform.isWindows || Platform.isLinux) {
    await windowManager.ensureInitialized();
    await windowManager.waitUntilReadyToShow(
      const WindowOptions(
        size: Size(1100, 720),
        minimumSize: Size(640, 420),
        title: 'Gennie',
        center: true,
        backgroundColor: Colors.transparent,
        titleBarStyle: TitleBarStyle.hidden,
      ),
    );
    await windowManager.setPreventClose(true);
    await windowManager.show();
  }
  final state = DesktopState(RustBackend());
  runApp(GennieApp(state: state));
  await state.start();
}

class GennieApp extends StatelessWidget {
  const GennieApp({super.key, required this.state});
  final DesktopState state;
  @override
  Widget build(BuildContext context) {
    ThemeData theme(Brightness brightness) => ThemeData(
      brightness: brightness,
      fontFamily: 'Inter',
      useMaterial3: true,
      colorScheme: ColorScheme.fromSeed(
        seedColor: const Color(0xff656a70),
        brightness: brightness,
      ),
      scaffoldBackgroundColor: Colors.transparent,
      tooltipTheme: const TooltipThemeData(waitDuration: Duration.zero),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: Colors.grey.withValues(alpha: .07),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(10),
          borderSide: BorderSide.none,
        ),
      ),
    );
    return MaterialApp(
      title: 'Gennie',
      debugShowCheckedModeBanner: false,
      theme: theme(Brightness.light),
      darkTheme: theme(Brightness.dark),
      builder: (context, child) => ListenableBuilder(
        listenable: state,
        builder: (context, _) => MediaQuery(
          data: MediaQuery.of(
            context,
          ).copyWith(disableAnimations: state.snapshot?.reducedMotion ?? false),
          child: child!,
        ),
      ),
      home: DesktopScreen(state: state),
    );
  }
}
