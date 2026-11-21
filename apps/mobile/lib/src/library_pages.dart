import 'rust/api/device.dart';

/// Shares pages between the visible list and the wheel's SELECT action.
class LibraryPages<T> {
  LibraryPages(this._load);

  static const _pageSize = 64;
  final Future<List<T>> Function(LibraryPage) _load;
  final _pages = <int, Future<List<T>>>{};

  Future<List<T>> pageFor(int index) {
    final offset = index ~/ _pageSize * _pageSize;
    return _pages.putIfAbsent(
      offset,
      () => _load(LibraryPage(offset: offset, limit: _pageSize)),
    );
  }

  T? itemIn(List<T>? page, int index) {
    final position = index % _pageSize;
    return page != null && position < page.length ? page[position] : null;
  }

  Future<T?> itemAt(int index) async => itemIn(await pageFor(index), index);

  void clear() => _pages.clear();
}
