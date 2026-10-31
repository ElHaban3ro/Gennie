import 'dart:typed_data';
import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'assistant_avatar.dart';
import 'backend.dart';
import 'glass.dart';

const maxAvatarBytes = 5 * 1024 * 1024;
Future<Uint8List?> pickAssistantImage() async {
  final file = await openFile(
    acceptedTypeGroups: const [
      XTypeGroup(
        label: 'Imágenes',
        extensions: ['png', 'jpg', 'jpeg', 'webp'],
        uniformTypeIdentifiers: [
          'public.png',
          'public.jpeg',
          'org.webmproject.webp',
        ],
      ),
    ],
  );
  if (file == null) return null;
  if (await file.length() > maxAvatarBytes) {
    throw StateError('La imagen debe ocupar como máximo 5 MiB');
  }
  final builder = BytesBuilder(copy: false);
  await for (final chunk in file.openRead(0, maxAvatarBytes + 1)) {
    builder.add(chunk);
    if (builder.length > maxAvatarBytes) {
      throw StateError('La imagen debe ocupar como máximo 5 MiB');
    }
  }
  return builder.takeBytes();
}

class AssistantProfileDialog extends StatefulWidget {
  const AssistantProfileDialog({
    super.key,
    required this.state,
    required this.profile,
    this.pickImage = pickAssistantImage,
  });
  final DesktopState state;
  final AssistantProfile profile;
  final Future<Uint8List?> Function() pickImage;
  @override
  State<AssistantProfileDialog> createState() => _AssistantProfileDialogState();
}

class _AssistantProfileDialogState extends State<AssistantProfileDialog> {
  late final name = TextEditingController(text: widget.profile.name);
  final scroll = ScrollController();
  final nameFocus = FocusNode();
  final nameRegion = GlobalKey();
  Uint8List? avatar;
  bool busy = false;
  bool preparing = false;
  String? error;
  @override
  void dispose() {
    name.dispose();
    nameFocus.dispose();
    scroll.dispose();
    super.dispose();
  }

  Future<void> choose() async {
    setState(() {
      busy = true;
      preparing = true;
      error = null;
    });
    try {
      final selected = await widget.pickImage();
      if (selected != null && mounted) {
        final prepared = await (widget.state.backend as AssistantProfileBackend)
            .prepareAvatar(selected);
        if (mounted) setState(() => avatar = prepared);
      }
    } catch (failure) {
      if (mounted) {
        setState(
          () => error =
              'No se pudo preparar la imagen. ${failure is StateError ? failure.message : failure}',
        );
      }
    } finally {
      if (mounted) {
        setState(() {
          busy = false;
          preparing = false;
        });
      }
    }
  }

