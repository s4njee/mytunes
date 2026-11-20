import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';

import 'theme/mt_theme.dart';
import 'theme/tokens.g.dart';

/// Shared display and wheel deck for the portrait player screens.
class PodShell extends StatelessWidget {
  const PodShell({super.key, required this.display, required this.deck});

  final Widget display;
  final Widget deck;

  @override
  Widget build(BuildContext context) {
    final colors = MtTheme.of(context).colors;
    final safe = MediaQuery.viewPaddingOf(context);
    return DefaultTextStyle(
      style: TextStyle(
        fontFamily: MtFonts.sans,
        color: colors.textPrimary,
        decoration: TextDecoration.none,
      ),
      child: ColoredBox(
        color: colors.bg,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Expanded(child: display),
            DecoratedBox(
              decoration: BoxDecoration(
                color: colors.deck,
                border: Border(top: BorderSide(color: colors.rule)),
              ),
              child: Padding(
                padding: EdgeInsets.only(
                  top: MtSpace.deckTop,
                  bottom: math.max(MtSpace.deckBottom, safe.bottom),
                ),
                child: Center(child: deck),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// A screen's heading and content slide together above the fixed wheel deck.
class PodDisplay extends StatelessWidget {
  const PodDisplay({
    super.key,
    required this.eyebrow,
    required this.metadata,
    required this.body,
    this.onEyebrowHold,
  });

  final String eyebrow;
  final String metadata;
  final Widget body;
  final VoidCallback? onEyebrowHold;

  @override
  Widget build(BuildContext context) {
    final colors = MtTheme.of(context).colors;
    final safe = MediaQuery.viewPaddingOf(context);
    return Padding(
      padding: EdgeInsets.fromLTRB(
        math.max(MtSpace.gutter, safe.left),
        math.max(MtSpace.displayTop, safe.top),
        math.max(MtSpace.gutter, safe.right),
        MtSpace.displayBottom,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          DecoratedBox(
            decoration: BoxDecoration(
              border: Border(bottom: BorderSide(color: colors.rule)),
            ),
            child: Padding(
              padding: const EdgeInsets.only(bottom: MtSpace.headerBottom),
              child: Row(
                children: [
                  Expanded(
                    child: Align(
                      alignment: Alignment.centerLeft,
                      child: _eyebrow(colors.accent),
                    ),
                  ),
                  const SizedBox(width: MtSpace.rowGap),
                  Expanded(
                    child: Text(
                      metadata,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      textAlign: TextAlign.end,
                      style: MtType.meta.copyWith(color: colors.textTertiary),
                    ),
                  ),
                ],
              ),
            ),
          ),
          Expanded(child: body),
        ],
      ),
    );
  }

  Widget _eyebrow(Color accent) {
    final label = Text(
      eyebrow.toUpperCase(),
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      style: MtType.eyebrow.copyWith(color: accent),
    );
    final hold = onEyebrowHold;
    if (hold == null) return label;
    return RawGestureDetector(
      behavior: HitTestBehavior.opaque,
      gestures: {
        LongPressGestureRecognizer:
            GestureRecognizerFactoryWithHandlers<LongPressGestureRecognizer>(
              () => LongPressGestureRecognizer(
                duration: const Duration(seconds: 3),
              ),
              (recognizer) => recognizer.onLongPress = hold,
            ),
      },
      child: label,
    );
  }
}
