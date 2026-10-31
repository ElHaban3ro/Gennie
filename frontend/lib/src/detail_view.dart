import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';
import 'sidebar.dart';
import 'glass.dart';

/// Marco común de las vistas de Encargos y Plugins.
class AppDetailView extends StatelessWidget {
  const AppDetailView({
    super.key,
    required this.title,
    required this.child,
    this.headerKey,
    this.actions = const [],
    this.notice,
  });

  final String title;
  final Widget child;
  final Key? headerKey;
  final List<Widget> actions;
  final Widget? notice;

  @override
  Widget build(BuildContext context) {
    final ink = sidebarInk(context);
    final dark = Theme.of(context).brightness == Brightness.dark;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Container(
          key: headerKey,
          height: 56,
          padding: const EdgeInsets.symmetric(horizontal: 24),
          decoration: BoxDecoration(
            color: Colors.white.withValues(alpha: dark ? .025 : .26),
            border: Border(
              bottom: BorderSide(color: ink.withValues(alpha: .1)),
            ),
          ),
          child: Row(
            children: [
              Expanded(
                child: DragToMoveArea(
                  child: Align(
                    alignment: Alignment.centerLeft,
                    child: Text(
                      title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        fontFamily: 'Inter',
                        fontSize: 14,
                        fontWeight: FontWeight.w500,
                        color: ink,
                      ),
                    ),
                  ),
                ),
              ),
              ...actions,
            ],
          ),
        ),
        ?notice,
        Expanded(child: child),
      ],
    );
  }
}

/// El ancho incluye los márgenes: misma geometría en lectura y edición.
class AppViewContent extends StatelessWidget {
  const AppViewContent({super.key, required this.child, this.footer = false});
  final Widget child;
  final bool footer;

  @override
  Widget build(BuildContext context) => Align(
    alignment: Alignment.topCenter,
    heightFactor: 1,
    child: ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 760),
      child: Padding(
        padding: EdgeInsets.symmetric(
          horizontal: 24,
          vertical: footer ? 12 : 24,
        ),
        child: SizedBox(width: double.infinity, child: child),
      ),
    ),
  );
}

/// El scroll pertenece a la vista; el contenido aporta solo sus secciones.
class AppViewScroll extends StatelessWidget {
  const AppViewScroll({super.key, required this.child});
  final Widget child;

  @override
  Widget build(BuildContext context) =>
      SingleChildScrollView(child: AppViewContent(child: child));
}

/// Identidad de una ficha; misma distribución para Encargos y Plugins.
class AppDetailHeading extends StatelessWidget {
  const AppDetailHeading({
    super.key,
    required this.icon,
    required this.label,
    required this.title,
    this.subtitle,
    this.labelKey,
    this.onEdit,
    this.editLabel,
    this.editKey,
  }) : assert(onEdit == null || editLabel != null);

  final Widget icon;
  final String label, title;
  final Widget? subtitle;
  final Key? labelKey, editKey;
  final VoidCallback? onEdit;
  final String? editLabel;

  @override
  Widget build(BuildContext context) {
    final ink = sidebarInk(context);
    final secondaryInk = ink.withValues(
      alpha: Theme.of(context).brightness == Brightness.dark ? .7 : .8,
    );
    final body = TextStyle(
      fontFamily: 'Inter',
      fontSize: 13,
      height: 1.6,
      color: ink,
    );
    return Stack(
      children: [
        Padding(
          padding: EdgeInsets.only(right: onEdit == null ? 0 : 44),
          child: Row(
            children: [
              icon,
              const SizedBox(width: 16),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      label,
                      key: labelKey,
                      style: body.copyWith(
                        fontSize: 12,
                        fontWeight: FontWeight.w600,
                        color: secondaryInk,
                      ),
                    ),
                    const SizedBox(height: 4),
                    Semantics(
                      header: true,
                      child: Text(
                        title,
                        style: body.copyWith(
                          fontSize: 22,
                          height: 1.25,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ),
                    if (subtitle != null) ...[
                      const SizedBox(height: 6),
                      DefaultTextStyle(
                        style: body.copyWith(fontSize: 12, color: secondaryInk),
                        child: subtitle!,
                      ),
                    ],
                  ],
                ),
              ),
            ],
          ),
        ),
        if (onEdit != null)
          Positioned(
            top: 0,
            right: 0,
            child: AppTooltip(
              label: editLabel!,
              child: MergeSemantics(
                child: Semantics(
                  label: editLabel,
                  child: IconButton(
                    key: editKey,
                    onPressed: onEdit,
                    constraints: const BoxConstraints.tightFor(
                      width: 32,
                      height: 32,
                    ),
                    padding: EdgeInsets.zero,
                    color: ink,
                    icon: const Icon(Icons.edit_outlined, size: 16),
                  ),
                ),
              ),
            ),
          ),
      ],
    );
  }
}