  Future<void> save() async {
    if (busy) return;
    final text = name.text;
    final message = text.contains('\n') || text.contains('\r')
        ? 'El nombre del asistente debe ocupar una sola línea'
        : text.trim().isEmpty || text.trim().runes.length > 200
        ? 'Escribe un nombre de entre 1 y 200 caracteres'
        : null;
    if (message != null) {
      setState(() => error = message);
      nameFocus.requestFocus();
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        final target = nameRegion.currentContext;
        if (target != null) {
          Scrollable.ensureVisible(
            target,
            alignment: 1,
            duration: MediaQuery.disableAnimationsOf(context)
                ? Duration.zero
                : const Duration(milliseconds: 180),
          );
        }
      });
      return;
    }
    setState(() {
      busy = true;
      error = null;
    });
    final backend = widget.state.backend;
    final ok = await widget.state.perform(
      () => backend is AssistantProfileBackend
          ? (backend as AssistantProfileBackend).saveProfile(name.text, avatar)
          : (backend as AssistantChatBackend).renameAssistant(name.text),
    );
    if (!mounted) return;
    if (ok) {
      Navigator.pop(context);
    } else {
      setState(() {
        busy = false;
        error = widget.state.failure;
      });
    }
  }

  @override
  Widget build(BuildContext context) => PopScope(
    canPop: !busy,
    child: Actions(
      actions: <Type, Action<Intent>>{
        DismissIntent: CallbackAction<DismissIntent>(
          onInvoke: (_) {
            if (!busy) Navigator.of(context).pop();
            return null;
          },
        ),
      },
      // Si se deshabilitan todos los controles durante IO, el foco permanece
      // dentro de estas acciones y Escape vuelve a servir al terminar/fallar.
      child: FocusScope(
        autofocus: true,
        child: Dialog(
          // Base propia para mantener legibles los controles sobre el barrier y vidrio.
          backgroundColor:
              (Theme.of(context).brightness == Brightness.dark
                      ? const Color(0xff26282c)
                      : const Color(0xfff5f5f4))
                  .withValues(
                    alpha: MediaQuery.highContrastOf(context) ? 1 : .94,
                  ),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
          child: Glass(
            padding: const EdgeInsets.all(24),
            child: SizedBox(
              width: 372,
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text(
                    'Personalizar asistente',
                    style: TextStyle(fontSize: 18),
                  ),
                  const SizedBox(height: 20),
                  Flexible(
                    child: Scrollbar(
                      controller: scroll,
                      child: SingleChildScrollView(
                        controller: scroll,
                        child: Column(
                          mainAxisSize: MainAxisSize.min,
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Row(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                avatar == null
                                    ? AssistantAvatar(
                                        state: widget.state,
                                        profile: widget.profile,
                                        size: 56,
                                        allowRetry: true,
                                      )
                                    : avatar!.isEmpty
                                    ? defaultAssistantAvatar(56)
                                    : assistantImage(avatar!, 56, name.text),
                                const SizedBox(width: 12),
                                Expanded(
                                  child: Wrap(
                                    spacing: 8,
                                    children: [
                                      TextButton(
                                        onPressed:
                                            busy ||
                                                widget.state.backend
                                                    is! AssistantProfileBackend
                                            ? null
                                            : choose,
                                        child: const Text('Elegir imagen…'),
                                      ),
                                      if (avatar?.isNotEmpty == true ||
                                          avatar == null &&
                                              widget.profile.avatarRevision !=
                                                  null)
                                        TextButton(
                                          onPressed: busy
                                              ? null
                                              : () => setState(() {
                                                  avatar = Uint8List(0);
                                                  error = null;
                                                }),
                                          child: const Text('Usar pingüino'),
                                        ),
                                    ],
                                  ),
                                ),
                              ],
                            ),
                            const SizedBox(height: 8),
                            if (preparing)
                              Semantics(
                                liveRegion: true,
                                child: const Text('Preparando imagen…'),
                              ),
                            const Text(
                              'PNG, JPEG o WebP · hasta 5 MiB y 4096 × 4096 px',
                              style: TextStyle(fontSize: 11),
                            ),
                            const SizedBox(height: 16),
                            Column(
                              key: nameRegion,
                              mainAxisSize: MainAxisSize.min,
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                TextField(
                                  controller: name,
                                  focusNode: nameFocus,
                                  autofocus: true,
                                  maxLength: 200,
                                  enabled: !busy,
                                  decoration: const InputDecoration(
                                    labelText: 'Nombre del asistente',
                                  ),
                                ),
                                if (error != null)
                                  Padding(
                                    padding: const EdgeInsets.only(top: 8),
                                    child: Semantics(
                                      liveRegion: true,
                                      child: Text(
                                        error!,
                                        style: TextStyle(
                                          color: Theme.of(
                                            context,
                                          ).colorScheme.error,
                                        ),
                                      ),
                                    ),
                                  ),
                              ],
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(height: 16),
                  Align(
                    alignment: Alignment.centerRight,
                    child: Wrap(
                      alignment: WrapAlignment.end,
                      spacing: 8,
                      children: [
                        TextButton(
                          onPressed: busy ? null : () => Navigator.pop(context),
                          child: const Text('Cancelar'),
                        ),
                        FilledButton(
                          onPressed: busy ? null : save,
                          child: Text(
                            busy && !preparing ? 'Guardando…' : 'Guardar',
                          ),
                        ),
                      ],
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
