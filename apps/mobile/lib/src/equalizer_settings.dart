import 'dart:convert';

import 'package:flutter/foundation.dart';

import 'rust/api/device.dart';
import 'rust/api/playback.dart';

enum EqualizerOption {
  gapless('Gapless playback'),
  volumeLimit('Volume limit'),
  shuffleAlbums('Shuffle album order');

  const EqualizerOption(this.label);
  final String label;
}

class EqualizerSettings {
  const EqualizerSettings({
    this.gains = const [3, 1, -2, 4, 2],
    this.gapless = true,
    this.volumeLimited = false,
    this.shuffleAlbums = true,
  });

  static const storageKey = 'equalizer';
  static const frequencies = ['60', '230', '910', '3.6k', '14k'];
  static const _volumeCap = 0.5;

  final List<int> gains;
  final bool gapless;
  final bool volumeLimited;
  final bool shuffleAlbums;

  factory EqualizerSettings.decode(String? json) =>
      EqualizerSettings.fromJson(json == null ? null : jsonDecode(json));

  factory EqualizerSettings.fromJson(Object? data) {
    if (data is! Map<String, dynamic>) return const EqualizerSettings();
    final bands = data['gains'];
    return EqualizerSettings(
      gains:
          bands is List &&
              bands.length == frequencies.length &&
              bands.every((band) => band is int && band >= -6 && band <= 6)
          ? List<int>.unmodifiable(bands.cast<int>())
          : const EqualizerSettings().gains,
      gapless: data['gapless'] is bool ? data['gapless'] as bool : true,
      volumeLimited: data['volumeLimited'] == true,
      shuffleAlbums: data['shuffleAlbums'] is bool
          ? data['shuffleAlbums'] as bool
          : true,
    );
  }

  Map<String, Object> toJson() => {
    'gains': gains,
    'gapless': gapless,
    'volumeLimited': volumeLimited,
    'shuffleAlbums': shuffleAlbums,
  };

  bool enabled(EqualizerOption option) => switch (option) {
    EqualizerOption.gapless => gapless,
    EqualizerOption.volumeLimit => volumeLimited,
    EqualizerOption.shuffleAlbums => shuffleAlbums,
  };

  EqualizerSettings adjust(int band, int direction) {
    final gain = (gains[band] + direction).clamp(-6, 6);
    if (gain == gains[band]) return this;
    return EqualizerSettings(
      gains: List<int>.of(gains)..[band] = gain,
      gapless: gapless,
      volumeLimited: volumeLimited,
      shuffleAlbums: shuffleAlbums,
    );
  }

  EqualizerSettings toggle(EqualizerOption option) => EqualizerSettings(
    gains: gains,
    gapless: option == EqualizerOption.gapless ? !gapless : gapless,
    volumeLimited: option == EqualizerOption.volumeLimit
        ? !volumeLimited
        : volumeLimited,
    shuffleAlbums: option == EqualizerOption.shuffleAlbums
        ? !shuffleAlbums
        : shuffleAlbums,
  );

  Future<void> apply(
    DeviceLibrary library, {
    EqualizerSettings? previous,
  }) async {
    if (!listEquals(gains, previous?.gains)) await library.setEq(gains: gains);
    if (gapless != previous?.gapless ||
        volumeLimited != previous?.volumeLimited) {
      await library.setFlags(
        gapless: gapless,
        soundCheck: PlaybackSoundCheck.off,
        volumeLimit: volumeLimited ? _volumeCap : 1,
      );
    }
    if (shuffleAlbums != previous?.shuffleAlbums) {
      await library.setShuffle(
        mode: shuffleAlbums ? PlaybackShuffle.albums : PlaybackShuffle.off,
      );
    }
  }
}
