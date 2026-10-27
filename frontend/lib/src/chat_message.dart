import 'dart:async';
import 'dart:convert';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:url_launcher/url_launcher.dart';
import 'backend.dart';

DateTime? messageLocalDay(String timestamp) {
  final date = DateTime.tryParse(timestamp)?.toLocal();
  return date == null ? null : DateTime(date.year, date.month, date.day);
}

String messageTimeLabel(String timestamp) {
  final date = DateTime.tryParse(timestamp)?.toLocal();
  if (date == null) return '';
  return '${date.hour.toString().padLeft(2, '0')}:${date.minute.toString().padLeft(2, '0')}';
}

String chatDayLabel(String timestamp, {required DateTime now}) {
  final date = messageLocalDay(timestamp);
  if (date == null) return '';
  final localNow = now.toLocal();
  final days = DateTime.utc(
    localNow.year,
    localNow.month,
    localNow.day,
  ).difference(DateTime.utc(date.year, date.month, date.day)).inDays;
  if (days == 0) return 'Hoy';
  if (days == 1) return 'Ayer';
  if (days == 2) return 'Antier';
  const weekdays = [
    'lunes',
    'martes',
    'miércoles',
    'jueves',
    'viernes',
    'sábado',
    'domingo',
  ];
  const months = [
    'enero',
    'febrero',
    'marzo',
    'abril',
    'mayo',
    'junio',
    'julio',
    'agosto',
    'septiembre',
    'octubre',
    'noviembre',
    'diciembre',
  ];
  final year = date.year == localNow.year ? '' : ' de ${date.year}';
  return '${weekdays[date.weekday - 1]}, ${date.day} de ${months[date.month - 1]}$year';
}

class ChatDayBadge extends StatelessWidget {
  const ChatDayBadge({super.key, required this.label});
  final String label;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(top: 4, bottom: 24),
    child: Center(
      child: ClipRRect(
        borderRadius: BorderRadius.circular(20),
        child: BackdropFilter(
          filter: ui.ImageFilter.blur(sigmaX: 4, sigmaY: 4),
          blendMode: BlendMode.src,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 5),
            decoration: BoxDecoration(
              color: const Color(0x14808080),
              borderRadius: BorderRadius.circular(20),
            ),
            child: Text(
              label,
              style: TextStyle(
                fontSize: 10,
                fontWeight: FontWeight.w500,
                color: Theme.of(
                  context,
                ).colorScheme.onSurface.withValues(alpha: .6),
              ),
            ),
          ),
        ),
      ),
    ),
  );
}

class ChatMessage extends StatelessWidget {
  const ChatMessage({
    super.key,
    required this.message,
    this.state,
    this.conversationId,
    this.onOpenPlugin,
  });
  final Message message;
  final DesktopState? state;
  final String? conversationId;
  final ValueChanged<String>? onOpenPlugin;

