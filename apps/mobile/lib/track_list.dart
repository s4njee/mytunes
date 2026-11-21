import 'package:flutter/widgets.dart';

import 'src/rust/api/device.dart';
import 'theme/mt_theme.dart';
import 'theme/tokens.g.dart';
import 'wheel_list.dart';

class SongsList extends StatefulWidget {
  const SongsList({
    super.key,
    required this.library,
    required this.count,
    required this.selection,
  });

  final Future<DeviceLibrary>? library;
  final int count;
  final int selection;

  @override
  State<SongsList> createState() => _SongsListState();
}

class _SongsListState extends State<SongsList> {
  static const _pageSize = 64;
  final _pages = <int, Future<List<DeviceTrack>>>{};

  @override
  void didUpdateWidget(SongsList oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.library != oldWidget.library ||
        widget.count != oldWidget.count) {
      _pages.clear();
    }
  }

  @override
  Widget build(BuildContext context) => WheelList(
    count: widget.library == null ? 0 : widget.count,
    selection: widget.selection,
    rowHeight: MtSize.queueRow,
    topPadding: 8,
    itemBuilder: (context, index) {
      final offset = index ~/ _pageSize * _pageSize;
      final page = _pages.putIfAbsent(
        offset,
        () => widget.library!.then(
          (library) => library.songs(
            page: LibraryPage(offset: offset, limit: _pageSize),
          ),
        ),
      );
      return FutureBuilder<List<DeviceTrack>>(
        future: page,
        builder: (context, snapshot) {
          final tracks = snapshot.data;
          final pageIndex = index - offset;
          if (tracks == null || pageIndex >= tracks.length) {
            return const SizedBox.shrink();
          }
          return TrackRow(
            track: tracks[pageIndex],
            number: index + 1,
            selected: widget.selection == index,
          );
        },
      );
    },
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
