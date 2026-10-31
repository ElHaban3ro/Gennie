import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/sidebar.dart';
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show measureContrast;

void main() {
  for (final brightness in Brightness.values) {
    for (final scale in [1.0, 1.5]) {
      testWidgets('textos laterales legibles ${brightness.name} $scale', (
        tester,
      ) async {
        await loadFonts();
        tester.view.physicalSize = const Size(640, 420);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        final search = TextEditingController();
        addTearDown(search.dispose);
        final dark = brightness == Brightness.dark;
        await tester.pumpWidget(
          MaterialApp(
            theme: ThemeData(brightness: brightness),
            home: MediaQuery(
              data: MediaQueryData(textScaler: TextScaler.linear(scale)),
              child: RepaintBoundary(
                key: const ValueKey('capture'),
                child: Material(
                  type: MaterialType.transparency,
                  // Base observada por PNG/ICC; AppSidebar compone su propio tinte.
                  child: ColoredBox(
                    color: dark
                        ? const Color(0xff26262a)
                        : const Color(0xffcececd),
                    child: Align(
                      alignment: Alignment.centerLeft,
                      child: AppSidebar(
                        width: 216,
                        title: 'Encargos',
                        searchController: search,
                        searchHint: 'Buscar encargos…',
                        onSearch: (_) {},
                        child: Column(
                          children: [
                            SidebarTile(
                              title: 'Informe semanal',
                              subtitle: 'Recurrente · Activo',
                              selected: false,
                              onSelect: () {},
                            ),
                            SidebarTile(
                              title: 'Revisión mensual',
                              subtitle: 'Puntual · Pausado',
                              selected: true,
                              onSelect: () {},
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        final ratios = await measureContrast(tester, {
          'cabecera': find.text('Encargos'),
          'buscador': find.text('Buscar encargos…'),
          'título': find.text('Informe semanal'),
          'estado': find.text('Recurrente · Activo'),
          'estado seleccionado': find.text('Puntual · Pausado'),
        });
        expect(tester.takeException(), isNull);
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          final output = Directory(
            const String.fromEnvironment('UI_CAPTURE_DIR'),
          )..createSync(recursive: true);
          await tester.runAsync(() async {
            final image = await tester
                .renderObject<RenderRepaintBoundary>(
                  find.byKey(const ValueKey('capture')),
                )
                .toImage();
            try {
              final png = await image.toByteData(
                format: ui.ImageByteFormat.png,
              );
              File(
                '${output.path}/sidebar-text-${brightness.name}-$scale.png',
              ).writeAsBytesSync(png!.buffer.asUint8List());
              File(
                '${output.path}/sidebar-text-${brightness.name}-$scale.txt',
              ).writeAsStringSync('$ratios\n');
            } finally {
              image.dispose();
            }
          });
        }
      });
    }
  }
}
