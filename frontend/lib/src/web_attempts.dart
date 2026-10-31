part of 'plugins.dart';

/// Actividad operativa del host; no presenta informes libres del helper.
class WebAttemptPanel extends StatefulWidget {
  const WebAttemptPanel({
    super.key,
    required this.pluginId,
    required this.initial,
    required this.call,
    this.onActivity,
  });
  final String pluginId;
  final Map? initial;
  final PluginCall call;
  final Future<void> Function()? onActivity;
  @override
  State<WebAttemptPanel> createState() => _WebAttemptPanelState();
}

class _WebAttemptPanelState extends State<WebAttemptPanel> {
  Map? page;
  List<Map<String, dynamic>> attempts = [];
  bool busy = false;
  String? error;
  @override
  void initState() {
    super.initState();
    receive(widget.initial);
  }

  void receive(Map? value) {
    page = value;
    attempts = (value?['attempts'] as List? ?? [])
        .map((r) => Map<String, dynamic>.from(r as Map))
        .toList();
  }

  @override
  void didUpdateWidget(covariant WebAttemptPanel old) {
    super.didUpdateWidget(old);
    if (!busy && old.initial != widget.initial) {
      receive(widget.initial);
    }
  }

  Future<void> load({bool more = false}) async {
    if (busy) return;
    setState(() {
      busy = true;
      error = null;
    });
    try {
      final value = await widget.call({
        'action': 'web_attempts',
        'plugin_id': widget.pluginId,
        if (more) 'after': page?['next_cursor'],
      });
      if (!mounted) return;
      setState(() {
        if (more) {
          final ids = attempts.map((a) => a['id']).toSet();
          attempts.addAll(
            (value['attempts'] as List)
                .map((r) => Map<String, dynamic>.from(r as Map))
                .where((a) => ids.add(a['id'])),
          );
          page = value;
        } else {
          receive(value);
        }
      });
    } catch (e) {
      if (mounted) setState(() => error = '$e');
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Future<void> review(Map<String, dynamic> attempt) async {
    if (busy || attempt['active'] == true) return;
    setState(() => busy = true);
    await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (_) => McpAttemptReviewDialog(
        attempt: attempt,
        onConfirm: (resolution) async {
          await widget.call({
            'action': 'web_reconcile',
            'plugin_id': widget.pluginId,
            'attempt_id': attempt['id'],
            'expected_revision': attempt['source_revision'],
            'resolution': resolution,
          });
        },
      ),
    );
    if (!mounted) return;
    setState(() => busy = false);
    await load();
    if (mounted) await widget.onActivity?.call();
  }

  @override
  Widget build(BuildContext context) {
    if (page == null ||
        (page?['status'] == 'ready' &&
            (page?['total'] as int? ?? 0) == 0 &&
            error == null)) {
      return const SizedBox.shrink();
    }
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            'Resultados web pendientes',
            style: TextStyle(fontSize: 14, fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 8),
          Text(
            page?['status'] == 'unavailable'
                ? 'No se pudo leer el diario web. Recarga antes de ejecutar.'
                : 'Comprueba el efecto antes de repetir esa función o modificar el plugin. Revisar no repite la operación.',
          ),
          for (final attempt in attempts)
            Padding(
              padding: const EdgeInsets.only(top: 12),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    '${attempt['tool']} · ${attempt['operation'] == 'verify' ? 'Verificación' : 'Ejecución'}',
                    style: const TextStyle(fontWeight: FontWeight.w600),
                  ),
                  Text(
                    '${mcpAttemptDate(attempt['created_at'] as String?)} · revisión ${attempt['source_revision']}',
                  ),
                  if (attempt['candidate_id'] != null)
                    const Text('Intento de un candidato del constructor.'),
                  Text(
                    attempt['active'] == true
                        ? 'La operación sigue en curso.'
                        : 'El resultado no está confirmado.',
                  ),
                  TextButton(
                    onPressed: busy || attempt['active'] == true
                        ? null
                        : () => review(attempt),
                    child: const Text('Revisar resultado'),
                  ),
                ],
              ),
            ),
          if (error != null)
            Text(
              error!,
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
          Wrap(
            spacing: 8,
            children: [
              TextButton(
                onPressed: busy ? null : load,
                child: const Text('Actualizar intentos'),
              ),
              if (page?['next_cursor'] != null)
                TextButton(
                  onPressed: busy ? null : () => load(more: true),
                  child: const Text('Cargar más intentos'),
                ),
            ],
          ),
        ],
      ),
    );
  }
}
