import 'dart:math' as math;

import 'package:flutter/widgets.dart';

import 'theme/mt_theme.dart';
import 'theme/tokens.g.dart';

/// Shared display and wheel deck for the portrait player screens.
class PodShell extends StatelessWidget {
  const PodShell({
    super.key,
    required this.eyebrow,
    required this.metadata,
    required this.display,
    required this.deck,
  });

  final String eyebrow;
  final String metadata;
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
            Expanded(
              child: Padding(
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
                        padding: const EdgeInsets.only(
                          bottom: MtSpace.headerBottom,
                        ),
                        child: Row(
                          children: [
                            Expanded(
                              child: Text(
                                eyebrow.toUpperCase(),
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: MtType.eyebrow.copyWith(
                                  color: colors.accent,
                                ),
                              ),
                            ),
                            const SizedBox(width: MtSpace.rowGap),
                            Expanded(
                              child: Text(
                                metadata,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                textAlign: TextAlign.end,
                                style: MtType.meta.copyWith(
                                  color: colors.textTertiary,
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                    Expanded(child: display),
                  ],
                ),
              ),
            ),
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
