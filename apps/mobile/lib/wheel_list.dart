import 'dart:math' as math;
import 'dart:ui' show SemanticsRole;

import 'package:flutter/widgets.dart';

/// The smallest scroll that brings the given row or tile into view.
double revealWheelSelection(
  ScrollPosition position,
  double top,
  double bottom,
) {
  final minOffset = math.min(top, bottom - position.viewportDimension);
  return position.pixels
      .clamp(minOffset, top)
      .clamp(0.0, position.maxScrollExtent);
}

ScrollPhysics? wheelScrollPhysics(BuildContext context) =>
    MediaQuery.accessibleNavigationOf(context)
    ? null
    : const NeverScrollableScrollPhysics();

/// A fixed-height list that keeps the wheel selection visible.
class WheelList extends StatefulWidget {
  const WheelList({
    super.key,
    required this.count,
    required this.selection,
    required this.rowHeight,
    required this.topPadding,
    required this.itemBuilder,
  });

  final int count;
  final int selection;
  final double rowHeight;
  final double topPadding;
  final IndexedWidgetBuilder itemBuilder;

  @override
  State<WheelList> createState() => _WheelListState();
}

class _WheelListState extends State<WheelList> {
  final _controller = ScrollController();

  @override
  void initState() {
    super.initState();
    _revealSelection();
  }

  @override
  void didUpdateWidget(WheelList oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.selection != oldWidget.selection ||
        widget.count != oldWidget.count) {
      _revealSelection();
    }
  }

  void _revealSelection() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_controller.hasClients || widget.count == 0) return;
      final position = _controller.position;
      final top = widget.topPadding + widget.selection * widget.rowHeight;
      final bottom = top + widget.rowHeight;
      final offset = revealWheelSelection(position, top, bottom);
      if (offset != position.pixels) _controller.jumpTo(offset);
    });
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Semantics(
    role: SemanticsRole.list,
    explicitChildNodes: true,
    child: ListView.builder(
      controller: _controller,
      physics: wheelScrollPhysics(context),
      padding: EdgeInsets.only(top: widget.topPadding),
      itemExtent: widget.rowHeight,
      itemCount: widget.count,
      itemBuilder: widget.itemBuilder,
    ),
  );
}
