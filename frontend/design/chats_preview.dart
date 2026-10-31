import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/chat_message.dart';
import 'package:gennie/src/glass.dart';
import 'package:gennie/src/sidebar.dart';

// Prototipo aislado: no accede a Rust, SQLite, modelos ni cuentas.
class ChatsPreview extends StatefulWidget {
  const ChatsPreview({super.key});
  @override
  State<ChatsPreview> createState() => _ChatsPreviewState();
}

class _ChatsPreviewState extends State<ChatsPreview> {
  final search = TextEditingController();
  final draft = TextEditingController();
  final composerFocus = FocusNode();
  String name = 'Gennie';
  String? quote;
  bool fresh = false;
  final sent = <String>[];

  @override
  void dispose() {
    search.dispose();
    draft.dispose();
    composerFocus.dispose();
    super.dispose();
  }

  Widget avatar(double size) => ExcludeSemantics(
    child: SvgPicture.asset(
      'assets/pets/penguin.svg',
      width: size,
      height: size,
    ),
  );

  Widget action(String label, IconData icon, VoidCallback callback) =>
      AppTooltip(
        label: label,
        child: IconButton(
          onPressed: callback,
          icon: Icon(icon, size: 18, semanticLabel: label),
        ),
      );

  Future<void> newConversation() async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('¿Iniciar otra conversación?'),
        content: const Text(
          'Gennie comenzará con un contexto nuevo. Tus mensajes anteriores '
          'seguirán disponibles en este chat.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Cancelar'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Iniciar conversación'),
          ),
        ],
      ),
    );
    if (!mounted || confirmed != true) return;
    setState(() {
      fresh = true;
      quote = null;
    });
    composerFocus.requestFocus();
  }

  Future<void> personalize() async {
    final controller = TextEditingController(text: name);
    final result = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Personalizar asistente'),
        content: TextField(
          controller: controller,
          autofocus: true,
          maxLength: 200,
          decoration: const InputDecoration(labelText: 'Nombre del asistente'),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Cancelar'),
          ),
          TextButton(
            onPressed: () {
              if (controller.text.trim().isNotEmpty) {
                Navigator.pop(context, controller.text.trim());
              }
            },
            child: const Text('Guardar'),
          ),
        ],
      ),
    );
    // El diálogo conserva el controlador hasta terminar su animación de cierre.
    await Future<void>.delayed(const Duration(milliseconds: 300));
    controller.dispose();
    if (mounted && result != null) setState(() => name = result);
  }

  void send() {
    if (draft.text.trim().isEmpty) return;
    setState(() {
      sent.add(draft.text.trim());
      draft.clear();
      quote = null;
    });
  }

  Widget message(String text, {bool user = false, String? plugin}) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      ChatMessage(
        message: Message(
          speaker: user ? 'user' : 'assistant',
          text: text,
          createdAt: '2026-10-28T19:20:00Z',
        ),
      ),
      Row(
        mainAxisAlignment: user
            ? MainAxisAlignment.end
            : MainAxisAlignment.start,
        children: [
          if (plugin != null)
            Flexible(
              child: TextButton.icon(
                onPressed: () => showDialog<void>(
                  context: context,
                  builder: (context) => AlertDialog(
                    title: Text(plugin),
                    content: const Text(
                      'Consulta completada · solo lectura.\nDatos de ejemplo del prototipo.',
                    ),
                    actions: [
                      TextButton(
                        onPressed: () => Navigator.pop(context),
                        child: const Text('Cerrar'),
                      ),
                    ],
                  ),
                ),
                icon: const Icon(Icons.extension_outlined, size: 14),
                label: Text('Usó: $plugin'),
              ),
            ),
          action('Responder mensaje', Icons.reply_rounded, () {
            setState(() => quote = text);
            composerFocus.requestFocus();
          }),
        ],
      ),
      const SizedBox(height: 12),
    ],
  );

  @override
  Widget build(BuildContext context) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    final ink = dark ? const Color(0xffeeeeec) : const Color(0xff24262c);
    final secondary = dark ? const Color(0xffb8b9bb) : const Color(0xff5b5e63);
    final surface = dark ? const Color(0xff26282c) : const Color(0xfff5f5f4);
    return Scaffold(
      backgroundColor: surface,
      body: Row(
        children: [
          ColoredBox(
            color: Colors.black.withValues(alpha: dark ? .12 : .055),
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
                        colorFilter: ColorFilter.mode(ink, BlendMode.srcIn),
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
                            selected: true,
                            onPressed: () {},
                          ),
                          const SizedBox(height: 6),
                          RailButton(
                            label: 'Encargos',
                            asset: 'assets/icons/assignments.svg',
                            icon: Icons.event_outlined,
                            onPressed: () {},
                          ),
                          const SizedBox(height: 6),
                          RailButton(
                            label: 'Plugins',
                            asset: 'assets/icons/plug.svg',
                            icon: Icons.extension_outlined,
                            onPressed: () {},
                          ),
                        ],
                      ),
                    ),
                  ),
                  for (final model in [
                    ('Laya', 'brain', Icons.psychology_outlined),
                    ('Voz · Kokoro', 'speaker', Icons.volume_up_outlined),
                    ('Transcripción · Nemotron', 'microphone', Icons.mic_none),
                  ]) ...[
                    RailButton(
                      label: model.$1,
                      asset: 'assets/icons/${model.$2}.svg',
                      icon: model.$3,
                      appearance: RailButtonAppearance.modelStatus,
                      statusColor: const Color(0xff858585),
                      onPressed: null,
                    ),
                    if (model.$2 != 'microphone') const SizedBox(height: 6),
                  ],
                  const SizedBox(height: 12),
                  RailButton(
                    label: 'Configuración',
                    asset: 'assets/icons/settings.svg',
                    icon: Icons.settings_outlined,
                    onPressed: personalize,
                  ),
                  const SizedBox(height: 12),
                ],
              ),
            ),
          ),
          LayoutBuilder(
            builder: (context, constraints) => SizedBox(
              width: MediaQuery.sizeOf(context).width < 800 ? 216 : 260,
              child: AppSidebar(
                width: MediaQuery.sizeOf(context).width < 800 ? 216 : 260,
                title: 'Chats',
                searchController: search,
                searchHint: 'Buscar asistentes…',
                onSearch: (_) => setState(() {}),
                child:
                    search.text.isNotEmpty &&
                        !name.toLowerCase().contains(search.text.toLowerCase())
                    ? const SidebarEmpty(
                        message: 'No se encontraron asistentes',
                      )
                    : Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 8),
                        child: Column(
                          children: [
                            SidebarTile(
                              title: name,
                              subtitle: 'Te encontré dos opciones.',
                              selected: true,
                              leading: avatar(38),
                              trailing: Text(
                                'Hoy',
                                style: TextStyle(
                                  fontSize: 10,
                                  color: secondary,
                                ),
                              ),
                              onSelect: () {},
                            ),
                          ],
                        ),
                      ),
              ),
            ),
          ),
          Expanded(
            child: Column(
              children: [
                Container(
                  height: 56,
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  decoration: BoxDecoration(
                    border: Border(
                      bottom: BorderSide(color: ink.withValues(alpha: .1)),
                    ),
                  ),
                  child: Row(
                    children: [
                      avatar(42),
                      const SizedBox(width: 10),
                      Expanded(
                        child: Text(
                          name,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                            fontSize: 14,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                      ),
                      action(
                        'Nueva conversación',
                        Icons.add_comment_outlined,
                        newConversation,
                      ),
                      action(
                        'Personalizar asistente',
                        Icons.more_horiz_rounded,
                        personalize,
                      ),
                    ],
                  ),
                ),
                Expanded(
                  child: ListView(
                    key: const ValueKey('preview-history'),
                    reverse: true,
                    padding: const EdgeInsets.fromLTRB(20, 20, 20, 24),
                    children: [
                      Center(
                        child: Text(
                          'Hoy',
                          style: TextStyle(fontSize: 11, color: secondary),
                        ),
                      ),
                      const SizedBox(height: 20),
                      message(
                        'Busca dos opciones para organizar mis pendientes.',
                        user: true,
                      ),
                      message(
                        'Te encontré dos opciones. Podemos empezar por lo que vence esta semana.',
                        plugin: 'Recordatorios',
                      ),
                      if (fresh) ...[
                        const Divider(),
                        Padding(
                          padding: const EdgeInsets.symmetric(vertical: 12),
                          child: Text(
                            'Nueva conversación · contexto nuevo',
                            textAlign: TextAlign.center,
                            style: TextStyle(fontSize: 12, color: secondary),
                          ),
                        ),
                      ],
                      for (final text in sent) message(text, user: true),
                    ].reversed.toList(),
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 8, 16, 16),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (quote != null)
                        Container(
                          padding: const EdgeInsets.only(left: 12),
                          decoration: BoxDecoration(
                            border: Border(
                              left: BorderSide(color: secondary, width: 3),
                            ),
                            color: ink.withValues(alpha: .06),
                          ),
                          child: Row(
                            children: [
                              Expanded(
                                child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    const Text(
                                      'Respondiendo a un mensaje',
                                      style: TextStyle(
                                        fontSize: 11,
                                        fontWeight: FontWeight.w600,
                                      ),
                                    ),
                                    Text(
                                      quote!,
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: const TextStyle(fontSize: 12),
                                    ),
                                  ],
                                ),
                              ),
                              action(
                                'Cancelar respuesta',
                                Icons.close_rounded,
                                () => setState(() => quote = null),
                              ),
                            ],
                          ),
                        ),
                      Container(
                        padding: const EdgeInsets.fromLTRB(14, 4, 4, 4),
                        decoration: BoxDecoration(
                          color: Colors.grey.withValues(alpha: .14),
                          borderRadius: BorderRadius.circular(18),
                        ),
                        child: Row(
                          crossAxisAlignment: CrossAxisAlignment.center,
                          children: [
                            Expanded(
                              child: TextField(
                                controller: draft,
                                focusNode: composerFocus,
                                minLines: 1,
                                maxLines: 4,
                                onChanged: (_) => setState(() {}),
                                decoration: const InputDecoration(
                                  hintText: 'Escribe un mensaje…',
                                  border: InputBorder.none,
                                  enabledBorder: InputBorder.none,
                                  focusedBorder: InputBorder.none,
                                  filled: false,
                                ),
                              ),
                            ),
                            AppTooltip(
                              label: 'Enviar',
                              child: IconButton(
                                onPressed: draft.text.trim().isEmpty
                                    ? null
                                    : send,
                                icon: const Icon(
                                  Icons.arrow_upward_rounded,
                                  size: 20,
                                  semanticLabel: 'Enviar',
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
