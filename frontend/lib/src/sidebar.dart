import 'dart:io';
import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';
import 'glass.dart';

Color sidebarInk(BuildContext context) =>
    Theme.of(context).brightness == Brightness.dark
    ? const Color(0xffe3e3e1)
    : const Color(0xff333333);

Color _informativeInk(BuildContext context) =>
    Theme.of(context).brightness == Brightness.dark
    ? const Color(0xffa3a3a3)
    : const Color(0xff555555);

/// Estructura única de Historial, Encargos y Plugins. El escritorio posee el ancho.
class AppSidebar extends StatelessWidget {
  const AppSidebar({
    super.key,
    required this.width,
    required this.title,
    required this.child,
    this.actionLabel,
    this.onAction,
    this.searchController,
    this.searchHint,
    this.searchKey,
    this.onSearch,
  });
  final double width;
  final String title;
  final Widget child;
  final String? actionLabel, searchHint;
  final VoidCallback? onAction;
  final TextEditingController? searchController;
  final Key? searchKey;
  final ValueChanged<String>? onSearch;
  @override
  Widget build(BuildContext context) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    return SizedBox(
      width: width,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: Colors.white.withValues(alpha: dark ? .025 : .26),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Container(
              key: const ValueKey('sidebar-header'),
              height: 56,
              decoration: BoxDecoration(
                border: Border(
                  bottom: BorderSide(
                    color: sidebarInk(context).withValues(alpha: .10),
                  ),
                ),
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
                          title,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            fontFamily: 'Inter',
                            fontSize: 12,
                            fontWeight: FontWeight.w400,
                            color: _informativeInk(context),
                          ),
                        ),
                      ),
                    ),
                  ),
                  if (actionLabel != null)
                    AppTooltip(
                      label: actionLabel!,
                      child: IconButton(
                        visualDensity: VisualDensity.compact,
                        onPressed: onAction,
                        icon: Icon(
                          Icons.add,
                          size: 17,
                          semanticLabel: actionLabel,
                        ),
                      ),
                    ),
                ],
              ),
            ),
            if (searchController != null)
              SidebarSearch(
                controller: searchController!,
                hint: searchHint!,
                fieldKey: searchKey,
                onChanged: onSearch!,
              ),
            Expanded(child: child),
          ],
        ),
      ),
    );
  }
}

