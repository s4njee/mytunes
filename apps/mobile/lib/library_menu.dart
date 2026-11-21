import 'package:flutter/widgets.dart';

import 'pod_navigation.dart';
import 'theme/mt_theme.dart';
import 'theme/tokens.g.dart';
import 'wheel_list.dart';

enum LibraryMenuItem {
  nowPlaying('Now Playing', PodScreen.nowPlaying),
  albums('Albums', PodScreen.albums),
  songs('Songs', PodScreen.songs),
  queue('Queue', PodScreen.queue),
  equalizer('Equalizer', PodScreen.equalizer);

  const LibraryMenuItem(this.label, this.destination);
  final String label;
  final PodScreen destination;
}

class LibraryMenu extends StatelessWidget {
  const LibraryMenu({
    super.key,
    required this.selection,
    required this.albums,
    required this.songs,
    required this.queueLength,
    required this.hasPlayed,
  });

  final int selection;
  final int? albums;
  final int? songs;
  final int queueLength;
  final bool hasPlayed;

  @override
  Widget build(BuildContext context) => WheelList(
    count: LibraryMenuItem.values.length,
    selection: selection,
    rowHeight: MtSize.menuRow,
    topPadding: 6,
    itemBuilder: (context, index) {
      final item = LibraryMenuItem.values[index];
      return _MenuRow(
        label: item.label,
        count: switch (item) {
          LibraryMenuItem.albums => albums,
          LibraryMenuItem.songs => songs,
          LibraryMenuItem.queue => queueLength,
          _ => null,
        },
        selected: index == selection,
        enabled: item != LibraryMenuItem.nowPlaying || hasPlayed,
      );
    },
  );
}

class _MenuRow extends StatelessWidget {
  const _MenuRow({
    required this.label,
    required this.count,
    required this.selected,
    required this.enabled,
  });

  final String label;
  final int? count;
  final bool selected;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final theme = MtTheme.of(context);
    final colors = theme.colors;
    final radius = theme.brightness == Brightness.dark
        ? MtRadius.selectedRowDark
        : MtRadius.selectedRowLight;
    final labelColor = selected ? colors.selectedRowText : colors.textMenu;
    return Semantics(
      container: true,
      selected: selected,
      enabled: enabled,
      child: DecoratedBox(
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(radius),
          color: selected ? colors.selectedRowBg : null,
        ),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: MtSpace.rowPaddingX),
          child: Row(
            spacing: MtSpace.rowGap,
            children: [
              Container(
                width: MtSize.menuDot,
                height: MtSize.menuDot,
                decoration: BoxDecoration(
                  color: selected ? colors.selectedDot : colors.dot,
                  shape: BoxShape.circle,
                ),
              ),
              Expanded(
                child: Text(
                  label,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: MtType.menuRow.copyWith(
                    color: enabled
                        ? labelColor
                        : labelColor.withValues(alpha: 0.35),
                  ),
                ),
              ),
              if (count case final value?)
                Text(
                  '$value',
                  style: MtType.timecode.copyWith(
                    color: selected
                        ? colors.selectedRowSecondary
                        : colors.textCount,
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}
