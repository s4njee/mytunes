import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter_test/flutter_test.dart';

const _width = 402;
const _height = 874;
const _tolerancePercent = 1.5;
const _screens = ['menu', 'albums', 'now-playing', 'queue', 'equalizer'];

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('counts changed pixels rather than changed colour channels', () {
    final reference = Uint8List(12);
    final capture = Uint8List.fromList([0, 5, 0, 0, 1, 2, 3, 4, 0, 0, 0, 0]);
    expect(
      _pixelDifference(reference, capture, Uint8List(3)),
      closeTo(200 / 3, 1e-9),
    );
  });

  test('counts only unmasked pixels in the comparison', () {
    final reference = Uint8List(16);
    final capture = Uint32List.fromList([1, 2, 0, 9]).buffer.asUint8List();
    expect(
      _pixelDifference(reference, capture, Uint8List.fromList([1, 0, 0, 1])),
      50,
    );
    expect(
      () => _pixelDifference(
        reference,
        capture,
        Uint8List.fromList([1, 1, 1, 1]),
      ),
      throwsFormatException,
    );
  });

  test('artwork rectangles use logical half-open bounds', () {
    final mask = _artworkMask([
      [1, 2, 3, 4],
      [3, 2, 4, 3],
    ]);
    expect(mask[2 * _width], 0);
    expect(mask[2 * _width + 1], 1);
    expect(mask[2 * _width + 3], 1);
    expect(mask[3 * _width + 3], 0);
    expect(mask[4 * _width + 1], 0);
    expect(
      () => _artworkMask([
        [0, 0, 403, 874],
      ]),
      throwsFormatException,
    );
  });

  test('decodes integer-scale PNGs into the same logical viewport', () async {
    final directory = await Directory.systemTemp.createTemp('mytunes-goldens-');
    addTearDown(() => directory.delete(recursive: true));
    final reference = File('${directory.path}/reference.png');
    final capture = File('${directory.path}/capture.png');
    await _writeSolidPng(reference, _width, _height);
    await _writeSolidPng(capture, 2 * _width, 2 * _height);
    final referencePixels = await _readScreenshot(reference);
    final capturePixels = await _readScreenshot(capture);
    expect(referencePixels.length, _width * _height * 4);
    expect(
      _pixelDifference(referencePixels, capturePixels, _artworkMask([])),
      0,
    );

    await _writeSolidPng(capture, _width, _height + 1);
    await expectLater(_readScreenshot(capture), throwsA(isA<TestFailure>()));
  });

  final captures = Platform.environment['MYTUNES_HANDOFF_CAPTURES'];
  group(
    'handoff screenshots',
    () {
      const handoff = '../../design/handoff/screens';
      late Map<String, dynamic> artwork;

      setUpAll(() {
        final file = File('$handoff/artwork.json');
        artwork = file.existsSync()
            ? jsonDecode(file.readAsStringSync()) as Map<String, dynamic>
            : const {};
        final expected = {
          for (final theme in ['dark', 'light'])
            for (final screen in _screens) '$theme-$screen',
        };
        expect(artwork.keys, everyElement(isIn(expected)));
      });

      for (final theme in ['dark', 'light']) {
        for (final screen in _screens) {
          final name = '$theme-$screen';
          test(name, () async {
            final reference = await _readScreenshot(File('$handoff/$name.png'));
            final capture = await _readScreenshot(File('$captures/$name.png'));
            final mask = _artworkMask(artwork[name] as List<dynamic>? ?? []);
            final difference = _pixelDifference(reference, capture, mask);
            expect(
              difference,
              lessThanOrEqualTo(_tolerancePercent),
              reason:
                  '$name changed ${difference.toStringAsFixed(3)}% of the '
                  'unmasked viewport. The limit is $_tolerancePercent%.',
            );
          });
        }
      }
    },
    skip: captures == null || captures.isEmpty
        ? 'Set MYTUNES_HANDOFF_CAPTURES to compare device captures.'
        : false,
  );
}

Future<void> _writeSolidPng(File file, int width, int height) async {
  final recorder = ui.PictureRecorder();
  ui.Canvas(recorder).drawColor(const ui.Color(0xff285370), ui.BlendMode.src);
  final picture = recorder.endRecording();
  try {
    final image = await picture.toImage(width, height);
    try {
      final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
      await file.writeAsBytes(
        bytes!.buffer.asUint8List(bytes.offsetInBytes, bytes.lengthInBytes),
      );
    } finally {
      image.dispose();
    }
  } finally {
    picture.dispose();
  }
}

Future<Uint8List> _readScreenshot(File file) async {
  if (!file.existsSync()) {
    fail('Missing screenshot: ${file.absolute.path}');
  }
  final buffer = await ui.ImmutableBuffer.fromUint8List(
    await file.readAsBytes(),
  );
  try {
    final descriptor = await ui.ImageDescriptor.encoded(buffer);
    try {
      final scale = descriptor.width ~/ _width;
      if (scale < 1 ||
          descriptor.width != _width * scale ||
          descriptor.height != _height * scale) {
        fail(
          '${file.path} is ${descriptor.width}×${descriptor.height}. '
          'Expected a 402×874 viewport at an integer pixel ratio.',
        );
      }
      final codec = await descriptor.instantiateCodec(
        targetWidth: _width,
        targetHeight: _height,
      );
      try {
        final frame = await codec.getNextFrame();
        try {
          final bytes = await frame.image.toByteData(
            format: ui.ImageByteFormat.rawStraightRgba,
          );
          return bytes!.buffer.asUint8List(
            bytes.offsetInBytes,
            bytes.lengthInBytes,
          );
        } finally {
          frame.image.dispose();
        }
      } finally {
        codec.dispose();
      }
    } finally {
      descriptor.dispose();
    }
  } finally {
    buffer.dispose();
  }
}

Uint8List _artworkMask(List<dynamic> rectangles) {
  final mask = Uint8List(_width * _height);
  for (final rectangle in rectangles) {
    if (rectangle case [int left, int top, int right, int bottom]
        when 0 <= left &&
            left < right &&
            right <= _width &&
            0 <= top &&
            top < bottom &&
            bottom <= _height) {
      for (var y = top; y < bottom; y++) {
        mask.fillRange(y * _width + left, y * _width + right, 1);
      }
    } else {
      throw FormatException('Invalid artwork rectangle: $rectangle');
    }
  }
  return mask;
}

double _pixelDifference(
  Uint8List reference,
  Uint8List capture,
  Uint8List mask,
) {
  final referencePixels = Uint32List.view(
    reference.buffer,
    reference.offsetInBytes,
    mask.length,
  );
  final capturePixels = Uint32List.view(
    capture.buffer,
    capture.offsetInBytes,
    mask.length,
  );
  var compared = 0;
  var changed = 0;
  for (var i = 0; i < mask.length; i++) {
    if (mask[i] == 0) {
      compared++;
      if (referencePixels[i] != capturePixels[i]) changed++;
    }
  }
  if (compared == 0) {
    throw const FormatException('The entire viewport is masked.');
  }
  return 100 * changed / compared;
}