class SidebarSearch extends StatelessWidget {
  const SidebarSearch({
    super.key,
    required this.controller,
    required this.hint,
    required this.onChanged,
    this.fieldKey,
  });
  final TextEditingController controller;
  final String hint;
  final ValueChanged<String> onChanged;
  final Key? fieldKey;
  @override
  Widget build(BuildContext context) {
    final ink = sidebarInk(context);
    final dark = Theme.of(context).brightness == Brightness.dark;
    return Padding(
      padding: const EdgeInsets.fromLTRB(8, 12, 8, 10),
      child: SizedBox(
        height: 40,
        child: Semantics(
          label: hint,
          child: TextField(
            key: fieldKey,
            controller: controller,
            onChanged: onChanged,
            textAlignVertical: TextAlignVertical.center,
            style: TextStyle(
              fontFamily: 'Inter',
              fontSize: 12,
              fontWeight: FontWeight.w400,
              color: ink,
            ),
            decoration: InputDecoration(
              hint: ExcludeSemantics(
                child: Text(
                  hint,
                  maxLines: 1,
                  softWrap: false,
                  overflow: TextOverflow.ellipsis,
                  style: Theme.of(context).textTheme.bodyLarge!.copyWith(
                    fontFamily: 'Inter',
                    fontSize: 12,
                    fontWeight: FontWeight.w400,
                    color: _informativeInk(context),
                  ),
                ),
              ),
              filled: false,
              isDense: true,
              contentPadding: const EdgeInsets.symmetric(
                horizontal: 12,
                vertical: 10,
              ),
              prefixIcon: Icon(
                Icons.search_rounded,
                size: 17,
                color: ink.withValues(alpha: .55),
              ),
              prefixIconConstraints: const BoxConstraints(minWidth: 36),
              suffixIconConstraints: const BoxConstraints(
                minWidth: 36,
                maxHeight: 40,
              ),
              suffixIcon: controller.text.isEmpty
                  ? null
                  : AppTooltip(
                      label: 'Limpiar búsqueda',
                      child: IconButton(
                        onPressed: () {
                          controller.clear();
                          onChanged('');
                        },
                        icon: Icon(
                          Icons.backspace_outlined,
                          size: 16,
                          color: ink.withValues(alpha: dark ? .55 : .65),
                          semanticLabel: 'Limpiar búsqueda',
                        ),
                        padding: EdgeInsets.zero,
                        constraints: const BoxConstraints.tightFor(
                          width: 36,
                          height: 36,
                        ),
                      ),
                    ),
              enabledBorder: OutlineInputBorder(
                borderRadius: BorderRadius.circular(8),
                borderSide: BorderSide(color: ink.withValues(alpha: .16)),
              ),
              focusedBorder: OutlineInputBorder(
                borderRadius: BorderRadius.circular(8),
                borderSide: BorderSide(
                  color: ink.withValues(alpha: dark ? .5 : .65),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class SidebarEmpty extends StatelessWidget {
  const SidebarEmpty({super.key, this.message = 'No hay contenido disponible'});
  final String message;
  @override
  Widget build(BuildContext context) => Center(
    child: Padding(
      padding: const EdgeInsets.all(16),
      child: Text(
        message,
        textAlign: TextAlign.center,
        style: TextStyle(
          fontFamily: 'Inter',
          fontSize: 12,
          fontWeight: FontWeight.w400,
          color: _informativeInk(context),
        ),
      ),
    ),
  );
}

/// Filas compactas con el mismo ritmo, selección neutra y tipografía.
class SidebarTile extends StatelessWidget {
  const SidebarTile({
    super.key,
    required this.title,
    required this.subtitle,
    required this.selected,
    required this.onSelect,
    this.subtitleMaxLines = 1,
    this.subtitleColor,
    this.trailing,
    this.leading,
  });
  final String title, subtitle;
  final int subtitleMaxLines;
  final Color? subtitleColor;
  final bool selected;
  final VoidCallback onSelect;
  final Widget? trailing, leading;
  @override
  Widget build(BuildContext context) {
    final ink = sidebarInk(context);
    final dark = Theme.of(context).brightness == Brightness.dark;
    return Material(
      color: selected ? const Color(0x24808080) : Colors.transparent,
      borderRadius: BorderRadius.circular(8),
      child: InkWell(
        onTap: onSelect,
        borderRadius: BorderRadius.circular(8),
        hoverColor: const Color(0x14808080),
        child: ConstrainedBox(
          constraints: const BoxConstraints(minHeight: 64),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 9),
            child: Row(
              children: [
                if (leading != null) ...[leading!, const SizedBox(width: 10)],
                Expanded(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    mainAxisAlignment: MainAxisAlignment.center,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Expanded(
                            child: Text(
                              title,
                              maxLines: 1,
                              softWrap: false,
                              overflow: TextOverflow.ellipsis,
                              style: TextStyle(
                                fontFamily: 'Inter',
                                fontSize: 12,
                                fontWeight: FontWeight.w600,
                                color: ink,
                              ),
                            ),
                          ),
                          if (trailing != null) ...[
                            const SizedBox(width: 8),
                            trailing!,
                          ],
                        ],
                      ),
                      const SizedBox(height: 5),
                      Text(
                        subtitle,
                        maxLines: subtitleMaxLines,
                        softWrap: subtitleMaxLines > 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          fontFamily: 'Inter',
                          fontSize: 11,
                          fontWeight: FontWeight.w400,
                          color:
                              subtitleColor ??
                              ink.withValues(alpha: dark ? .62 : .8),
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
    );
  }
}

/// El estado vacío del lector conserva la presentación aprobada de Plugins.
class ContentEmpty extends StatelessWidget {
  const ContentEmpty({
    super.key,
    required this.title,
    this.message = 'No hay contenido disponible',
    this.action,
  });
  final String title, message;
  final Widget? action;
  @override
  Widget build(BuildContext context) => Center(
    child: Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            title,
            style: TextStyle(
              fontFamily: 'Inter',
              fontSize: 26,
              fontWeight: FontWeight.w500,
              letterSpacing: -.7,
              color: sidebarInk(context),
            ),
          ),
          const SizedBox(height: 12),
          Text(
            message,
            textAlign: TextAlign.center,
            style: TextStyle(
              fontFamily: 'Inter',
              fontSize: 13,
              fontWeight: FontWeight.w400,
              color: _informativeInk(context),
            ),
          ),
          ?action,
        ],
      ),
    ),
  );
}
