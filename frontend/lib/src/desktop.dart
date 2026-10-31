import 'dart:async';
import 'dart:math' as math;
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:window_manager/window_manager.dart';
import 'backend.dart';
import 'assistant_avatar.dart';
import 'assistant_profile.dart';
import 'chat_history.dart';
import 'chat_viewport.dart';
import 'chat_timeline.dart';
import 'message_quote.dart';
import 'glass.dart';
import 'settings.dart';
import 'model_controls.dart';
import 'assignments.dart';
import 'plugins.dart';
import 'sidebar.dart';
import 'architects.dart';

class DesktopScreen extends StatefulWidget {
  const DesktopScreen({super.key, required this.state});
  final DesktopState state;
  @override
  State<DesktopScreen> createState() => _DesktopScreenState();
}

class _DesktopScreenState extends State<DesktopScreen> with WindowListener {
  final _draft = TextEditingController();
  final _composerFocus = FocusNode();
  ChatEntry? _replySource;
  String? _submittedReplyId;
  final _historySearch = TextEditingController();
  final _scroll = ScrollController();
  double sidebar = 260;
  int tab = 0;
  String? _pluginToOpen;
  bool _sending = false;
  bool _closing = false;
  bool _openingBrowser = false;
  bool _architectSaving = false;
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
      if (s.acknowledgedText == _draft.text &&
          _draft.text.isNotEmpty &&
          _replySource?.id == _submittedReplyId) {
        _draft.clear();
        _replySource = null;
      }
    }
    final selected = s.selectedId;
    final changedSelection = selected != _lastSelection;
    final wasAtEnd =
        !_scroll.hasClients ||
        _scroll.offset - _scroll.position.minScrollExtent < 70;
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
      final end = _scroll.position.minScrollExtent;
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
    _composerFocus.dispose();
    _historySearch.dispose();
    _scroll.dispose();
    super.dispose();
  }

  String? _stopRequested;
  void _insertNewline() {
    final value = _draft.value;
    final selection = value.selection;
    final start = selection.isValid ? selection.start : value.text.length;
    final end = selection.isValid ? selection.end : value.text.length;
    _draft.value = TextEditingValue(
      text: value.text.replaceRange(start, end, '\n'),
      selection: TextSelection.collapsed(offset: start + 1),
    );
  }

  Future<void> _stopResponse() async {
    final progress = state.snapshot?.turn;
    if (progress == null ||
        progress.stopping ||
        _stopRequested == progress.id) {
      return;
    }
    final id = progress.id;
    setState(() => _stopRequested = id);
    final stopped = await state.stopResponse(id);
    if (!mounted) return;
    if (!stopped && _stopRequested == id) setState(() => _stopRequested = null);
    if (state.snapshot?.turn?.id == id || state.snapshot?.turn == null) {
      _composerFocus.requestFocus();
    }
  }

  Future<void> _send() async {
    final s = state.snapshot;
    if (_sending ||
        _draft.text.trim().isEmpty ||
        s == null ||
        s.busy ||
        s.assistant == null) {
      return;
    }
    setState(() => _sending = true);
    final source = _replySource;
    _submittedReplyId = source?.id;
    await state.perform(
      () => source == null
          ? state.backend.send(_draft.text)
          : (state.backend as AssistantChatBackend).sendReply(
              _draft.text,
              source.id,
            ),
    );
    if (mounted) setState(() => _sending = false);
  }

  Future<void> _renameAssistant(AssistantProfile profile) async {
    await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (_) => AssistantProfileDialog(state: state, profile: profile),
    );
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: state,
    builder: (context, _) {
      final s = state.snapshot;
      final query = _historySearch.text.trim().toLowerCase();
      final profile = s?.assistantProfile;
      final lastConversation = s?.conversations
          .where((c) => c.lastMessage != null)
          .firstOrNull;
      final matches =
          profile != null &&
          (query.isEmpty || profile.name.toLowerCase().contains(query));
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
                                        label: 'Chats',
                                        asset: 'assets/icons/history.svg',
                                        icon: Icons.history_rounded,
                                        selected: tab == 0,
                                        onPressed: _architectSaving
                                            ? null
                                            : () => setState(() => tab = 0),
                                      ),
                                      const SizedBox(height: 6),
                                      RailButton(
                                        label: 'Encargos',
                                        asset: 'assets/icons/assignments.svg',
                                        icon: Icons.event_outlined,
                                        selected: tab == 2,
                                        onPressed: _architectSaving
                                            ? null
                                            : () => setState(() => tab = 2),
                                      ),
                                      const SizedBox(height: 6),
                                      RailButton(
                                        label: 'Plugins',
                                        asset: 'assets/icons/plug.svg',
                                        icon: Icons.extension_outlined,
                                        selected: tab == 1,
                                        onPressed: _architectSaving
                                            ? null
                                            : () => setState(() => tab = 1),
                                      ),
                                      const SizedBox(height: 6),
                                      RailButton(
                                        label: 'Arquitectos',
                                        asset: 'assets/icons/architects.svg',
                                        icon: Icons.account_tree_outlined,
                                        selected: tab == 3,
                                        onPressed: _architectSaving
                                            ? null
                                            : () => setState(() => tab = 3),
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
                                onPressed: _architectSaving
                                    ? null
                                    : () {
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
                      if (tab == 3)
                        Expanded(
                          child: ArchitectsView(
                            state: state,
                            sidebarWidth: width,
                            onSaving: (value) =>
                                setState(() => _architectSaving = value),
                          ),
                        ),
                      if (tab == 2)
                        Expanded(
                          child: AssignmentsView(
                            state: state,
                            sidebarWidth: width,
                            onOpenChat: () => setState(() => tab = 0),
                            onOpenPlugin: (id) => setState(() {
                              _pluginToOpen = id;
                              tab = 1;
                            }),
                          ),
                        )
                      else if (tab == 1)
                        Expanded(
                          child: PluginsView(
                            state: state,
                            sidebarWidth: width,
                            initialSelection: _pluginToOpen,
                            onSelectionChanged: (id) => _pluginToOpen = id,
                          ),
                        )
                      else if (tab == 0) ...[
                        AppSidebar(
                          width: width,
                          title: 'Chats',
                          actionLabel: 'Nueva conversación',
                          onAction: s?.assistant != null && !s!.busy
                              ? () => state.perform(state.backend.newChat)
                              : null,
                          searchController: _historySearch,
                          searchHint: 'Buscar asistentes…',
                          searchKey: const ValueKey('history-search'),
                          onSearch: (_) => setState(() {}),
                          child: state.loading
                              ? const Center(
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : !matches
                              ? SidebarEmpty(
                                  message: query.isNotEmpty
                                      ? 'No se encontraron asistentes'
                                      : 'No hay contenido disponible',
                                )
                              : Padding(
                                  padding: const EdgeInsets.symmetric(
                                    horizontal: 8,
                                  ),
                                  child: Column(
                                    children: [
                                      SidebarTile(
                                        title: profile.name,
                                        subtitle:
                                            lastConversation?.lastMessage
                                                ?.replaceAll(
                                                  RegExp(r'\s+'),
                                                  ' ',
                                                )
                                                .trim() ??
                                            '',
                                        selected: true,
                                        onSelect: () {},
                                        leading: AssistantAvatar(
                                          state: state,
                                          profile: profile,
                                          size: 38,
                                        ),
                                        trailing: Text(
                                          conversationDateLabel(
                                            lastConversation?.updatedAt ?? '',
                                            now: DateTime.now(),
                                          ),
                                          style: const TextStyle(fontSize: 10),
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
                                                  (s?.assistantProfile !=
                                                          null ||
                                                      (s?.conversations.any(
                                                            (chat) =>
                                                                chat.id ==
                                                                s.selectedId,
                                                          ) ??
                                                          false))) ...[
                                                AssistantAvatar(
                                                  state: state,
                                                  profile: s?.assistantProfile,
                                                  size: 48,
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
                                                      fontWeight:
                                                          FontWeight.w500,
                                                    ),
                                                  ),
                                                ),
                                              ),
                                            ],
                                          ),
                                        ),
                                      ),
                                    ),
                                    if (tab == 0)
                                      AppTooltip(
                                        label: 'Abrir Gennie Web',
                                        child: MergeSemantics(
                                          child: Semantics(
                                            label: 'Abrir Gennie Web',
                                            button: true,
                                            child: IconButton(
                                              key: const ValueKey(
                                                'open-browser',
                                              ),
                                              onPressed:
                                                  s == null || _openingBrowser
                                                  ? null
                                                  : () async {
                                                      setState(
                                                        () => _openingBrowser =
                                                            true,
                                                      );
                                                      await state.perform(
                                                        state
                                                            .backend
                                                            .openBrowser,
                                                      );
                                                      if (mounted) {
                                                        setState(
                                                          () =>
                                                              _openingBrowser =
                                                                  false,
                                                        );
                                                      }
                                                    },
                                              icon: _openingBrowser
                                                  ? const SizedBox(
                                                      width: 18,
                                                      height: 18,
                                                      child:
                                                          CircularProgressIndicator(
                                                            strokeWidth: 1.5,
                                                          ),
                                                    )
                                                  : SvgPicture.asset(
                                                      'assets/icons/browser.svg',
                                                      width: 20,
                                                      height: 20,
                                                      colorFilter:
                                                          ColorFilter.mode(
                                                            Theme.of(context)
                                                                .colorScheme
                                                                .onSurface,
                                                            BlendMode.srcIn,
                                                          ),
                                                    ),
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
                                          semanticLabel: 'Opciones del sistema',
                                        ),
                                        onSelected: (action) {
                                          if (action == 'profile' &&
                                              s?.assistantProfile != null) {
                                            final profile =
                                                s!.assistantProfile!;
                                            _renameAssistant(profile);
                                          }
                                          if (action == 'settings') {
                                            showDialog<void>(
                                              context: context,
                                              builder: (_) =>
                                                  SettingsDialog(state: state),
                                            );
                                          }
                                        },
                                        itemBuilder: (_) => [
                                          if (s?.assistantProfile != null)
                                            PopupMenuItem(
                                              value: 'profile',
                                              enabled:
                                                  state.backend
                                                      is AssistantChatBackend,
                                              child: const Text(
                                                'Personalizar asistente',
                                              ),
                                            ),
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
                                        child: s == null
                                            ? _empty(s)
                                            : ChatTimeline(
                                                state: state,
                                                controller: _scroll,
                                                onReply: (entry) {
                                                  setState(
                                                    () => _replySource = entry,
                                                  );
                                                  _composerFocus.requestFocus();
                                                },
                                                onOpenPlugin: (id) =>
                                                    setState(() {
                                                      _pluginToOpen = id;
                                                      tab = 1;
                                                    }),
                                              ),
                                      ),
                                    ),
                                    if (tab == 0 && s == null && !state.loading)
                                      TextButton(
                                        onPressed: state.start,
                                        child: const Text(
                                          'Reintentar conexión',
                                        ),
                                      ),
                                    if (tab == 0 && s != null) _composer(s),
                                  ],
                                ),
                              ),
                            ],
                          ),
                        ),
                      ],
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
    if (s?.assistantProfile != null) return s!.assistantProfile!.name;
    return 'Gennie';
  }

  Widget _empty(Snapshot? s) => ContentEmpty(
    title: tab == 1 ? 'Plugins' : '¿En qué te ayudo?',
    message: tab == 1
        ? 'No hay contenido disponible'
        : 'Un espacio para pensar, conversar y hacer.',
    action: s == null && !state.loading
        ? TextButton(
            onPressed: state.start,
            child: const Text('Reintentar conexión'),
          )
        : null,
  );
  Widget _composer(Snapshot s) {
    final canStop = s.turn != null && state.backend is TurnCancellationBackend;
    final stopping =
        canStop && (s.turn!.stopping || _stopRequested == s.turn!.id);
    return Padding(
      padding: const EdgeInsets.fromLTRB(24, 8, 24, 22),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 800),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (_replySource != null)
              Padding(
                padding: const EdgeInsets.only(bottom: 8),
                child: Row(
                  children: [
                    Expanded(
                      child: MessageQuote(
                        quote: ReplyQuote(
                          sourceMessageId: _replySource!.id,
                          speaker: _replySource!.message.speaker,
                          text: String.fromCharCodes(
                            _replySource!.message.text.runes.take(4096),
                          ),
                          createdAt: _replySource!.message.createdAt,
                          truncated:
                              _replySource!.message.text.runes.length > 4096,
                        ),
                        assistantName: s.assistantProfile?.name ?? 'Gennie',
                      ),
                    ),
                    AppTooltip(
                      label: 'Cancelar respuesta',
                      child: IconButton(
                        onPressed: () {
                          setState(() => _replySource = null);
                          _composerFocus.requestFocus();
                        },
                        icon: const Icon(
                          Icons.close,
                          size: 18,
                          semanticLabel: 'Cancelar respuesta',
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            DecoratedBox(
              decoration: BoxDecoration(
                color: Colors.grey.withValues(alpha: .14),
                borderRadius: BorderRadius.circular(18),
              ),
              child: Padding(
                padding: const EdgeInsets.fromLTRB(18, 6, 8, 6),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.center,
                  children: [
                    Expanded(
                      child: CallbackShortcuts(
                        bindings: {
                          const SingleActivator(LogicalKeyboardKey.enter):
                              _send,
                          const SingleActivator(LogicalKeyboardKey.numpadEnter):
                              _send,
                          const SingleActivator(
                            LogicalKeyboardKey.enter,
                            shift: true,
                          ): _insertNewline,
                          const SingleActivator(
                            LogicalKeyboardKey.numpadEnter,
                            shift: true,
                          ): _insertNewline,
                        },
                        child: Semantics(
                          label:
                              'Mensaje para ${s.assistantProfile?.name ?? 'Gennie'}',
                          child: TextField(
                            controller: _draft,
                            focusNode: _composerFocus,
                            minLines: 1,
                            maxLines: 6,
                            style: const TextStyle(fontSize: 14),
                            decoration: InputDecoration(
                              hint: ExcludeSemantics(
                                child: Text(
                                  'Escribe a Gennie…',
                                  style: Theme.of(context).textTheme.bodyLarge!
                                      .copyWith(
                                        fontSize: 14,
                                        color: Theme.of(
                                          context,
                                        ).colorScheme.onSurfaceVariant,
                                      ),
                                ),
                              ),
                              filled: false,
                              border: InputBorder.none,
                              enabledBorder: InputBorder.none,
                              focusedBorder: InputBorder.none,
                            ),
                          ),
                        ),
                      ),
                    ),
                    AppTooltip(
                      label: canStop
                          ? (stopping
                                ? 'Deteniendo respuesta'
                                : 'Detener respuesta')
                          : 'Enviar mensaje',
                      child: IconButton(
                        key: ValueKey(
                          canStop ? 'stop-response' : 'send-message',
                        ),
                        onPressed: canStop
                            ? (stopping ? null : _stopResponse)
                            : (s.busy || _sending || s.assistant == null
                                  ? null
                                  : _send),
                        icon: Semantics(
                          label: canStop
                              ? (stopping
                                    ? 'Deteniendo respuesta'
                                    : 'Detener respuesta')
                              : 'Enviar mensaje',
                          excludeSemantics: true,
                          child: canStop && !stopping
                              ? const Icon(Icons.stop_rounded, size: 21)
                              : s.busy
                              ? const SizedBox(
                                  width: 18,
                                  height: 18,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : const Icon(
                                  Icons.arrow_upward_rounded,
                                  size: 21,
                                ),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
            const SizedBox(height: 9),
            Text(
              stopping
                  ? 'Deteniendo respuesta…'
                  : s.busy
                  ? 'Gennie está respondiendo…'
                  : s.confirmation == 'Respuesta detenida'
                  ? 'Respuesta detenida · Intro para enviar'
                  : 'Intro para enviar · Mayús + Intro para otra línea',
              style: TextStyle(
                fontSize: 10,
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
