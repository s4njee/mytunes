import 'package:flutter/widgets.dart';

import 'src/library_pages.dart';
import 'src/playback_format.dart';
import 'src/rust/api/device.dart';
import 'theme/mt_theme.dart';
import 'theme/tokens.g.dart';
import 'wheel_item_semantics.dart';
import 'wheel_list.dart';

class SongsList extends StatelessWidget {
  const SongsList({
    super.key,
    required this.songs,
    required this.count,
    required this.selection,
    required this.onSelect,
    required this.onActivate,
  });

  final LibraryPages<DeviceTrack> songs;
  final int count;
  final int selection;
  final ValueChanged<int> onSelect;
  final ValueChanged<int> onActivate;

  @override
  Widget build(BuildContext context) => WheelList(
    count: count,
    selection: selection,
    rowHeight: MtSize.queueRow,
    topPadding: 8,
    itemBuilder: (context, index) => FutureBuilder<List<DeviceTrack>>(
      future: songs.pageFor(index),
      builder: (context, snapshot) {
        final track = songs.itemIn(snapshot.data, index);
        if (track == null) return const SizedBox.shrink();
        return TrackRow(
          track: track,
          number: index + 1,
          total: count,
          selected: selection == index,
          onSelect: () => onSelect(index),
          onActivate: () => onActivate(index),
        );
      },
    ),
  );
}

/// Shared row for Songs and Queue.
class TrackRow extends StatelessWidget {
  const TrackRow({
    super.key,
    required this.track,
    required this.number,
    required this.total,
    required this.selected,
    required this.onSelect,
    required this.onActivate,
    this.current = false,
  });

  final DeviceTrack track;
  final int number;
  final int total;
  final bool selected;
  final bool current;
  final VoidCallback onSelect;
  final VoidCallback onActivate;

  @override
  Widget build(BuildContext context) {
    final theme = MtTheme.of(context);
    final colors = theme.colors;
    final lightSelection = selected && theme.brightness == Brightness.light;
    final secondary = lightSelection
        ? colors.selectedRowSecondary
        : colors.textCount;
    final highlight = lightSelection
        ? colors.selectedRowText
        : current
        ? colors.accent
        : null;
    final ms = track.durationMs?.toInt();
    return WheelItemSemantics(
      label: [
        track.title ?? 'Untitled',
        ?track.artist,
        if (ms != null) trackTime(ms),
      ].join(', '),
      number: number,
      total: total,
      selected: selected,
      value: current ? 'Current track' : null,
      onSelect: onSelect,
      onActivate: onActivate,
      child: DecoratedBox(
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(MtRadius.queueRow),
          color: selected ? colors.selectedRowBg : null,
        ),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: MtSpace.rowPaddingX),
          child: Row(
            spacing: MtSpace.rowGap,
            children: [
              ConstrainedBox(
                constraints: const BoxConstraints(minWidth: 20),
                child: Text(
                  number.toString().padLeft(2, '0'),
                  style: MtType.timecode.copyWith(
                    color: current ? highlight : secondary,
                  ),
                ),
              ),
              Expanded(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  spacing: 3,
                  children: [
                    Text(
                      track.title ?? 'Untitled',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: MtType.queueTitle.copyWith(
                        color: highlight ?? colors.textTitle,
                      ),
                    ),
                    if (track.artist case final artist?)
                      Text(
                        artist,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: MtType.albumArtist.copyWith(color: secondary),
                      ),
                  ],
                ),
              ),
              if (ms != null)
                Text(
                  trackTime(ms),
                  style: MtType.timecode.copyWith(color: secondary),
                ),
            ],
          ),
        ),
      ),
    );
  }
}
