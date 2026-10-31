import 'package:flutter/material.dart';
import 'backend.dart';
import 'glass.dart';

String toolDisplayName(ToolCall call) =>
    call.displayName ??
    call.pluginId ??
    (call.server == 'gennie'
        ? call.tool.startsWith('browser_')
              ? 'Gennie Web'
              : call.tool.startsWith('assignments_')
              ? 'Encargos'
              : call.tool.startsWith('plugin_build_')
              ? 'Constructor de plugins'
              : 'Gennie'
        : call.server);

String toolStateLabel(ToolCallOutcome state) => switch (state) {
  ToolCallOutcome.running => 'En curso',
  ToolCallOutcome.completed => 'Finalizada',
  ToolCallOutcome.failed => 'Falló',
  ToolCallOutcome.unknown => 'Sin confirmar',
};

class ToolEvidence extends StatelessWidget {
  const ToolEvidence({
    super.key,
    required this.calls,
    this.onOpenPlugin,
    this.onRequest = false,
  });
  final List<ToolCall> calls;
  final ValueChanged<String>? onOpenPlugin;
  final bool onRequest;

  @override
  Widget build(BuildContext context) {
    final groups = <String, List<ToolCall>>{};
    for (final call in calls) {
      final key = '${call.server}/${call.pluginId ?? toolDisplayName(call)}';
      (groups[key] ??= []).add(call);
    }
    final ink = Theme.of(context).colorScheme.onSurface;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (onRequest)
          const Padding(
            padding: EdgeInsets.only(bottom: 2),
            child: Text(
              'Actividad del asistente',
              style: TextStyle(fontSize: 10),
            ),
          ),
        Wrap(
          spacing: 8,
          runSpacing: 4,
          children: [
            for (final group in groups.values)
              Builder(
                builder: (context) {
                  final named =
                      group.where((c) => c.displayName != null).firstOrNull ??
                      group.first;
                  final name = toolDisplayName(named);
                  final completed = group
                      .where((c) => c.outcome == ToolCallOutcome.completed)
                      .length;
                  final failed = group
                      .where((c) => c.outcome == ToolCallOutcome.failed)
                      .length;
                  final running = group
                      .where((c) => c.outcome == ToolCallOutcome.running)
                      .length;
                  final unknown = group
                      .where((c) => c.outcome == ToolCallOutcome.unknown)
                      .length;
                  final label = completed > 0
                      ? 'Usó: $name${failed > 0 ? ' · $failed ${failed == 1 ? 'fallo' : 'fallos'}' : ''}${unknown > 0 ? ' · $unknown sin confirmar' : ''}${running > 0 ? ' · $running en curso' : ''}'
                      : running > 0
                      ? 'Usando: $name${failed > 0 ? ' · $failed ${failed == 1 ? 'fallo' : 'fallos'}' : ''}${unknown > 0 ? ' · $unknown sin confirmar' : ''}'
                      : unknown > 0
                      ? '$name · sin confirmar${failed > 0 ? ' · $failed ${failed == 1 ? 'fallo' : 'fallos'}' : ''}'
                      : 'Intentó: $name · falló';
                  return AppTooltip(
                    label: 'Ver uso de $name',
                    child: TextButton(
                      onPressed: () => showDialog<void>(
                        context: context,
                        builder: (_) => _ToolEvidenceDialog(
                          name: name,
                          calls: group,
                          onOpenPlugin: onOpenPlugin,
                        ),
                      ),
                      style: ButtonStyle(
                        foregroundColor: WidgetStatePropertyAll(ink),
                        padding: const WidgetStatePropertyAll(
                          EdgeInsets.symmetric(horizontal: 2, vertical: 4),
                        ),
                        minimumSize: const WidgetStatePropertyAll(Size(24, 28)),
                        side: WidgetStateProperty.resolveWith(
                          (states) => states.contains(WidgetState.focused)
                              ? BorderSide(color: ink)
                              : BorderSide.none,
                        ),
                        overlayColor: WidgetStatePropertyAll(
                          ink.withValues(alpha: .08),
                        ),
                      ),
                      child: Text(
                        label,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                          fontSize: 11,
                          height: 1.4,
                          decoration: TextDecoration.underline,
                        ),
                      ),
                    ),
                  );
                },
              ),
          ],
        ),
      ],
    );
  }
}

