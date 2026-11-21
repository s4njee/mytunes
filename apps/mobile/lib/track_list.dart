import 'package:flutter/widgets.dart';

import 'src/library_pages.dart';
import 'src/rust/api/device.dart';
import 'theme/mt_theme.dart';
import 'theme/tokens.g.dart';
import 'wheel_list.dart';

class SongsList extends StatelessWidget {
  const SongsList({
    super.key,
    required this.songs,
    required this.count,
    required this.selection,
  });

  final LibraryPages<DeviceTrack> songs;
  final int count;
  final int selection;

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
          selected: selection == index,
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
    required this.selected,
  });

  final DeviceTrack track;
  final int number;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final theme = MtTheme.of(context);
    final colors = theme.colors;
    final lightSelection = selected && theme.brightness == Brightness.light;
    final secondary = lightSelection
        ? colors.selectedRowSecondary
        : colors.textQuaternary;
    final ms = track.durationMs?.toInt();
    return Semantics(
      container: true,
      selected: selected,
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
                  style: MtType.timecode.copyWith(color: secondary),
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
                        color: lightSelection
                            ? colors.selectedRowText
                            : colors.textTitle,
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
                  _trackTime(ms),
                  style: MtType.timecode.copyWith(color: secondary),
                ),
            ],
          ),
        ),
      ),
    );
  }

  String _trackTime(int milliseconds) {
    final seconds = Duration(milliseconds: milliseconds).inSeconds;
    return '${seconds ~/ 60}:${(seconds % 60).toString().padLeft(2, '0')}';
  }
}
