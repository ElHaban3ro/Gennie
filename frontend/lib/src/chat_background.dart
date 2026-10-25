import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

/// Mosaico decorativo fijo al lector, independiente del scroll y sus mensajes.
class ChatBackground extends StatelessWidget {
  const ChatBackground({super.key});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final dark = theme.brightness == Brightness.dark;
    final color = (dark ? Colors.white : Colors.black).withValues(
      alpha: MediaQuery.highContrastOf(context) ? .14 : (dark ? .085 : .065),
    );
    const tileWidth = 360.0;
    const tileHeight = 300.0;

    return IgnorePointer(
      child: ExcludeSemantics(
        child: RepaintBoundary(
          child: LayoutBuilder(
            builder: (context, constraints) => ClipRect(
              child: Stack(
                children: [
                  for (
                    var row = 0;
                    row < (constraints.maxHeight / tileHeight).ceil();
                    row++
                  )
                    for (
                      var column = 0;
                      column < (constraints.maxWidth / tileWidth).ceil();
                      column++
                    )
                      Positioned(
                        left: column * tileWidth,
                        top: row * tileHeight,
                        width: tileWidth,
                        height: tileHeight,
                        child: SvgPicture.asset(
                          'assets/patterns/chat_faces.svg',
                          colorFilter: ColorFilter.mode(color, BlendMode.srcIn),
                          excludeFromSemantics: true,
                        ),
                      ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