class _ToolEvidenceDialog extends StatefulWidget {
  const _ToolEvidenceDialog({
    required this.name,
    required this.calls,
    this.onOpenPlugin,
  });
  final String name;
  final List<ToolCall> calls;
  final ValueChanged<String>? onOpenPlugin;
  @override
  State<_ToolEvidenceDialog> createState() => _ToolEvidenceDialogState();
}

class _ToolEvidenceDialogState extends State<_ToolEvidenceDialog> {
  final scroll = ScrollController();
  String get name => widget.name;
  List<ToolCall> get calls => widget.calls;
  ValueChanged<String>? get onOpenPlugin => widget.onOpenPlugin;
  @override
  void dispose() {
    scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final local = calls
        .where(
          (c) =>
              (c.server.startsWith('mcp:') &&
                  RegExp(r'^mcp:[a-z][a-z0-9_]{0,63}$').hasMatch(c.server)) ||
              c.server == 'gennie' &&
                  c.tool == 'browser_plugin_run' &&
                  c.pluginId != null,
        )
        .firstOrNull;
    return Dialog(
      backgroundColor:
          (Theme.of(context).brightness == Brightness.dark
                  ? const Color(0xff26282c)
                  : const Color(0xfff5f5f4))
              .withValues(alpha: MediaQuery.highContrastOf(context) ? 1 : .94),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      child: Glass(
        padding: const EdgeInsets.all(24),
        child: SizedBox(
          width: 420,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Uso de $name',
                style: const TextStyle(fontSize: 18),
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
              ),
              const SizedBox(height: 12),
              Text(
                '${calls.length} ${calls.length == 1 ? 'llamada MCP registrada' : 'llamadas MCP registradas'} en esta solicitud o respuesta.',
                style: const TextStyle(fontSize: 12),
              ),
              const SizedBox(height: 16),
              Flexible(
                child: Scrollbar(
                  controller: scroll,
                  thumbVisibility: true,
                  child: SingleChildScrollView(
                    controller: scroll,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        for (final call in calls)
                          Padding(
                            padding: const EdgeInsets.only(bottom: 16),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Text(
                                  toolStateLabel(call.outcome),
                                  style: const TextStyle(
                                    fontSize: 13,
                                    fontWeight: FontWeight.w600,
                                  ),
                                ),
                                const SizedBox(height: 4),
                                Text(
                                  'Función: ${call.functionName ?? call.tool}',
                                  style: const TextStyle(fontSize: 12),
                                ),
                                Text(
                                  'Servidor: ${call.server}',
                                  style: const TextStyle(fontSize: 12),
                                ),
                                if (call.operation == 'verify')
                                  const Text(
                                    'Modo: verificación',
                                    style: TextStyle(fontSize: 12),
                                  ),
                                if (call.outcome == ToolCallOutcome.unknown)
                                  const Text(
                                    'No se recibió un resultado confirmado para esta llamada.',
                                    style: TextStyle(fontSize: 12),
                                  ),
                                if (call.detail == 'authentication_required')
                                  const Text(
                                    'La función necesita acceso a la aplicación.',
                                    style: TextStyle(fontSize: 12),
                                  ),
                                if (call.detail == 'permission_required')
                                  const Text(
                                    'Faltan permisos para esta función.',
                                    style: TextStyle(fontSize: 12),
                                  ),
                                if (call.detail == 'result_uncertain' &&
                                    call.server.startsWith('mcp:'))
                                  const Text(
                                    'Este intento requirió revisión; consulta su estado en Plugins antes de repetir.',
                                    style: TextStyle(fontSize: 12),
                                  ),
                              ],
                            ),
                          ),
                      ],
                    ),
                  ),
                ),
              ),
              const SizedBox(height: 12),
              Align(
                alignment: Alignment.centerRight,
                child: Wrap(
                  spacing: 8,
                  children: [
                    if (local != null && onOpenPlugin != null)
                      TextButton(
                        onPressed: () {
                          Navigator.pop(context);
                          onOpenPlugin!(
                            local.server.startsWith('mcp:')
                                ? local.server
                                : local.pluginId!,
                          );
                        },
                        child: const Text('Abrir plugin'),
                      ),
                    TextButton(
                      onPressed: () => Navigator.pop(context),
                      child: const Text('Listo'),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
