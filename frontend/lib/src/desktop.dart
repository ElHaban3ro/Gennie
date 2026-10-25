import 'dart:async';
import 'dart:math' as math;
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:window_manager/window_manager.dart';
import 'backend.dart';
import 'chat_history.dart';
import 'chat_message.dart';
import 'chat_viewport.dart';
import 'glass.dart';
import 'settings.dart';
import 'model_controls.dart';

class DesktopScreen extends StatefulWidget {
  const DesktopScreen({super.key, required this.state});
  final DesktopState state;
  @override
  State<DesktopScreen> createState() => _DesktopScreenState();
}

class _DesktopScreenState extends State<DesktopScreen> with WindowListener {
  final _draft = TextEditingController();
  final _historySearch = TextEditingController();
  final _scroll = ScrollController();
  double sidebar = 260;
  int tab = 0;
  bool _sending = false;
  bool _closing = false;
  static const _lifecycle = MethodChannel('gennie/lifecycle');
  String? _lastSelection;
  int _messageCount = 0;
  String _reply = '';
  int _acknowledgement = 0;
  DesktopState get state => widget.state;
  @override
  void initState() {
    super.initState();
    state.addListener(_changed);
    _changed();
    windowManager.addListener(this);
    _lifecycle.setMethodCallHandler((call) async {
      if (call.method != 'requestQuit') throw MissingPluginException();
      if (_closing) return false;
      _closing = true;
      try {
        await state.close();
        return true;
      } catch (error) {
        state.failure = 'No se pudo cerrar Gennie: $error';
        _closing = false;
        if (mounted) setState(() {});
        return false;
      }
    });
  }

  void _changed() {
    final s = state.snapshot;
    if (s == null) return;
    if (s.acknowledgement != _acknowledgement) {
      _acknowledgement = s.acknowledgement;
      if (s.acknowledgedText == _draft.text && _draft.text.isNotEmpty) {
        _draft.clear();
      }
    }
    final selected = s.selectedId;
    final changedSelection = selected != _lastSelection;
    final wasAtEnd =
        !_scroll.hasClients ||
        _scroll.position.maxScrollExtent - _scroll.offset < 70;
    final changedContent =
        _messageCount != s.messages.length || _reply != s.reply;
    _lastSelection = selected;
    _messageCount = s.messages.length;
    _reply = s.reply;
    if (changedSelection || changedContent && wasAtEnd) {
      _scrollToEnd();
    }
  }

