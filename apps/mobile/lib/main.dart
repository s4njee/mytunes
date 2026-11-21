import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';
import 'package:path_provider/path_provider.dart';

import 'albums_grid.dart';
import 'click_wheel.dart';
import 'dev_import.dart';
import 'equalizer.dart';
import 'library_menu.dart';
import 'now_playing.dart';
import 'pod_navigation.dart';
import 'pod_shell.dart';
import 'src/equalizer_settings.dart';
import 'src/library_pages.dart';
import 'src/player_events.dart';
import 'src/pod_session.dart';
import 'src/rust/api/device.dart';
import 'src/rust/api/playback.dart';
import 'src/rust/frb_generated.dart';
import 'src/wheel_scrub.dart';
import 'theme/font_licences.dart';
import 'theme/mt_theme.dart';
import 'theme/tokens.g.dart';
import 'track_list.dart';
import 'wheel_list.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await SystemChrome.setPreferredOrientations([DeviceOrientation.portraitUp]);
  await SystemChrome.setEnabledSystemUIMode(SystemUiMode.edgeToEdge);
  registerFontLicences();
  await RustLib.init();
  runApp(MyTunesPod(library: _openLibrary()));
}

Future<DeviceLibrary> _openLibrary() async {
  final directory = await getApplicationSupportDirectory();
  return openDeviceDb(
    path: directory.uri.resolve('device.sqlite').toFilePath(),
  );
}

/// MyTunes Pod follows the platform's light or dark appearance.
class MyTunesPod extends StatefulWidget {
  const MyTunesPod({super.key, this.library});

  final Future<DeviceLibrary>? library;

  @override
  State<MyTunesPod> createState() => _MyTunesPodState();
}

class _MyTunesPodState extends State<MyTunesPod> {
  ThemeMode _theme = ThemeMode.system;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'MyTunes Pod',
      debugShowCheckedModeBanner: false,
      theme: MtTheme.light.materialTheme,
      darkTheme: MtTheme.dark.materialTheme,
      themeMode: _theme,
      themeAnimationDuration: Duration.zero,
      builder: (context, child) => AnnotatedRegion<SystemUiOverlayStyle>(
        value: MtTheme.of(context).systemUi,
        child: child!,
      ),
      routes: {
        '/licences': (context) =>
            const LicensePage(applicationName: 'MyTunes Pod'),
      },
      home: _LibraryHome(
        library: widget.library,
        onThemeRestored: (mode) {
          if (_theme != mode) setState(() => _theme = mode);
        },
      ),
    );
  }
}

class _LibraryHome extends StatefulWidget {
  const _LibraryHome({required this.library, required this.onThemeRestored});

  final Future<DeviceLibrary>? library;
  final ValueChanged<ThemeMode> onThemeRestored;

  @override
  State<_LibraryHome> createState() => _LibraryHomeState();
}