  @override
  Widget build(BuildContext context) {
    final user = message.speaker == 'user';
    final system = message.speaker == 'system';
    final pluginPrefix = system
        ? RegExp(r'^Plugin(?: · ([^:\n]+))?: ').firstMatch(message.text)
        : null;
    final pluginSource = pluginPrefix?.group(1);
    final pluginLabel = pluginPrefix == null
        ? null
        : pluginSource == null ||
              RegExp(r'^trabajo [0-9]+$').hasMatch(pluginSource)
        ? 'Plugin'
        : 'Plugin · $pluginSource';
    final displayText = pluginPrefix == null
        ? message.text
        : message.text.substring(pluginPrefix.end);
    final time = messageTimeLabel(message.createdAt);
    return LayoutBuilder(
      builder: (context, constraints) => Padding(
        padding: const EdgeInsets.only(bottom: 24),
        child: Align(
          alignment: user ? Alignment.centerRight : Alignment.centerLeft,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(16),
            child: BackdropFilter(
              filter: ui.ImageFilter.blur(sigmaX: 4, sigmaY: 4),
              // Sustituir el fondo filtrado: srcOver conserva el patrón nítido
              // al componer sobre el canvas transparente de macOS.
              blendMode: BlendMode.src,
              child: Container(
                constraints: BoxConstraints(
                  minWidth: constraints.maxWidth * .4,
                  maxWidth: constraints.maxWidth * .7,
                ),
                padding: const EdgeInsets.symmetric(
                  horizontal: 16,
                  vertical: 12,
                ),
                decoration: BoxDecoration(
                  color: user
                      ? const Color(0x24D6AD42)
                      : const Color(0x14808080),
                  borderRadius: BorderRadius.circular(16),
                ),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    Flexible(
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          if (system)
                            Padding(
                              padding: const EdgeInsets.only(bottom: 6),
                              child:
                                  pluginLabel != null &&
                                      state != null &&
                                      conversationId != null &&
                                      onOpenPlugin != null
                                  ? _PluginSender(
                                      label: pluginLabel,
                                      message: message,
                                      state: state!,
                                      conversationId: conversationId!,
                                      onOpen: onOpenPlugin!,
                                    )
                                  : Text(
                                      pluginLabel ??
                                          (message.createdAt.isEmpty
                                              ? 'Sistema · sin guardar'
                                              : 'Sistema'),
                                      style: const TextStyle(
                                        fontSize: 11,
                                        fontWeight: FontWeight.w600,
                                      ),
                                    ),
                            ),
                          if (system &&
                              pluginPrefix != null &&
                              state != null &&
                              conversationId != null)
                            _PluginActions(
                              message: message,
                              state: state!,
                              conversationId: conversationId!,
                              displayText: displayText,
                            )
                          else if (user || system)
                            Text(
                              displayText,
                              style: const TextStyle(
                                fontSize: 14,
                                height: 1.65,
                              ),
                            )
                          else
                            MarkdownBody(
                              data: displayText,
                              selectable: false,
                              styleSheet: MarkdownStyleSheet(
                                p: const TextStyle(
                                  fontFamily: 'Inter',
                                  fontSize: 14,
                                  height: 1.65,
                                ),
                              ),
                              onTapLink: (text, href, title) {
                                final uri = Uri.tryParse(href ?? '');
                                if (uri != null &&
                                    ['https', 'http'].contains(uri.scheme)) {
                                  unawaited(
                                    launchUrl(
                                      uri,
                                      mode: LaunchMode.externalApplication,
                                    ),
                                  );
                                }
                              },
                            ),
                        ],
                      ),
                    ),
                    if (time.isNotEmpty)
                      Padding(
                        padding: const EdgeInsets.only(left: 12),
                        child: Transform.translate(
                          offset: const Offset(0, 6),
                          child: Text(
                            time,
                            style: TextStyle(
                              fontSize: 10,
                              color: Theme.of(
                                context,
                              ).colorScheme.onSurface.withValues(alpha: .5),
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

// Los avisos históricos conservan el identificador; los actuales se vinculan
// solo si el resumen y el chat identifican un único trabajo vigente.
class _PluginActions extends StatefulWidget {
  const _PluginActions({
    required this.message,
    required this.state,
    required this.conversationId,
    required this.displayText,
  });
  final Message message;
  final DesktopState state;
  final String conversationId;
  final String displayText;
  @override
  State<_PluginActions> createState() => _PluginActionsState();
}

class _PluginActionsState extends State<_PluginActions> {
  Map<String, dynamic>? job;
  bool busy = false;
  int generation = 0;
  String? error;

  @override
  void initState() {
    super.initState();
    widget.state.addListener(refresh);
    refresh();
  }

  @override
  void didUpdateWidget(covariant _PluginActions oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.state != widget.state) {
      oldWidget.state.removeListener(refresh);
      widget.state.addListener(refresh);
    }
    if (oldWidget.message != widget.message ||
        oldWidget.conversationId != widget.conversationId ||
        oldWidget.state != widget.state) {
      refresh();
    }
  }

  @override
  void dispose() {
    widget.state.removeListener(refresh);
    super.dispose();
  }

  Future<void> refresh() async {
    final version = ++generation;
    final backend = widget.state.backend;
    if (backend is! PluginBackend ||
        !widget.message.text.startsWith('Plugin')) {
      return;
    }
    try {
      final data =
          jsonDecode(
                await (backend as PluginBackend).pluginRequest(
                  jsonEncode({'action': 'list'}),
                ),
              )
              as Map;
      final legacy = RegExp(
        r'^Plugin · trabajo ([0-9]+): ',
      ).firstMatch(widget.message.text);
      final allJobs = (data['jobs'] as List? ?? []).whereType<Map>().toList();
      final matches = allJobs.where((j) {
        if ('${j['trigger_conversation_id']}' != widget.conversationId ||
            ![
              'waiting_user',
              'interrupted',
              'cancelled',
            ].contains(j['state'])) {
          return false;
        }
        if (j['state'] == 'cancelled' &&
            allJobs.any(
              (other) =>
                  other['plugin_id'] == j['plugin_id'] &&
                  [
                    'queued',
                    'running',
                    'waiting_user',
                    'interrupted',
                  ].contains(other['state']),
            )) {
          return false;
        }
        if (legacy != null) {
          return '${j['id']}' == legacy.group(1) &&
              widget.message.text.substring(legacy.end) == j['summary'];
        }
        final host = Uri.tryParse(j['origin'] as String? ?? '')?.host;
        final label = host == null || host.isEmpty
            ? 'Plugin'
            : 'Plugin · $host';
        return widget.message.text == '$label: ${j['summary']}';
      }).toList();
      if (mounted && version == generation) {
        setState(
          () => job = matches.length == 1
              ? Map<String, dynamic>.from(matches.single)
              : null,
        );
      }
    } catch (_) {
      if (mounted && version == generation) setState(() => job = null);
    }
  }

  String get accessKey =>
      '${widget.conversationId}:${job?["id"]}:${widget.message.createdAt}:${widget.message.text}';

  Future<void> act(String action) async {
    if (busy || job == null) return;
    setState(() {
      busy = true;
      error = null;
    });
    try {
      final selectedJob = job!;
      Map<String, dynamic> request;
      if (action == 'rebuild') {
        final id = selectedJob['id'] as int;
        final key = widget.state.pluginRebuildKeys.putIfAbsent(
          id,
          () =>
              'plugin-chat-rebuild-$id-${DateTime.now().microsecondsSinceEpoch}',
        );
        request = {
          'action': 'build',
          'trigger_conversation_id': selectedJob['trigger_conversation_id'],
          'request': {
            'request_key': key,
            'retry_of': id,
            'plugin_id': selectedJob['plugin_id'],
            'origin': selectedJob['origin'],
            'objective': selectedJob['objective'],
            'max_attempts': selectedJob['max_attempts'],
            'max_seconds': selectedJob['max_seconds'],
          },
        };
      } else {
        request = {'action': action, 'job_id': selectedJob['id']};
      }
      await (widget.state.backend as PluginBackend).pluginRequest(
        jsonEncode(request),
      );
      if (action == 'rebuild') {
        widget.state.pluginRebuildKeys.remove(selectedJob['id']);
      }
      if (action == 'connect') widget.state.pluginAccessOpened.add(accessKey);
      if (action == 'resume') widget.state.pluginAccessOpened.remove(accessKey);
      await refresh();
    } catch (e) {
      if (mounted) setState(() => error = 'No se pudo realizar la acción: $e');
      await refresh();
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final text = job?['state'] == 'waiting_user'
        ? widget.displayText.replaceAll(
            'Continuar en Plugins',
            'Continuar en este mensaje',
          )
        : widget.displayText;
    final ink = Theme.of(context).colorScheme.onSurface;
    final buttonStyle = TextButton.styleFrom(
      foregroundColor: ink,
      disabledForegroundColor: ink.withValues(alpha: .38),
      backgroundColor: const Color(0x14808080),
      disabledBackgroundColor: const Color(0x0A808080),
      overlayColor: ink,
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      minimumSize: const Size(0, 36),
      tapTargetSize: MaterialTapTargetSize.shrinkWrap,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
      textStyle: const TextStyle(
        fontFamily: 'Inter',
        fontSize: 12,
        fontWeight: FontWeight.w600,
      ),
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(text, style: const TextStyle(fontSize: 14, height: 1.65)),
        if (job != null)
          Padding(
            padding: const EdgeInsets.only(top: 10),
            child: Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                if (job!['state'] == 'cancelled')
                  TextButton(
                    onPressed: busy ? null : () => act('rebuild'),
                    style: buttonStyle,
                    child: const Text('Volver a generar'),
                  ),
                if (job!['state'] == 'waiting_user' &&
                    !widget.state.pluginAccessOpened.contains(accessKey))
                  TextButton(
                    onPressed: busy ? null : () => act('connect'),
                    style: buttonStyle,
                    child: const Text('Abrir Gennie Web'),
                  ),
                if (job!['state'] == 'interrupted' ||
                    (job!['state'] == 'waiting_user' &&
                        widget.state.pluginAccessOpened.contains(accessKey)))
                  TextButton(
                    onPressed: busy ? null : () => act('resume'),
                    style: buttonStyle,
                    child: const Text('Continuar'),
                  ),
              ],
            ),
          ),
        if (error != null)
          Text(
            error!,
            style: TextStyle(
              color: Theme.of(context).colorScheme.error,
              fontSize: 12,
            ),
          ),
      ],
    );
  }
}

class _PluginSender extends StatefulWidget {
  const _PluginSender({
    required this.label,
    required this.message,
    required this.state,
    required this.conversationId,
    required this.onOpen,
  });
  final String label;
  final Message message;
  final DesktopState state;
  final String conversationId;
  final ValueChanged<String> onOpen;
  @override
  State<_PluginSender> createState() => _PluginSenderState();
}

class _PluginSenderState extends State<_PluginSender> {
  String? pluginId;
  bool hover = false;
  int generation = 0;
  @override
  void initState() {
    super.initState();
    widget.state.addListener(resolve);
    resolve();
  }

  @override
  void didUpdateWidget(covariant _PluginSender old) {
    super.didUpdateWidget(old);
    if (old.state != widget.state) {
      old.state.removeListener(resolve);
      widget.state.addListener(resolve);
    }
    if (old.message != widget.message ||
        old.conversationId != widget.conversationId ||
        old.state != widget.state) {
      resolve();
    }
  }

  @override
  void dispose() {
    widget.state.removeListener(resolve);
    super.dispose();
  }

  Future<void> resolve() async {
    final version = ++generation;
    final backend = widget.state.backend;
    if (backend is! PluginBackend) return;
    try {
      final data =
          jsonDecode(
                await (backend as PluginBackend).pluginRequest(
                  jsonEncode({'action': 'list'}),
                ),
              )
              as Map;
      final legacy = RegExp(
        r'^Plugin · trabajo ([0-9]+): ',
      ).firstMatch(widget.message.text);
      final prefix = RegExp(
        r'^Plugin(?: · ([^:\n]+))?: ',
      ).firstMatch(widget.message.text)!;
      final body = widget.message.text.substring(prefix.end);
      final ids = <String>{};
      for (final item in (data['jobs'] as List? ?? []).whereType<Map>()) {
        if ('${item['trigger_conversation_id']}' != widget.conversationId) {
          continue;
        }
        if (legacy != null && '${item['id']}' != legacy.group(1)) continue;
        if (legacy == null) {
          final host = Uri.tryParse(item['origin'] as String? ?? '')?.host;
          if (prefix.group(1) != host &&
              !(prefix.group(1) == null && (host == null || host.isEmpty))) {
            continue;
          }
        }
        final events = (item['events'] as List? ?? []).whereType<Map>();
        if (item['summary'] != body &&
            !events.any((event) => event['message'] == body)) {
          continue;
        }
        final id = item['plugin_id'];
        if (id is String && id.isNotEmpty) ids.add(id);
      }
      if (mounted && version == generation) {
        setState(() => pluginId = ids.length == 1 ? ids.single : null);
      }
    } catch (_) {
      if (mounted && version == generation) setState(() => pluginId = null);
    }
  }

  @override
  Widget build(BuildContext context) {
    final ink = Theme.of(context).colorScheme.onSurface;
    final style = TextStyle(
      fontSize: 11,
      fontWeight: FontWeight.w600,
      color: hover ? ink.withValues(alpha: .6) : ink,
      decoration: pluginId == null ? null : TextDecoration.underline,
    );
    if (pluginId == null) return Text(widget.label, style: style);
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => hover = true),
      onExit: (_) => setState(() => hover = false),
      child: TextButton(
        onPressed: () => widget.onOpen(pluginId!),
        style: TextButton.styleFrom(
          padding: EdgeInsets.zero,
          minimumSize: Size.zero,
          tapTargetSize: MaterialTapTargetSize.shrinkWrap,
          overlayColor: Colors.transparent,
        ),
        child: Text(widget.label, style: style),
      ),
    );
  }
}
