import 'package:flutter/widgets.dart';

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
      final target =
          position.pixels < top &&
              bottom > position.pixels + position.viewportDimension
          ? bottom - position.viewportDimension
          : position.pixels.clamp(0.0, top);
      final offset = target.clamp(0.0, position.maxScrollExtent);
      if (offset != position.pixels) _controller.jumpTo(offset);
    });
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ListView.builder(
    controller: _controller,
    physics: const NeverScrollableScrollPhysics(),
    padding: EdgeInsets.only(top: widget.topPadding),
    itemExtent: widget.rowHeight,
    itemCount: widget.count,
    itemBuilder: widget.itemBuilder,
  );
}
