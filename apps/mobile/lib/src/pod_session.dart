import 'dart:convert';

import 'package:material_ui/material_ui.dart' show ThemeMode;

import '../pod_navigation.dart';
import 'equalizer_settings.dart';

/// The persistent mobile state contains library IDs and user choices.
class PodSession {
  const PodSession({
    this.trackIds = const [],
    this.queueIndex = 0,
    this.positionMs = 0,
    this.volumePercent = 62,
    this.screen = PodScreen.menu,
    this.theme = ThemeMode.system,
    this.lastForegroundMs = 0,
    this.albumArtwork,
    this.albumYear,
    this.paletteIndex = 0,
    this.equalizer = const EqualizerSettings(),
  });

  static const storageKey = 'pod_session';

  final List<String> trackIds;
  final int queueIndex;
  final int positionMs;
  final int volumePercent;
  final PodScreen screen;
  final ThemeMode theme;
  final int lastForegroundMs;
  final String? albumArtwork;
  final int? albumYear;
  final int paletteIndex;
  final EqualizerSettings equalizer;

  static PodSession? decode(String? json) {
    if (json == null) return null;
    final data = jsonDecode(json);
    if (data is! Map<String, dynamic>) return null;
    final ids = data['queue'];
    return PodSession(
      trackIds: ids is List ? ids.whereType<String>().toList() : const [],
      queueIndex: _readInt(data, 'queueIndex'),
      positionMs: _readInt(data, 'positionMs'),
      volumePercent: _readInt(
        data,
        'volumePercent',
        fallback: 62,
      ).clamp(0, 100),
      screen: PodScreen.values.asNameMap()[data['screen']] ?? PodScreen.menu,
      theme: ThemeMode.values.asNameMap()[data['theme']] ?? ThemeMode.system,
      lastForegroundMs: _readInt(data, 'lastForegroundMs'),
      albumArtwork: data['albumArtwork'] is String
          ? data['albumArtwork'] as String
          : null,
      albumYear: data['albumYear'] is int ? data['albumYear'] as int : null,
      paletteIndex: _readInt(data, 'paletteIndex'),
      equalizer: EqualizerSettings.fromJson(data['equalizer']),
    );
  }

  static int _readInt(
    Map<String, dynamic> data,
    String key, {
    int fallback = 0,
  }) {
    final value = data[key];
    return value is int && value >= 0 ? value : fallback;
  }

  PodScreen startScreen(DateTime now, {required bool hasQueue}) {
    final idle = now.millisecondsSinceEpoch - lastForegroundMs;
    if (idle < 0 || idle > const Duration(hours: 1).inMilliseconds) {
      return PodScreen.menu;
    }
    if (screen == PodScreen.nowPlaying && !hasQueue) return PodScreen.menu;
    return screen;
  }

  String encode() => jsonEncode({
    'queue': trackIds,
    'queueIndex': queueIndex,
    'positionMs': positionMs,
    'volumePercent': volumePercent,
    'screen': screen.name,
    'theme': theme.name,
    'lastForegroundMs': lastForegroundMs,
    'albumArtwork': albumArtwork,
    'albumYear': albumYear,
    'paletteIndex': paletteIndex,
    'equalizer': equalizer.toJson(),
  });
}
