import 'dart:ui';
import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

// Material compartido para tooltips, menús y diálogos sobre contenido Flutter.
class Glass extends StatelessWidget {
  const Glass({
    super.key,
    required this.child,
    this.radius = 12,
    this.padding = EdgeInsets.zero,
  });
  final Widget child;
  final double radius;
  final EdgeInsetsGeometry padding;
  @override
  Widget build(BuildContext context) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    return ClipRRect(
      borderRadius: BorderRadius.circular(radius),
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: 16, sigmaY: 16),
        child: ColoredBox(
          color: (dark ? const Color(0xff26282c) : const Color(0xffd8dadc))
              .withValues(alpha: .72),
          child: Material(
            type: MaterialType.transparency,
            child: Padding(padding: padding, child: child),
          ),
        ),
      ),
    );
  }
}

class RailButton extends StatefulWidget {
  const RailButton({
    super.key,
    required this.label,
    required this.icon,
    required this.onPressed,
    this.selected = false,
    this.asset,
  });
  final String? asset;
  final String label;
  final IconData icon;
  final VoidCallback onPressed;
  final bool selected;
  @override
  State<RailButton> createState() => _RailButtonState();
}

class _RailButtonState extends State<RailButton> {
  final _link = LayerLink();
  OverlayEntry? _tooltip;
  void hide() {
    _tooltip?.remove();
    _tooltip = null;
  }

  void show() {
    hide();
    _tooltip = OverlayEntry(
      builder: (context) => Positioned(
        top: 0,
        left: 0,
        child: CompositedTransformFollower(
          link: _link,
          showWhenUnlinked: false,
          targetAnchor: Alignment.centerRight,
          followerAnchor: Alignment.centerLeft,
          offset: const Offset(4, 0),
          child: IgnorePointer(
            child: Material(
              type: MaterialType.transparency,
              child: Glass(
                radius: 8,
                padding: const EdgeInsets.symmetric(
                  horizontal: 14,
                  vertical: 7,
                ),
                child: Text(
                  widget.label,
                  style: const TextStyle(
                    fontFamily: 'Inter',
                    fontSize: 12,
                    fontWeight: FontWeight.w500,
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
    Overlay.of(context).insert(_tooltip!);
  }

  @override
  void dispose() {
    hide();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => CompositedTransformTarget(
    link: _link,
    child: MouseRegion(
      onEnter: (_) => show(),
      onExit: (_) => hide(),
      child: Semantics(
        button: true,
        label: widget.label,
        selected: widget.selected,
        child: SizedBox(
          width: 36,
          height: 36,
          child: IconButton(
            onPressed: () {
              hide();
              widget.onPressed();
            },
            padding: EdgeInsets.zero,
            style: ButtonStyle(
              shape: WidgetStatePropertyAll(
                RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
              ),
              backgroundColor: WidgetStateProperty.resolveWith(
                (states) => widget.selected
                    ? const Color(0x24808080)
                    : states.contains(WidgetState.hovered)
                    ? const Color(0x14808080)
                    : Colors.transparent,
              ),
            ),
            icon: widget.asset == null
                ? Icon(
                    widget.icon,
                    size: 19,
                    color: Theme.of(context).brightness == Brightness.dark
                        ? const Color(0xffd0d0d0)
                        : const Color(0xff333333),
                  )
                : SvgPicture.asset(
                    widget.asset!,
                    width: 18,
                    height: 18,
                    colorFilter: ColorFilter.mode(
                      Theme.of(context).brightness == Brightness.dark
                          ? const Color(0xffd0d0d0)
                          : const Color(0xff333333),
                      BlendMode.srcIn,
                    ),
                  ),
          ),
        ),
      ),
    ),
  );
}
