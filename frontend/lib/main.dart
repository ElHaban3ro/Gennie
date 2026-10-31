import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';
import 'src/backend.dart';
import 'src/desktop.dart';
import 'src/glass.dart';
import 'src/system_motion.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  if (Platform.isMacOS || Platform.isWindows || Platform.isLinux) {
    await configureDesktopWindow();
  }
  final state = DesktopState(RustBackend());
  final motion = Platform.isMacOS ? SystemMotion() : null;
  // Obtener el valor inicial antes del primer frame; después llegan cambios.
  await motion?.start();
  runApp(GennieApp(state: state, systemMotion: motion));
  await state.start();
}

/// Configura la ventana antes de iniciar el backend.
Future<void> configureDesktopWindow() async {
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
  // setTitleBarStyle reactiva hasShadow después de la configuración Swift.
  if (Platform.isMacOS) await windowManager.setHasShadow(false);
  await windowManager.show();
}

class GennieApp extends StatelessWidget {
  const GennieApp({super.key, required this.state, this.systemMotion});
  final DesktopState state;
  final ValueListenable<bool>? systemMotion;
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
        listenable: Listenable.merge([state, systemMotion]),
        builder: (context, _) => MediaQuery(
          data: MediaQuery.of(context).copyWith(
            disableAnimations:
                MediaQuery.disableAnimationsOf(context) ||
                (systemMotion?.value ?? false) ||
                (state.snapshot?.reducedMotion ?? false),
          ),
          child: GlassCanvas(child: child!),
        ),
      ),
      home: DesktopScreen(state: state),
    );
  }
}
