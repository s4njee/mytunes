import 'package:flutter/widgets.dart';

/// Exposes a list item to assistive technology without adding a touch action.
class WheelItemSemantics extends StatelessWidget {
  const WheelItemSemantics({
    super.key,
    required this.label,
    required this.number,
    required this.total,
    required this.selected,
    required this.onSelect,
    required this.onActivate,
    required this.child,
    this.enabled = true,
    this.value,
  });

  final String label;
  final int number;
  final int total;
  final bool selected;
  final VoidCallback? onSelect;
  final VoidCallback? onActivate;
  final Widget child;
  final bool enabled;
  final String? value;

  @override
  Widget build(BuildContext context) => Semantics(
    container: true,
    excludeSemantics: true,
    focusable: true,
    selected: selected,
    enabled: enabled,
    label: '$label, $number of $total',
    value: value,
    onDidGainAccessibilityFocus: onSelect,
    onTap: enabled ? onActivate : null,
    child: child,
  );
}
