import 'dart:math' as math;
import 'package:flutter/material.dart';
import 'chat_background.dart';

/// Desvanece el lector completo antes de los controles inferiores.
class ChatViewport extends StatelessWidget {
  const ChatViewport({super.key, required this.child, this.enabled = true});

  final Widget child;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    if (!enabled) return child;
    return ShaderMask(
      blendMode: BlendMode.dstIn,
      shaderCallback: (bounds) =>
          const LinearGradient(
            begin: Alignment.topCenter,
            end: Alignment.bottomCenter,
            colors: [Colors.white, Colors.transparent],
          ).createShader(
            Rect.fromLTWH(
              0,
              math.max(0, bounds.height - 56),
              bounds.width,
              math.min(56, bounds.height),
            ),
          ),
      child: Stack(
        fit: StackFit.expand,
        children: [const ChatBackground(), child],
      ),
    );
  }
}
