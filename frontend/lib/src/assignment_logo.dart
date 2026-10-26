import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'sidebar.dart';

/// Recurso visual de la fuente principal; no consulta contenido ni conecta cuentas.
class AssignmentLogo extends StatefulWidget {
  const AssignmentLogo({super.key, required this.document, this.size = 38});
  final Map<String, dynamic> document;
  final double size;

  @visibleForTesting
  static void clearMemoryCache() => _AssignmentLogoState.requests.clear();

  @override
  State<AssignmentLogo> createState() => _AssignmentLogoState();
}

class _AssignmentLogoState extends State<AssignmentLogo> {
  static const channel = MethodChannel('gennie/assignment-logo');
  // Comparte peticiones entre filas y reconstrucciones; la caché PNG vive en macOS.
  static final requests = <String, Future<Uint8List?>>{};
  late Future<Uint8List?> image;
  String identity = '', label = '';

  void resolve() {
    final apps = widget.document['applications'] as List? ?? const [];
    final app = apps.isEmpty ? null : apps.first as Map;
    label = (app?['name'] ?? widget.document['title'] ?? 'Encargo') as String;
    final kind = app?['kind'] as String? ?? '';
    final url = app?['url'] as String? ?? '';
    final key = '$kind\n$label\n$url';
    if (identity == key) return;
    identity = key;
    if (app == null) {
      image = Future.value(null);
    } else {
      if (requests.length >= 128 && !requests.containsKey(key)) {
        requests.clear();
      }
      image = requests.putIfAbsent(key, () async {
        try {
          return await channel.invokeMethod<Uint8List>('resolve', {
            'name': label,
            'kind': kind,
            'url': url,
          });
        } on MissingPluginException {
          return null;
        } on PlatformException {
          return null;
        }
      });
    }
  }

  @override
  void initState() {
    super.initState();
    resolve();
  }

  @override
  void didUpdateWidget(covariant AssignmentLogo oldWidget) {
    super.didUpdateWidget(oldWidget);
    resolve();
  }

  @override
  Widget build(BuildContext context) {
    final ink = sidebarInk(context);
    final fallback = DecoratedBox(
      decoration: BoxDecoration(
        color: ink.withValues(alpha: .08),
        borderRadius: BorderRadius.circular(9),
      ),
      child: Center(
        child: Text(
          label.trim().isEmpty
              ? 'E'
              : label.trim().characters.first.toUpperCase(),
          style: TextStyle(
            fontFamily: 'Inter',
            fontSize: 16,
            fontWeight: FontWeight.w500,
            color: ink.withValues(alpha: .65),
          ),
        ),
      ),
    );
    return Semantics(
      image: true,
      label: label,
      child: SizedBox(
        width: widget.size,
        height: widget.size,
        child: FutureBuilder<Uint8List?>(
          key: ValueKey(identity),
          future: image,
          builder: (context, result) => result.data == null
              ? fallback
              : Image.memory(
                  result.data!,
                  width: widget.size,
                  height: widget.size,
                  fit: BoxFit.contain,
                  excludeFromSemantics: true,
                  errorBuilder: (context, error, stack) => fallback,
                ),
        ),
      ),
    );
  }
}
