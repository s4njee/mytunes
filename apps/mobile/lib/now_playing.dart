import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

import 'cover_art.dart';
import 'src/playback_format.dart';
import 'src/rust/api/device.dart';
import 'src/rust/api/playback.dart';
import 'theme/mt_theme.dart';
import 'theme/tokens.g.dart';

class NowPlaying extends StatelessWidget {
  const NowPlaying({
    super.key,
    required this.track,
    required this.artworkPath,
    required this.paletteIndex,
    required this.events,
    required this.scrubPreview,
  });

  final DeviceTrack track;
  final Future<String?>? artworkPath;
  final int paletteIndex;
  final ValueListenable<PlaybackEvent?> events;
  final ValueListenable<int?> scrubPreview;

  /// The mobile engine starts at the handoff's 62% volume.
  static const defaultVolumePercent = 62;

  @override
  Widget build(BuildContext context) {
    final theme = MtTheme.of(context);
    final colors = theme.colors;
    final dark = theme.brightness == Brightness.dark;
    final format = trackFormat(track);
    final title = track.title ?? 'Untitled';
    final radius = BorderRadius.circular(MtRadius.artwork);
    return LayoutBuilder(
      builder: (context, constraints) => SingleChildScrollView(
        child: ConstrainedBox(
          constraints: BoxConstraints(minHeight: constraints.maxHeight),
          child: Padding(
            padding: const EdgeInsets.only(top: 6),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              spacing: MtSpace.nowPlayingGap,
              children: [
                Row(
                  crossAxisAlignment: CrossAxisAlignment.end,
                  spacing: 16,
                  children: [
                    SizedBox.square(
                      dimension: MtSize.nowPlayingArt,
                      child: DecoratedBox(
                        decoration: BoxDecoration(
                          borderRadius: radius,
                          boxShadow: [
                            dark
                                ? const BoxShadow(
                                    color: Color(0x80000000),
                                    offset: Offset(0, 14),
                                    blurRadius: 30,
                                  )
                                : const BoxShadow(
                                    color: Color(0x2E141E2D),
                                    offset: Offset(0, 10),
                                    blurRadius: 24,
                                  ),
                          ],
                        ),
                        child: CoverArt(
                          artworkPath: artworkPath,
                          caption: track.album ?? title,
                          paletteIndex: paletteIndex,
                          width: MtSize.nowPlayingArt,
                          radius: radius,
                          captionInset: 11,
                        ),
                      ),
                    ),
                    Expanded(
                      child: Padding(
                        padding: const EdgeInsets.only(bottom: 4),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              title,
                              maxLines: 3,
                              overflow: TextOverflow.ellipsis,
                              style: MtType.nowPlayingTitle,
                            ),
                            if (track.artist case final artist?) ...[
                              const SizedBox(height: 5),
                              Text(
                                artist,
                                maxLines: 2,
                                overflow: TextOverflow.ellipsis,
                                style: MtType.artist.copyWith(
                                  color: colors.textSecondary,
                                ),
                              ),
                            ],
                            if (format != null) ...[
                              const SizedBox(height: 9),
                              Text(
                                format,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: MtType.formatBadge.copyWith(
                                  color: colors.accent,
                                ),
                              ),
                            ],
                          ],
                        ),
                      ),
                    ),
                  ],
                ),
                ListenableBuilder(
                  listenable: Listenable.merge([events, scrubPreview]),
                  builder: (context, _) {
                    final event = events.value;
                    final position = event?.trackId == track.id
                        ? event!.positionMs.toInt()
                        : 0;
                    return _Progress(
                      position: scrubPreview.value ?? position,
                      duration: track.durationMs?.toInt() ?? 0,
                    );
                  },
                ),
                ValueListenableBuilder(
                  valueListenable: events,
                  builder: (context, event, _) => _Volume(
                    percent: event?.volumePercent ?? defaultVolumePercent,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _Progress extends StatelessWidget {
  const _Progress({required this.position, required this.duration});

  final int position;
  final int duration;

  @override
  Widget build(BuildContext context) {
    final colors = MtTheme.of(context).colors;
    final known = duration > 0;
    final elapsed = known ? position.clamp(0, duration) : position;
    final clock = MtType.timecode.copyWith(color: colors.textTimecode);
    return Column(
      spacing: 9,
      children: [
        ClipRRect(
          borderRadius: BorderRadius.circular(MtRadius.progress),
          child: SizedBox(
            height: MtSize.progressTrack,
            width: double.infinity,
            child: ColoredBox(
              color: colors.trackInactive,
              child: FractionallySizedBox(
                widthFactor: known ? elapsed / duration : 0,
                alignment: Alignment.centerLeft,
                child: ColoredBox(color: colors.accent),
              ),
            ),
          ),
        ),
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            Text(trackTime(elapsed), style: clock),
            if (known) Text('-${trackTime(duration - elapsed)}', style: clock),
          ],
        ),
      ],
    );
  }
}

class _Volume extends StatelessWidget {
  const _Volume({required this.percent});

  final int percent;

  @override
  Widget build(BuildContext context) {
    final colors = MtTheme.of(context).colors;
    final bars = MtSize.volumeBars.toInt();
    return Semantics(
      label: 'Volume',
      value: '$percent%',
      excludeSemantics: true,
      child: Row(
        spacing: 10,
        children: [
          Text(
            'VOL',
            style: MtType.volumeLabel.copyWith(color: colors.textQuaternary),
          ),
          Expanded(
            child: SizedBox(
              height: 14,
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.end,
                spacing: MtSize.volumeBarGap,
                children: [
                  for (var i = 0; i < bars; i++)
                    Expanded(
                      child: SizedBox(
                        height: 5 + 2.0 * (i % 3),
                        child: DecoratedBox(
                          decoration: BoxDecoration(
                            color: i * 100 < percent * bars
                                ? colors.accent
                                : colors.trackInactive,
                            borderRadius: BorderRadius.circular(1),
                          ),
                        ),
                      ),
                    ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}
