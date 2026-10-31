import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'backend.dart';
import 'glass.dart';

/// Mantiene el tamaño del avatar vigente; los píxeles se solicitan una vez por revisión.
class AssistantAvatar extends StatefulWidget {
  const AssistantAvatar({
    super.key,
    required this.state,
    required this.profile,
    required this.size,
    this.allowRetry = false,
  });
  final DesktopState state;
  final AssistantProfile? profile;
  final double size;
  final bool allowRetry;
  @override
  State<AssistantAvatar> createState() => _AssistantAvatarState();
}

class _AssistantAvatarState extends State<AssistantAvatar> {
  @override
  Widget build(BuildContext context) {
    final revision = widget.profile?.avatarRevision;
    if (revision == null) {
      return defaultAssistantAvatar(widget.size);
    }
    return FutureBuilder<Uint8List>(
      future: widget.state.avatar(revision),
      builder: (context, snapshot) {
        if (snapshot.hasError) {
          if (!widget.allowRetry) {
            return AppTooltip(
              label: 'No se pudo leer la imagen. Abre Personalizar asistente',
              child: SizedBox(
                width: widget.size,
                height: widget.size,
                child: const Icon(
                  Icons.broken_image_outlined,
                  semanticLabel: 'Imagen del asistente no disponible',
                ),
              ),
            );
          }
          return AppTooltip(
            label: 'No se pudo leer la imagen. Reintentar',
            child: IconButton(
              onPressed: () {
                widget.state.retryAvatar();
                setState(() {});
              },
              icon: const Icon(
                Icons.broken_image_outlined,
                semanticLabel: 'Reintentar imagen del asistente',
              ),
              constraints: BoxConstraints.tightFor(
                width: widget.size,
                height: widget.size,
              ),
              padding: EdgeInsets.zero,
            ),
          );
        }
        if (snapshot.connectionState != ConnectionState.done ||
            !snapshot.hasData) {
          return SizedBox(width: widget.size, height: widget.size);
        }
        return assistantImage(
          snapshot.data!,
          widget.size,
          widget.profile?.name ?? 'Gennie',
        );
      },
    );
  }
}

Widget defaultAssistantAvatar(double size) => SvgPicture.asset(
  'assets/pets/penguin.svg',
  width: size,
  height: size,
  semanticsLabel: 'Gennie, avatar de pingüino',
);

Widget assistantImage(Uint8List bytes, double size, String name) => ClipRRect(
  borderRadius: BorderRadius.circular(8),
  child: Image.memory(
    bytes,
    width: size,
    height: size,
    fit: BoxFit.cover,
    semanticLabel: 'Imagen de $name',
    gaplessPlayback: false,
    errorBuilder: (_, _, _) => SizedBox(
      width: size,
      height: size,
      child: const Icon(
        Icons.broken_image_outlined,
        semanticLabel: 'Imagen no disponible',
      ),
    ),
  ),
);