class _LibraryHomeState extends State<_LibraryHome>
    with WidgetsBindingObserver {
  late Future<LibraryCounts>? _counts = widget.library?.then(
    (library) => library.libraryCounts(),
  );
  late final _songs = LibraryPages<DeviceTrack>(
    (page) async => (await widget.library!).songs(page: page),
  );
  late final _albums = LibraryPages<DeviceAlbum>(
    (page) async => (await widget.library!).albums(page: page),
  );
  bool _importing = false;
  String? _notice;
  List<DeviceTrack> _queue = const [];
  int? _queueIndex;
  String? _albumArtwork;
  int? _albumYear;
  int _artPaletteIndex = 0;
  final _artwork = <String, Future<String?>>{};
  final _playback = ValueNotifier<PlaybackEvent?>(null);
  late final _scrub = WheelScrub(_seekTo);
  StreamSubscription<PlaybackEvent>? _playerEvents;
  Future<void> _commands = Future.value();
  bool _playerConfigured = false;
  int _volume = NowPlaying.defaultVolumePercent;
  EqualizerSettings _equalizer = const EqualizerSettings();
  EqualizerSettings? _appliedEqualizer;
  bool _equalizerError = false;
  bool _hasPlayed = false;
  bool _playing = false;
  bool _startingPlayback = false;
  PodScreen? _playbackError;
  bool _restoring = false;
  PodScreen _screen = PodScreen.menu;
  ThemeMode _theme = ThemeMode.system;
  DateTime _lastForeground = DateTime.now();
  bool _foreground = true;
  Timer? _saveTimer;
  Future<void> _saves = Future.value();

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    if (widget.library != null) {
      _restoring = true;
      unawaited(_restoreSession());
    }
  }

  Future<void> _restoreSession() async {
    try {
      final library = await widget.library!;
      final json = await library.settingsGet(key: PodSession.storageKey);
      PodSession? saved;
      try {
        saved = PodSession.decode(json);
      } on FormatException {
        // An unreadable session can still have a valid legacy EQ preference.
      }
      if (!mounted) return;
      if (saved == null) {
        final legacy = await library.settingsGet(
          key: EqualizerSettings.storageKey,
        );
        if (mounted) _equalizer = EqualizerSettings.decode(legacy);
        return;
      }
      _equalizer = saved.equalizer;
      _volume = saved.volumePercent;
      _theme = saved.theme;
      widget.onThemeRestored(_theme);
      await _restoreQueue(library, saved);
      _screen = saved.startScreen(DateTime.now(), hasQueue: _queue.isNotEmpty);
    } catch (_) {
      // The library still opens when a saved track or audio output is unavailable.
    } finally {
      if (mounted) setState(() => _restoring = false);
    }
  }

  Future<void> _restoreQueue(DeviceLibrary library, PodSession saved) async {
    if (saved.trackIds.isEmpty) return;
    final tracks = await library.tracksByIds(ids: saved.trackIds);
    if (!mounted || tracks.isEmpty) return;
    final savedIndex = saved.queueIndex.clamp(0, saved.trackIds.length - 1);
    final surviving = tracks.map((track) => track.id).toSet();
    final index = saved.trackIds
        .take(savedIndex)
        .where(surviving.contains)
        .length
        .clamp(0, tracks.length - 1);
    final track = tracks[index];
    await _preparePlayer(library);
    if (!mounted) return;
    await library.loadQueue(
      trackIds: tracks.map((track) => track.id).toList(growable: false),
      startIndex: index,
    );
    if (!mounted) return;
    _queue = tracks;
    _queueIndex = index;
    _albumArtwork = saved.albumArtwork;
    _albumYear = saved.albumYear;
    _artPaletteIndex = saved.paletteIndex;
    _hasPlayed = true;
    final duration = track.durationMs?.toInt() ?? 0;
    if (track.id == saved.trackIds[savedIndex] &&
        saved.positionMs > 0 &&
        duration > 0) {
      await library.seek(
        positionMs: BigInt.from(saved.positionMs.clamp(0, duration - 1)),
      );
    }
  }

  void _scheduleSave() {
    if (_restoring || widget.library == null) return;
    if (_foreground) _lastForeground = DateTime.now();
    _saveTimer ??= Timer(const Duration(seconds: 1), _saveSession);
  }

  void _saveSession() {
    _saveTimer?.cancel();
    _saveTimer = null;
    final database = widget.library;
    if (_restoring || database == null) return;
    final event = _playback.value;
    final position =
        event?.trackId == _currentTrack?.id && event?.queueIndex == _queueIndex
        ? event?.positionMs.toInt() ?? 0
        : 0;
    final session = PodSession(
      trackIds: _queue.map((track) => track.id).toList(growable: false),
      queueIndex: _queueIndex ?? 0,
      positionMs: position,
      volumePercent: _volume,
      screen: _screen,
      theme: _theme,
      lastForegroundMs: _lastForeground.millisecondsSinceEpoch,
      albumArtwork: _albumArtwork,
      albumYear: _albumYear,
      paletteIndex: _artPaletteIndex,
      equalizer: _equalizer,
    ).encode();
    _saves = _saves.then((_) async {
      try {
        final library = await database;
        await library.settingsSet(
          key: PodSession.storageKey,
          valueJson: session,
        );
      } catch (_) {
        debugPrint('Could not save the mobile session.');
      }
    });
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      _foreground = true;
      _scheduleSave();
    } else if (_foreground) {
      _lastForeground = DateTime.now();
      _foreground = false;
      _saveSession();
    }
  }

  void _equalizerStep(PodNavigationState navigation, int direction) {
    final index = navigation.selection;
    final bands = EqualizerSettings.frequencies.length;
    if (index < bands) {
      _changeEqualizer(_equalizer.adjust(index, direction));
    } else {
      navigation.select(
        (index + direction).clamp(
          bands - 1,
          bands + EqualizerOption.values.length - 1,
        ),
      );
    }
  }

  void _equalizerCentre(PodNavigationState navigation) {
    final index = navigation.selection;
    final bands = EqualizerSettings.frequencies.length;
    if (index < bands) {
      navigation.select(index + 1);
    } else {
      _changeEqualizer(
        _equalizer.toggle(EqualizerOption.values[index - bands]),
      );
    }
  }

  void _changeEqualizer(EqualizerSettings settings) {
    if (identical(settings, _equalizer)) return;
    setState(() => _equalizer = settings);
    _scheduleSave();
    unawaited(_applyEqualizer(settings));
  }

  Future<void> _applyEqualizer(EqualizerSettings settings) async {
    try {
      await _withTransport((library) async {
        if (!identical(settings, _equalizer)) return;
        if (_playerConfigured) {
          await settings.apply(library, previous: _appliedEqualizer);
          _appliedEqualizer = settings;
        }
      });
      if (mounted && _equalizerError) setState(() => _equalizerError = false);
    } catch (_) {
      if (mounted) setState(() => _equalizerError = true);
    }
  }

  Future<void> _importTracks(Future<DeviceLibrary> database) async {
    if (_importing) return;
    setState(() {
      _importing = true;
      _notice = null;
    });
    try {
      final library = await database;
      final notice = await importPickedTracks(library);
      if (notice != null && mounted) {
        setState(() {
          _songs.clear();
          _albums.clear();
          _counts = library.libraryCounts();
          _notice = notice;
        });
      }
    } catch (_) {
      if (mounted) setState(() => _notice = 'Could not import audio.');
    } finally {
      if (mounted) setState(() => _importing = false);
    }
  }

  Future<void> _playSelection(PodNavigationState navigation) async {
    final database = widget.library;
    if (database == null || _startingPlayback) return;
    _startingPlayback = true;
    final origin = navigation.screen;
    final selection = navigation.selection;
    try {
      final library = await database;
      final album = origin == PodScreen.albums
          ? await _albums.itemAt(selection)
          : null;
      final tracks = switch (origin) {
        PodScreen.albums when album != null => await library.albumTracks(
          id: album.id,
        ),
        PodScreen.songs => [?await _songs.itemAt(selection)],
        PodScreen.queue => _queue,
        _ => const <DeviceTrack>[],
      };
      if (tracks.isEmpty || !mounted) return;
      final startIndex = origin == PodScreen.queue ? selection : 0;
      await _withTransport((library) async {
        if (!navigation.mounted || navigation.screen != origin) return;
        _scrub.cancel();
        await _preparePlayer(library);
        if (!mounted) return;
        await library.loadQueue(
          trackIds: tracks.map((track) => track.id).toList(growable: false),
          startIndex: startIndex,
        );
        if (!mounted) return;
        setState(() {
          _queue = tracks;
          _queueIndex = startIndex;
          if (origin != PodScreen.queue) {
            _albumArtwork = album?.artworkId;
            _albumYear = album?.id.year;
            _artPaletteIndex = album == null ? 0 : selection;
          }
        });
        await library.play();
        if (mounted) {
          setState(() {
            _hasPlayed = true;
            _playbackError = null;
          });
          _scheduleSave();
          if (navigation.mounted && navigation.screen == origin) {
            navigation.push(PodScreen.nowPlaying);
          }
        }
      });
    } catch (_) {
      if (mounted) setState(() => _playbackError = origin);
    } finally {
      _startingPlayback = false;
    }
  }

  Future<void> _preparePlayer(DeviceLibrary library) async {
    if (_playerConfigured) return;
    final equalizer = _equalizer;
    await equalizer.apply(library);
    _appliedEqualizer = equalizer;
    await library.setVolume(volume: _volume / 100);
    if (!mounted) return;
    _playerConfigured = true;
    _playerEvents = library.playerEvents().listen(
      _receivePlayback,
      onError: (Object _) {},
    );
  }

  Future<void> _withTransport(Future<void> Function(DeviceLibrary) action) {
    final next = _commands.then((_) async {
      final library = await widget.library;
      if (mounted && library != null) await action(library);
    });
    _commands = next.then<void>((_) {}, onError: (Object _, StackTrace _) {});
    return next;
  }

  (String?, int?, BigInt?) _trackPass(PlaybackEvent? event) =>
      (event?.trackId, event?.queueIndex, event?.pass);

  void _receivePlayback(PlaybackEvent event) {
    if (!mounted) return;
    if (_trackPass(_playback.value) != _trackPass(event)) _scrub.cancel();
    _playback.value = event;
    _volume = event.volumePercent;
    _scheduleSave();
    final playing = event.state == PlaybackState.playing;
    if (playing != _playing || event.queueIndex != _queueIndex) {
      setState(() {
        _playing = playing;
        _queueIndex = event.queueIndex;
      });
    }
  }

  Future<void> _control(Future<void> Function(DeviceLibrary) action) async {
    try {
      await _withTransport(action);
      if (mounted && _playbackError == PodScreen.nowPlaying) {
        setState(() => _playbackError = null);
      }
    } catch (_) {
      if (mounted) setState(() => _playbackError = PodScreen.nowPlaying);
    }
  }

  void _toggle() => unawaited(_control((library) => library.toggle()));

  void _skip({required bool forward}) {
    _scrub.cancel();
    unawaited(_control((library) => forward ? library.next() : library.prev()));
  }

  void _scrubStep(int direction) {
    final event = _playback.value;
    final track = _currentTrack;
    if (event == null || track == null || event.trackId != track.id) return;
    _scrub.step(
      direction,
      position: event.positionMs.toInt(),
      duration: track.durationMs?.toInt() ?? 0,
    );
  }

  Future<void> _seekTo(int position) {
    final track = _trackPass(_playback.value);
    return _control((library) async {
      if (track == _trackPass(_playback.value)) {
        await library.seek(positionMs: BigInt.from(position));
      }
    });
  }

  DeviceTrack? get _currentTrack {
    final index = _queueIndex ?? 0;
    return index < _queue.length ? _queue[index] : null;
  }

  VoidCallback? _centreAction(
    PodNavigationState navigation,
    LibraryCounts? counts,
  ) {
    switch (navigation.screen) {
      case PodScreen.menu:
        final item = LibraryMenuItem.values[navigation.selection];
        if (item == LibraryMenuItem.nowPlaying && !_hasPlayed) return null;
        final initialIndex = item == LibraryMenuItem.queue && _queue.isNotEmpty
            ? (_queueIndex ?? 0).clamp(0, _queue.length - 1)
            : 0;
        return () => navigation.push(item.destination, selection: initialIndex);
      case PodScreen.songs when (counts?.songs ?? 0) > 0:
      case PodScreen.albums when (counts?.albums ?? 0) > 0:
      case PodScreen.queue when _queue.isNotEmpty:
        return () => unawaited(_playSelection(navigation));
      case PodScreen.nowPlaying when _queue.isNotEmpty:
        return _toggle;
      case PodScreen.equalizer:
        return () => _equalizerCentre(navigation);
      default:
        return null;
    }
  }

  int _selectionCount(PodScreen screen, LibraryCounts? counts) =>
      switch (screen) {
        PodScreen.menu => LibraryMenuItem.values.length,
        PodScreen.albums => counts?.albums ?? 0,
        PodScreen.songs => counts?.songs ?? 0,
        PodScreen.queue => _queue.length,
        _ => 0,
      };

  Widget _display(
    PodScreen screen,
    int selection,
    LibraryCounts? counts,
    String libraryMetadata,
  ) {
    final library = widget.library;
    return switch (screen) {
      PodScreen.menu => PodDisplay(
        eyebrow: 'Library',
        metadata: libraryMetadata,
        onEyebrowHold: kDebugMode && library != null && !_importing
            ? () => unawaited(_importTracks(library))
            : null,
        body: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: LibraryMenu(
                selection: selection,
                albums: counts?.albums ?? (library == null ? 0 : null),
                songs: counts?.songs ?? (library == null ? 0 : null),
                queueLength: _queue.length,
                hasPlayed: _hasPlayed,
              ),
            ),
            if (_notice case final notice?)
              Semantics(
                liveRegion: true,
                child: Text(
                  notice,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
          ],
        ),
      ),
      PodScreen.albums => PodDisplay(
        eyebrow: 'Albums',
        metadata: _playbackError == screen
            ? 'Could not play album'
            : _countLabel(counts?.albums ?? 0, 'album'),
        body: library == null
            ? const SizedBox.shrink()
            : AlbumsGrid(
                library: library,
                albums: _albums,
                count: counts?.albums ?? 0,
                selection: selection,
              ),
      ),
      PodScreen.songs => PodDisplay(
        eyebrow: 'Songs',
        metadata: _playbackError == screen
            ? 'Could not play song'
            : _countLabel(counts?.songs ?? 0, 'song'),
        body: SongsList(
          songs: _songs,
          count: counts?.songs ?? 0,
          selection: selection,
        ),
      ),
      PodScreen.queue => PodDisplay(
        eyebrow: 'Queue',
        metadata: _countLabel(_queue.length, 'track'),
        body: WheelList(
          count: _queue.length,
          selection: selection,
          rowHeight: MtSize.queueRow,
          topPadding: 8,
          itemBuilder: (context, index) => TrackRow(
            track: _queue[index],
            number: index + 1,
            selected: index == selection,
            current: index == _queueIndex,
          ),
        ),
      ),
      PodScreen.nowPlaying => _nowPlaying(),
      PodScreen.equalizer => PodDisplay(
        eyebrow: 'Equalizer',
        metadata: _equalizerError ? 'Could not apply settings' : 'Custom',
        body: Equalizer(settings: _equalizer, selection: selection),
      ),
    };
  }

  Widget _nowPlaying() {
    final track = _currentTrack;
    final artworkId = track?.artworkId ?? _albumArtwork;
    final year = _albumYear;
    final library = widget.library;
    return PodDisplay(
      eyebrow: track?.album ?? 'Now Playing',
      metadata: _playbackError == PodScreen.nowPlaying
          ? 'Could not control playback'
          : (year?.toString() ?? ''),
      body: track == null
          ? const SizedBox.shrink()
          : NowPlaying(
              track: track,
              artworkPath: artworkId == null || library == null
                  ? null
                  : _artwork.putIfAbsent(
                      artworkId,
                      () => library.then(
                        (library) => library.artworkPath(
                          id: artworkId,
                          size: ThumbnailSize.small,
                        ),
                      ),
                    ),
              paletteIndex: _artPaletteIndex,
              events: _playback,
              scrubPreview: _scrub.preview,
            ),
    );
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _saveTimer?.cancel();
    unawaited(_playerEvents?.cancel());
    _scrub.dispose();
    _playback.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_restoring) {
      return const PodShell(
        display: PodDisplay(
          eyebrow: 'Library',
          metadata: 'Opening library…',
          body: SizedBox.shrink(),
        ),
        deck: ClickWheel(screen: PodScreen.menu),
      );
    }
    return FutureBuilder<LibraryCounts>(
      future: _counts,
      builder: (context, snapshot) {
        final library = widget.library;
        final counts = snapshot.data;
        final metadata = switch (snapshot) {
          _ when _importing => 'Importing…',
          _ when snapshot.hasError => 'Could not open library',
          _ when counts != null =>
            '${_countLabel(counts.songs, 'song')} · ${_countLabel(counts.albums, 'album')}',
          _ when library != null => 'Opening library…',
          _ => '0 songs · 0 albums',
        };
        return PodNavigation(
          initialScreen: _screen,
          initialSelection: _screen == PodScreen.queue ? _queueIndex ?? 0 : 0,
          onScreenChanged: (screen) {
            _screen = screen;
            _scheduleSave();
          },
          display: (context, screen, selection) =>
              _display(screen, selection, counts, metadata),
          deck: (context, navigation) {
            final count = _selectionCount(navigation.screen, counts);
            final now = navigation.screen == PodScreen.nowPlaying;
            final equalizer = navigation.screen == PodScreen.equalizer;
            final hasQueue = _queue.isNotEmpty;
            final bands = EqualizerSettings.frequencies;
            return ClickWheel(
              screen: navigation.screen,
              playing: _playing,
              eqBand: equalizer && navigation.selection < bands.length
                  ? bands[navigation.selection]
                  : null,
              onMenu: navigation.menu,
              onCentre: _centreAction(navigation, counts),
              onPrevious: hasQueue ? () => _skip(forward: false) : null,
              onNext: hasQueue ? () => _skip(forward: true) : null,
              onPlayPause: hasQueue ? _toggle : null,
              onDragEnd: now ? _scrub.release : null,
              onStep: equalizer
                  ? (direction) => _equalizerStep(navigation, direction)
                  : now
                  ? _scrubStep
                  : count == 0
                  ? null
                  : (direction) => navigation.select(
                      (navigation.selection + direction).clamp(0, count - 1),
                    ),
            );
          },
        );
      },
    );
  }

  String _countLabel(int count, String noun) =>
      '$count $noun${count == 1 ? '' : 's'}';
}