  void _scrollToEnd() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_scroll.hasClients) return;
      final end = _scroll.position.maxScrollExtent;
      if ((_scroll.offset - end).abs() < 1) return;
      _scroll.jumpTo(end);
      // La lista estima alturas: volver a comprobar tras construir el destino.
      _scrollToEnd();
      WidgetsBinding.instance.scheduleFrame();
    });
  }

  @override
  void onWindowClose() async {
    if (Platform.isMacOS) {
      await windowManager.hide();
      return;
    }
    if (_closing) return;
    _closing = true;
    try {
      await state.close();
      await windowManager.destroy();
    } catch (e) {
      state.failure = 'No se pudo cerrar Gennie: $e';
      _closing = false;
      if (mounted) setState(() {});
    }
  }

  @override
  void dispose() {
    _lifecycle.setMethodCallHandler(null);
    windowManager.removeListener(this);
    state.removeListener(_changed);
    _draft.dispose();
    _historySearch.dispose();
    _scroll.dispose();
    super.dispose();
  }

  Future<void> _send() async {
    final s = state.snapshot;
    if (_sending ||
        _draft.text.trim().isEmpty ||
        s == null ||
        s.busy ||
        s.assistant == null ||
        s.selectedId != s.activeId) {
      return;
    }
    setState(() => _sending = true);
    await state.perform(() => state.backend.send(_draft.text));
    if (mounted) setState(() => _sending = false);
  }

  Future<void> _rename(Conversation chat) async {
    final controller = TextEditingController(text: chat.title);
    bool saving = false;
    String? error;
    await showDialog<void>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, refresh) => Dialog(
          backgroundColor: Colors.transparent,
          child: Glass(
            padding: const EdgeInsets.all(24),
            child: SizedBox(
              width: 372,
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text('Cambiar nombre', style: TextStyle(fontSize: 18)),
                  const SizedBox(height: 20),
                  TextField(
                    controller: controller,
                    autofocus: true,
                    maxLength: 200,
                    enabled: !saving,
                    decoration: InputDecoration(
                      labelText: 'Nombre del chat',
                      errorText: error,
                    ),
                  ),
                  const SizedBox(height: 16),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: [
                      TextButton(
                        onPressed: saving ? null : () => Navigator.pop(context),
                        child: const Text('Cancelar'),
                      ),
                      FilledButton(
                        onPressed: saving
                            ? null
                            : () async {
                                refresh(() {
                                  saving = true;
                                  error = null;
                                });
                                final ok = await state.perform(
                                  () => state.backend.rename(
                                    chat.id,
                                    controller.text,
                                  ),
                                );
                                if (!context.mounted) return;
                                if (ok) {
                                  Navigator.pop(context);
                                } else {
                                  refresh(() {
                                    saving = false;
                                    error = state.failure;
                                  });
                                }
                              },
                        child: Text(saving ? 'Guardando…' : 'Guardar'),
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
    // El cierre del diálogo termina su animación antes de liberar el controlador.
    await Future<void>.delayed(const Duration(milliseconds: 220));
    controller.dispose();
  }

  Future<void> _delete(Conversation chat) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Eliminar chat'),
        content: const Text(
          'Se eliminarán sus mensajes de Gennie. El hilo del proveedor se conserva.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Cancelar'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Eliminar chat'),
          ),
        ],
      ),
    );
    if (confirmed == true) {
      await state.perform(() => state.backend.delete(chat.id));
    }
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: state,
    builder: (context, _) {
      final s = state.snapshot;
      final messages = <Message>[
        ...?s?.messages,
        if (s != null && s.reply.isNotEmpty && s.selectedId == s.activeId)
          Message(speaker: 'assistant', text: s.reply, createdAt: ''),
        if (state.failure != null &&
            !(s?.errorRecorded == true && state.failure == s?.error))
          Message(
            speaker: 'system',
            text: 'Error: ${state.failure}',
            createdAt: '',
          ),
      ];
      final query = _historySearch.text.trim().toLowerCase();
      final conversations = (s?.conversations ?? <Conversation>[]).where((
        chat,
      ) {
        return query.isEmpty ||
            chat.title.toLowerCase().contains(query) ||
            (chat.lastMessage?.toLowerCase().contains(query) ?? false);
      }).toList();
      final dark = Theme.of(context).brightness == Brightness.dark;
      final ink = dark ? const Color(0xffe3e3e1) : const Color(0xff333333);
      final panelDivider = BorderSide(color: ink.withValues(alpha: .1));
      return Scaffold(
        body: Glass(
          radius: 0,
          canvas: true,
          child: LayoutBuilder(
            builder: (context, bounds) {
              final width = sidebar
                  .clamp(
                    216.0,
                    math.max(216.0, math.min(420.0, bounds.maxWidth - 380)),
                  )
                  .toDouble();
              return Stack(
                fit: StackFit.expand,
                children: [
                  // Tinte común para historial y lector; los 60 px de navegación
                  // conservan exactamente el material y la transparencia previos.
                  Positioned.fill(
                    left: 60,
                    child: IgnorePointer(
                      child: ColoredBox(
                        color:
                            (dark
                                    ? const Color(0xff26282c)
                                    : const Color(0xfff5f5f4))
                                .withValues(alpha: .68),
                      ),
                    ),
                  ),
                  Row(
                    children: [
                      ColoredBox(
                        color: Colors.black.withValues(
                          alpha: dark ? .12 : .055,
                        ),
                        child: SizedBox(
                          width: 60,
                          child: Column(
                            children: [
                              SizedBox(
                                height: 56,
                                child: Center(
                                  child: SvgPicture.asset(
                                    'assets/icons/gennie-face.svg',
                                    width: 28,
                                    height: 28,
                                    colorFilter: ColorFilter.mode(
                                      ink,
                                      BlendMode.srcIn,
                                    ),
                                  ),
                                ),
                              ),
                              Expanded(
                                child: Center(
                                  child: Column(
                                    mainAxisSize: MainAxisSize.min,
                                    children: [
                                      RailButton(
                                        label: 'Historial',
                                        asset: 'assets/icons/history.svg',
                                        icon: Icons.history_rounded,
                                        selected: tab == 0,
                                        onPressed: () =>
                                            setState(() => tab = 0),
                                      ),
                                      const SizedBox(height: 6),
                                      RailButton(
                                        label: 'Plugins',
                                        asset: 'assets/icons/plug.svg',
                                        icon: Icons.extension_outlined,
                                        selected: tab == 1,
                                        onPressed: () =>
                                            setState(() => tab = 1),
                                      ),
                                    ],
                                  ),
                                ),
                              ),
                              ModelControls(state: state),
                              const SizedBox(height: 12),
                              RailButton(
                                label: 'Configuración',
                                asset: 'assets/icons/settings.svg',
                                icon: Icons.settings_outlined,
                                onPressed: () {
                                  if (s != null) {
                                    showDialog<void>(
                                      context: context,
                                      builder: (_) =>
                                          SettingsDialog(state: state),
                                    );
                                  }
                                },
                              ),
                              const SizedBox(height: 12),
                            ],
                          ),
                        ),
                      ),
                      SizedBox(
                        width: width,
                        child: DecoratedBox(
                          decoration: BoxDecoration(
                            color: Colors.white.withValues(
                              alpha: dark ? .025 : .26,
                            ),
                          ),
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Container(
                                key: const ValueKey('sidebar-header'),
                                height: 56,
                                decoration: BoxDecoration(
                                  border: Border(bottom: panelDivider),
                                ),
                                padding: EdgeInsets.only(
                                  left: Platform.isMacOS ? 84 : 20,
                                  right: 10,
                                ),
                                child: Row(
                                  children: [
                                    Expanded(
                                      child: DragToMoveArea(
                                        child: Align(
                                          alignment: Alignment.centerLeft,
                                          child: Text(
                                            tab == 0
                                                ? 'Historial de chats'
                                                : 'Plugins',
                                            maxLines: 1,
                                            overflow: TextOverflow.ellipsis,
                                            style: const TextStyle(
                                              fontSize: 12,
                                              color: Color(0xff737373),
                                            ),
                                          ),
                                        ),
                                      ),
                                    ),
                                    if (tab == 0)
                                      AppTooltip(
                                        label: 'Nuevo chat',
                                        child: IconButton(
                                          visualDensity: VisualDensity.compact,
                                          icon: const Icon(Icons.add, size: 17),
                                          onPressed:
                                              s?.assistant != null && !s!.busy
                                              ? () => state.perform(
                                                  state.backend.newChat,
                                                )
                                              : null,
                                        ),
                                      ),
                                  ],
                                ),
                              ),
                              if (tab == 0)
                                Padding(
                                  padding: const EdgeInsets.fromLTRB(
                                    8,
                                    12,
                                    8,
                                    10,
                                  ),
                                  child: SizedBox(
                                    height: 40,
                                    child: TextField(
                                      key: const ValueKey('history-search'),
                                      textAlignVertical:
                                          TextAlignVertical.center,
                                      controller: _historySearch,
                                      onChanged: (_) => setState(() {}),
                                      style: const TextStyle(fontSize: 12),
                                      decoration: InputDecoration(
                                        hintText: 'Buscar chats…',
                                        hintStyle: TextStyle(
                                          fontSize: 12,
                                          color: ink.withValues(alpha: .5),
                                        ),
                                        filled: false,
                                        isDense: true,
                                        contentPadding:
                                            const EdgeInsets.symmetric(
                                              horizontal: 12,
                                              vertical: 10,
                                            ),
                                        prefixIcon: Icon(
                                          Icons.search_rounded,
                                          size: 17,
                                          color: ink.withValues(alpha: .55),
                                        ),
                                        prefixIconConstraints:
                                            const BoxConstraints(minWidth: 36),
                                        suffixIconConstraints:
                                            const BoxConstraints(
                                              minWidth: 36,
                                              maxHeight: 40,
                                            ),
                                        suffixIcon: _historySearch.text.isEmpty
                                            ? null
                                            : AppTooltip(
                                                label: 'Limpiar búsqueda',
                                                child: IconButton(
                                                  onPressed: () => setState(
                                                    _historySearch.clear,
                                                  ),
                                                  icon: Icon(
                                                    Icons.backspace_outlined,
                                                    size: 16,
                                                    color: ink.withValues(
                                                      alpha: .55,
                                                    ),
                                                  ),
                                                  padding: EdgeInsets.zero,
                                                  constraints:
                                                      const BoxConstraints.tightFor(
                                                        width: 36,
                                                        height: 36,
                                                      ),
                                                ),
                                              ),
                                        enabledBorder: OutlineInputBorder(
                                          borderRadius: BorderRadius.circular(
                                            8,
                                          ),
                                          borderSide: BorderSide(
                                            color: ink.withValues(alpha: .16),
                                          ),
                                        ),
                                        focusedBorder: OutlineInputBorder(
                                          borderRadius: BorderRadius.circular(
                                            8,
                                          ),
                                          borderSide: BorderSide(
                                            color: ink.withValues(alpha: .5),
                                          ),
                                        ),
                                      ),
                                    ),
                                  ),
                                ),
                              Expanded(
                                child: state.loading
                                    ? const Center(
                                        child: CircularProgressIndicator(
                                          strokeWidth: 2,
                                        ),
                                      )
                                    : tab == 1 ||
                                          s == null ||
                                          conversations.isEmpty
                                    ? Center(
                                        child: Text(
                                          tab == 0 && query.isNotEmpty
                                              ? 'No se encontraron chats'
                                              : 'No hay contenido disponible',
                                          style: TextStyle(
                                            fontSize: 12,
                                            color: Color(0xff737373),
                                          ),
                                        ),
                                      )
                                    : ListView.builder(
                                        padding: const EdgeInsets.fromLTRB(
                                          8,
                                          0,
                                          8,
                                          0,
                                        ),
                                        itemCount: conversations.length,
                                        itemBuilder: (context, index) {
                                          final chat = conversations[index];
                                          return Padding(
                                            padding: const EdgeInsets.only(
                                              bottom: 2,
                                            ),
                                            child: ChatHistoryTile(
                                              chat: chat,
                                              selected: s.selectedId == chat.id,
                                              onSelect: () => state.perform(
                                                () => state.backend.select(
                                                  chat.id,
                                                ),
                                              ),
                                              onRename: () => _rename(chat),
                                              onDelete: () => _delete(chat),
                                            ),
                                          );
                                        },
                                      ),
                              ),
                            ],
                          ),
                        ),
                      ),
                      Expanded(
                        child: Column(
                          children: [
                            Container(
                              key: const ValueKey('chat-header'),
                              height: 56,
                              decoration: BoxDecoration(
                                color: Colors.white.withValues(
                                  alpha: dark ? .025 : .26,
                                ),
                                border: Border(bottom: panelDivider),
                              ),
                              child: Row(
                                children: [
                                  Expanded(
                                    child: DragToMoveArea(
                                      child: Padding(
                                        padding: const EdgeInsets.symmetric(
                                          horizontal: 24,
                                        ),
                                        child: Row(
                                          children: [
                                            if (tab == 0 &&
                                                (s?.conversations.any(
                                                      (chat) =>
                                                          chat.id ==
                                                          s.selectedId,
                                                    ) ??
                                                    false)) ...[
                                              SvgPicture.asset(
                                                'assets/pets/penguin.svg',
                                                width: 48,
                                                height: 48,
                                                semanticsLabel:
                                                    'Gennie, avatar de pingüino',
                                              ),
                                              const SizedBox(width: 10),
                                            ],
                                            Flexible(
                                              child: ConstrainedBox(
                                                constraints:
                                                    const BoxConstraints(
                                                      maxWidth: 320,
                                                    ),
                                                child: Text(
                                                  _title(s),
                                                  maxLines: 1,
                                                  overflow:
                                                      TextOverflow.ellipsis,
                                                  style: const TextStyle(
                                                    fontSize: 14,
                                                    fontWeight: FontWeight.w500,
                                                  ),
                                                ),
                                              ),
                                            ),
                                          ],
                                        ),
                                      ),
                                    ),
                                  ),
                                  AppTooltip(
                                    label: 'Opciones del sistema',
                                    child: PopupMenuButton<String>(
                                      tooltip: '',
                                      icon: const Icon(
                                        Icons.more_horiz,
                                        size: 20,
                                      ),
                                      onSelected: (action) {
                                        if (action == 'settings') {
                                          showDialog<void>(
                                            context: context,
                                            builder: (_) =>
                                                SettingsDialog(state: state),
                                          );
                                        }
                                      },
                                      itemBuilder: (_) => [
                                        if (s != null)
                                          PopupMenuItem<String>(
                                            enabled: false,
                                            child: Text(s.connection),
                                          ),
                                        PopupMenuItem(
                                          value: 'settings',
                                          enabled: s != null,
                                          child: const Text('Configuración'),
                                        ),
                                      ],
                                    ),
                                  ),
                                  const SizedBox(width: 12),
                                ],
                              ),
                            ),
                            Expanded(
                              child: Column(
                                children: [
                                  Expanded(
                                    child: ChatViewport(
                                      enabled: tab == 0,
                                      child: tab == 1 || messages.isEmpty
                                          ? _empty(s)
                                          : Align(
                                              alignment: Alignment.topCenter,
                                              child: ConstrainedBox(
                                                constraints:
                                                    const BoxConstraints(
                                                      maxWidth: 840,
                                                    ),
                                                child: SelectionArea(
                                                  child: ListView.builder(
                                                    controller: _scroll,
                                                    padding:
                                                        const EdgeInsets.fromLTRB(
                                                          32,
                                                          24,
                                                          32,
                                                          24,
                                                        ),
                                                    itemCount: messages.length,
                                                    itemBuilder: (context, index) {
                                                      final message =
                                                          messages[index];
                                                      final day =
                                                          messageLocalDay(
                                                            message.createdAt,
                                                          );
                                                      final previousDay =
                                                          index > 0
                                                          ? messageLocalDay(
                                                              messages[index -
                                                                      1]
                                                                  .createdAt,
                                                            )
                                                          : null;
                                                      return Column(
                                                        children: [
                                                          if (day != null &&
                                                              day !=
                                                                  previousDay)
                                                            ChatDayBadge(
                                                              label: chatDayLabel(
                                                                message
                                                                    .createdAt,
                                                                now:
                                                                    DateTime.now(),
                                                              ),
                                                            ),
                                                          ChatMessage(
                                                            message: message,
                                                          ),
                                                        ],
                                                      );
                                                    },
                                                  ),
                                                ),
                                              ),
                                            ),
                                    ),
                                  ),
                                  if (tab == 0 && s == null && !state.loading)
                                    TextButton(
                                      onPressed: state.start,
                                      child: const Text('Reintentar conexión'),
                                    ),
                                  if (tab == 0 && s != null) _composer(s),
                                ],
                              ),
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                  Positioned(
                    left: 60 + width - panelDivider.width,
                    top: 56,
                    bottom: 0,
                    width: panelDivider.width,
                    child: IgnorePointer(
                      child: ColoredBox(color: panelDivider.color),
                    ),
                  ),
                  Positioned(
                    left: 60 + width - 4,
                    top: 0,
                    bottom: 0,
                    width: 8,
                    child: Semantics(
                      label: 'Ancho de la sidebar',
                      slider: true,
                      onIncrease: () => setState(
                        () => sidebar = (sidebar + 16).clamp(216, 420),
                      ),
                      onDecrease: () => setState(
                        () => sidebar = (sidebar - 16).clamp(216, 420),
                      ),
                      child: MouseRegion(
                        cursor: SystemMouseCursors.resizeColumn,
                        child: GestureDetector(
                          behavior: HitTestBehavior.opaque,
                          onHorizontalDragUpdate: (d) => setState(
                            () =>
                                sidebar = (width + d.delta.dx).clamp(216, 420),
                          ),
                          child: const SizedBox(
                            width: 8,
                            height: double.infinity,
                          ),
                        ),
                      ),
                    ),
                  ),
                ],
              );
            },
          ),
        ),
      );
    },
  );
  String _title(Snapshot? s) {
    if (tab == 1) return 'Plugins';
    for (final chat in s?.conversations ?? <Conversation>[]) {
      if (chat.id == s?.selectedId) return chat.title;
    }
    return 'Gennie';
  }

  Widget _empty(Snapshot? s) => Center(
    child: Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            tab == 1 ? 'Plugins' : '¿En qué te ayudo?',
            style: const TextStyle(
              fontSize: 26,
              fontWeight: FontWeight.w500,
              letterSpacing: -.7,
            ),
          ),
          const SizedBox(height: 12),
          Text(
            tab == 1
                ? 'No hay contenido disponible'
                : 'Un espacio para pensar, conversar y hacer.',
            style: const TextStyle(fontSize: 13, color: Color(0xff737373)),
          ),
          if (s == null && !state.loading)
            TextButton(
              onPressed: state.start,
              child: const Text('Reintentar conexión'),
            ),
        ],
      ),
    ),
  );
  Widget _composer(Snapshot s) {
    final readonly = s.selectedId != s.activeId;
    if (readonly) {
      return Padding(
        padding: const EdgeInsets.all(20),
        child: TextButton.icon(
          onPressed: () =>
              state.perform(() => state.backend.select(s.activeId)),
          icon: const Icon(Icons.arrow_forward, size: 16),
          label: const Text('Volver al chat activo'),
        ),
      );
    }
    return Padding(
      padding: const EdgeInsets.fromLTRB(24, 8, 24, 22),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 800),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            DecoratedBox(
              decoration: BoxDecoration(
                color: Colors.grey.withValues(alpha: .08),
                borderRadius: BorderRadius.circular(18),
              ),
              child: Padding(
                padding: const EdgeInsets.fromLTRB(18, 6, 8, 6),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    Expanded(
                      child: CallbackShortcuts(
                        bindings: {
                          const SingleActivator(LogicalKeyboardKey.enter):
                              _send,
                        },
                        child: TextField(
                          controller: _draft,
                          minLines: 1,
                          maxLines: 6,
                          style: const TextStyle(fontSize: 14),
                          decoration: const InputDecoration(
                            hintText: 'Escribe a Gennie…',
                            filled: false,
                            border: InputBorder.none,
                            enabledBorder: InputBorder.none,
                            focusedBorder: InputBorder.none,
                          ),
                        ),
                      ),
                    ),
                    AppTooltip(
                      label: 'Enviar mensaje',
                      child: IconButton(
                        onPressed: s.busy || _sending || s.assistant == null
                            ? null
                            : _send,
                        icon: s.busy
                            ? const SizedBox(
                                width: 18,
                                height: 18,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : const Icon(Icons.arrow_upward_rounded, size: 21),
                      ),
                    ),
                  ],
                ),
              ),
            ),
            const SizedBox(height: 9),
            Text(
              s.busy
                  ? 'Gennie está respondiendo…'
                  : 'Intro para enviar · Mayús + Intro para otra línea',
              style: const TextStyle(fontSize: 10, color: Color(0xff858585)),
            ),
          ],
        ),
      ),
    );
  }
}
