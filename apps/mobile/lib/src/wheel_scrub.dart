import 'dart:async';

import 'package:flutter/foundation.dart';

/// Coalesces detents, with at most one seek in flight.
class WheelScrub {
  WheelScrub(this._seek);

  final Future<void> Function(int) _seek;
  final preview = ValueNotifier<int?>(null);
  Timer? _timer;
  int? _pending;
  bool _sending = false;
  bool _flushAfterSend = false;
  bool _released = true;
  bool _disposed = false;

  void step(int direction, {required int position, required int duration}) {
    if (duration <= 0) return;
    final from = preview.value ?? position;
    final target = (from + 4000 * direction).clamp(0, duration - 1);
    if (from == target) return;
    _released = false;
    _pending = target;
    preview.value = target;
    if (!_flushAfterSend) {
      _timer ??= Timer(const Duration(milliseconds: 150), _flush);
    }
  }

  void release() {
    _released = true;
    _flush();
  }

  void cancel() {
    _timer?.cancel();
    _timer = null;
    _pending = null;
    _released = true;
    _flushAfterSend = false;
    preview.value = null;
  }

  void _flush() {
    _timer?.cancel();
    _timer = null;
    final target = _pending;
    if (_sending) {
      _flushAfterSend = target != null;
      return;
    }
    if (target == null) {
      if (_released) preview.value = null;
      return;
    }
    _pending = null;
    _sending = true;
    unawaited(_send(target));
  }

  Future<void> _send(int target) async {
    try {
      await _seek(target);
    } finally {
      _sending = false;
      if (!_disposed) {
        if (_flushAfterSend) {
          _flushAfterSend = false;
          _flush();
        } else if (_released && _pending == null) {
          preview.value = null;
        }
      }
    }
  }

  void dispose() {
    _disposed = true;
    _timer?.cancel();
    preview.dispose();
  }
}
