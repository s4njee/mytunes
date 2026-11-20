import 'dart:ui' show lerpDouble;

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

import 'pod_shell.dart';

enum PodScreen { menu, nowPlaying, albums, songs, queue, equalizer }

/// Screen history inside the wheel surface, independent of Flutter routes.
class PodNavigation extends StatefulWidget {
  const PodNavigation({super.key, required this.display, required this.deck});

  final Widget Function(BuildContext, PodScreen, int selection) display;
  final Widget Function(BuildContext, PodNavigationState) deck;

  @override
  State<PodNavigation> createState() => PodNavigationState();
}

class PodNavigationState extends State<PodNavigation>
    with SingleTickerProviderStateMixin {
  final _screens = [_Screen(PodScreen.menu, 0)];
  late final _progress =
      AnimationController(
        vsync: this,
        duration: const Duration(milliseconds: 220),
      )..addStatusListener((status) {
        if (status == AnimationStatus.completed) {
          setState(() => _outgoing = null);
        }
      });
  _Departure? _outgoing;
  double _enterFrom = 0;

  PodScreen get screen => _screens.last.id;
  int get selection => _screens.last.selection;

  void select(int index) {
    if (selection != index) setState(() => _screens.last.selection = index);
  }

  void push(PodScreen destination, {int selection = 0}) {
    if (destination == screen ||
        destination == PodScreen.menu ||
        _progress.isAnimating) {
      return;
    }
    final previous = _screens.last;
    _screens.add(_Screen(destination, selection));
    _slideFrom(previous, direction: 1);
  }

  void menu() {
    if (_screens.length == 1) return;
    final previous = _screens.last;
    _screens.removeRange(1, _screens.length);
    _screens.first.selection = 0;
    _slideFrom(previous, direction: -1);
  }

  void _slideFrom(_Screen previous, {required double direction}) {
    final outgoing = _outgoing;
    final progress = Curves.easeOut.transform(_progress.value);
    // MENU can interrupt a push. Keep both visible panels at their positions
    // when reversing an in-flight slide between the menu and its child.
    final enterFrom = outgoing != null && outgoing.screen == _screens.last
        ? lerpDouble(outgoing.from, outgoing.to, progress)!
        : direction;
    final exitFrom = outgoing == null
        ? 0.0
        : lerpDouble(_enterFrom, 0, progress)!;
    setState(() {
      _enterFrom = enterFrom;
      _outgoing = _Departure(previous, exitFrom, -direction);
    });
    _progress.forward(from: 0);
  }

  Widget _panel(_Screen entry, double from, double to, {bool leaving = false}) {
    return KeyedSubtree(
      key: ObjectKey(entry),
      child: ExcludeSemantics(
        excluding: leaving,
        child: IgnorePointer(
          ignoring: leaving,
          child: SlideTransition(
            position: _outgoing == null
                ? const AlwaysStoppedAnimation(Offset.zero)
                : _progress.drive(
                    Tween(
                      begin: Offset(from, 0),
                      end: Offset(to, 0),
                    ).chain(CurveTween(curve: Curves.easeOut)),
                  ),
            child: Builder(
              builder: (context) =>
                  widget.display(context, entry.id, entry.selection),
            ),
          ),
        ),
      ),
    );
  }

  @override
  void dispose() {
    _progress.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final departing = _outgoing;
    return PopScope<void>(
      canPop: false,
      onPopInvokedWithResult: (didPop, _) {
        if (!didPop && defaultTargetPlatform == TargetPlatform.android) menu();
      },
      child: PodShell(
        display: ClipRect(
          child: Stack(
            fit: StackFit.expand,
            children: [
              if (departing != null)
                _panel(
                  departing.screen,
                  departing.from,
                  departing.to,
                  leaving: true,
                ),
              _panel(_screens.last, _enterFrom, 0),
            ],
          ),
        ),
        deck: Builder(builder: (context) => widget.deck(context, this)),
      ),
    );
  }
}

class _Screen {
  _Screen(this.id, this.selection);

  final PodScreen id;
  int selection;
}

class _Departure {
  const _Departure(this.screen, this.from, this.to);

  final _Screen screen;
  final double from;
  final double to;
}
