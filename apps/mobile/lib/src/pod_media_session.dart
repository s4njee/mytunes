import 'dart:async';

import 'package:audio_service/audio_service.dart';
import 'package:audio_session/audio_session.dart';

import 'rust/api/device.dart';
import 'rust/api/playback.dart' as core;

enum MediaCommand {
  play,
  pause,
  toggle,
  pauseForInterruption,
  previous,
  next,
  seek,
}

typedef MediaCommandHandler = Future<void> Function(
  MediaCommand command,
  Duration? position,
);

/// Publishes the Rust player to OS controls and forwards commands back to it.
class PodMediaSession extends BaseAudioHandler {
  PodMediaSession._(this._session) {
    _session.interruptionEventStream.listen(_interruption);
    _session.becomingNoisyEventStream.listen((_) => unawaited(pause()));
  }

  final AudioSession _session;
  MediaCommandHandler? onCommand;
  bool _resumeAfterInterruption = false;
  bool _wantsPlayback = false;
  bool _stopped = false;
  int _artRevision = 0;
  String? _publishedTrack;

  bool get wantsPlayback => _wantsPlayback;

  static Future<PodMediaSession> start() async {
    final session = await AudioSession.instance;
    await session.configure(
      const AudioSessionConfiguration.music().copyWith(
        androidWillPauseWhenDucked: true,
      ),
    );
    return AudioService.init(
      builder: () => PodMediaSession._(session),
      config: const AudioServiceConfig(
        androidNotificationChannelId: 'io.github.s4njee.mytunes_pod.playback',
        androidNotificationChannelName: 'MyTunes Pod playback',
        androidStopForegroundOnPause: false,
      ),
    );
  }

  Future<void> activate() async {
    if (!await _session.setActive(true)) {
      throw StateError('The system denied audio focus.');
    }
    _stopped = false;
    _wantsPlayback = true;
  }

  Future<void> deactivate() async {
    _wantsPlayback = false;
    await _session.setActive(false);
  }

  void _interruption(AudioInterruptionEvent event) {
    if (event.begin) {
      _resumeAfterInterruption |= _wantsPlayback;
      _wantsPlayback = false;
      unawaited(_send(MediaCommand.pauseForInterruption));
    } else {
      final resume = _resumeAfterInterruption;
      _resumeAfterInterruption = false;
      if (resume && event.type != AudioInterruptionType.unknown) {
        unawaited(play());
      }
    }
  }

  Future<void> _send(MediaCommand command, [Duration? position]) async {
    await onCommand?.call(command, position);
  }

  Future<void> toggle() {
    _resumeAfterInterruption = false;
    return _send(MediaCommand.toggle);
  }

  @override
  Future<void> play() {
    _resumeAfterInterruption = false;
    return _send(MediaCommand.play);
  }

  @override
  Future<void> pause() {
    _resumeAfterInterruption = false;
    return _send(MediaCommand.pause);
  }

  @override
  Future<void> skipToNext() => _send(MediaCommand.next);

  @override
  Future<void> skipToPrevious() => _send(MediaCommand.previous);

  @override
  Future<void> seek(Duration position) => _send(MediaCommand.seek, position);

  @override
  Future<void> stop() async {
    _stopped = true;
    _publishedTrack = null;
    _artRevision++;
    await pause();
    mediaItem.add(null);
    await super.stop();
  }

  @override
  Future<void> onTaskRemoved() async {
    if (!playbackState.value.playing) await stop();
  }

  void publish(
    core.PlaybackEvent event,
    DeviceTrack? track,
    Future<String?>? artwork,
  ) {
    final playing = event.state == core.PlaybackState.playing;
    if (event.state == core.PlaybackState.ended ||
        event.state == core.PlaybackState.error) {
      _wantsPlayback = false;
    }
    if (_stopped) return;
    if (_publishedTrack != track?.id) {
      _publishedTrack = track?.id;
      final revision = ++_artRevision;
      if (track == null) {
        mediaItem.add(null);
      } else {
        final duration = track.durationMs?.toInt();
        final item = MediaItem(
          id: track.id,
          title: track.title ?? 'Untitled',
          artist: track.artist,
          album: track.album,
          duration: duration == null ? null : Duration(milliseconds: duration),
        );
        mediaItem.add(item);
        if (artwork != null) {
          unawaited(_publishArtwork(item, artwork, revision));
        }
      }
    }
    playbackState.add(
      PlaybackState(
        processingState: switch (event.state) {
          core.PlaybackState.empty => AudioProcessingState.idle,
          core.PlaybackState.ended => AudioProcessingState.completed,
          core.PlaybackState.error => AudioProcessingState.error,
          _ => AudioProcessingState.ready,
        },
        playing: playing,
        controls: track == null
            ? const []
            : [
                MediaControl.skipToPrevious,
                if (playing) MediaControl.pause else MediaControl.play,
                MediaControl.skipToNext,
              ],
        androidCompactActionIndices: track == null ? const [] : const [0, 1, 2],
        systemActions: track?.durationMs == null
            ? const {}
            : const {MediaAction.seek},
        updatePosition: Duration(milliseconds: event.positionMs.toInt()),
        queueIndex: event.queueIndex,
        errorMessage: event.error,
      ),
    );
  }

  Future<void> _publishArtwork(
    MediaItem item,
    Future<String?> artwork,
    int revision,
  ) async {
    try {
      final path = await artwork;
      if (path != null && _artRevision == revision) {
        mediaItem.add(item.copyWith(artUri: Uri.file(path)));
      }
    } catch (_) {
      // The OS controls can keep displaying the track metadata.
    }
  }
}
