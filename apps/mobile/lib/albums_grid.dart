import 'dart:async';
import 'dart:io';

import 'package:flutter/widgets.dart';

import 'src/library_pages.dart';
import 'src/rust/api/device.dart';
import 'theme/mt_theme.dart';
import 'theme/tokens.g.dart';
import 'wheel_list.dart';

const _captionGap = 7.0;
const _artistGap = 4.0;

class AlbumsGrid extends StatefulWidget {
  const AlbumsGrid({
    super.key,
    required this.library,
    required this.albums,
    required this.count,
    required this.selection,
  });

  final Future<DeviceLibrary> library;
  final LibraryPages<DeviceAlbum> albums;
  final int count;
  final int selection;

  @override
  State<AlbumsGrid> createState() => _AlbumsGridState();
}

class _AlbumsGridState extends State<AlbumsGrid> {
  static const _columns = 2;

  final _scroll = ScrollController();
  final _artwork = <String, Future<String?>>{};
  double? _cellHeight;
  bool _selectionChanged = true;

  @override
  void didUpdateWidget(AlbumsGrid oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.selection != oldWidget.selection ||
        widget.count != oldWidget.count) {
      _selectionChanged = true;
    }
    if (widget.library != oldWidget.library) _artwork.clear();
  }

  void _revealSelection(double cellHeight) {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_scroll.hasClients || widget.count == 0) return;
      final row = widget.selection ~/ _columns;
      final top = MtSpace.gridGap + row * (cellHeight + MtSpace.gridGap);
      final offset = revealWheelSelection(
        _scroll.position,
        top,
        top + cellHeight,
      );
      if (offset == _scroll.offset) {
        if (_scroll.position.isScrollingNotifier.value) _scroll.jumpTo(offset);
        return;
      }
      unawaited(
        _scroll.animateTo(
          offset,
          duration: const Duration(milliseconds: 120),
          curve: Curves.easeOut,
        ),
      );
    });
  }

  Future<String?>? _artworkFor(String? id) {
    if (id == null) return null;
    return _artwork.putIfAbsent(
      id,
      () => widget.library.then(
        (library) => library.artworkPath(id: id, size: ThumbnailSize.large),
      ),
    );
  }

  @override
  void dispose() {
    _scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final width = (constraints.maxWidth - MtSpace.gridGap) / _columns;
      final scaler = MediaQuery.textScalerOf(context);
      final titleHeight = _lineHeight(MtType.albumTitle, scaler);
      final artistHeight = _lineHeight(MtType.albumArtist, scaler);
      final height =
          width + _captionGap + titleHeight + _artistGap + artistHeight;
      if (_selectionChanged || height != _cellHeight) {
        _cellHeight = height;
        _selectionChanged = false;
        _revealSelection(height);
      }
      return GridView.builder(
        controller: _scroll,
        physics: const NeverScrollableScrollPhysics(),
        padding: const EdgeInsets.only(top: MtSpace.gridGap),
        gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
          crossAxisCount: _columns,
          crossAxisSpacing: MtSpace.gridGap,
          mainAxisSpacing: MtSpace.gridGap,
          mainAxisExtent: height,
        ),
        itemCount: widget.count,
        itemBuilder: (context, index) => FutureBuilder<List<DeviceAlbum>>(
          future: widget.albums.pageFor(index),
          builder: (context, snapshot) {
            final album = widget.albums.itemIn(snapshot.data, index);
            if (album == null) return const SizedBox.shrink();
            return _AlbumTile(
              album: album,
              index: index,
              selected: index == widget.selection,
              artworkPath: _artworkFor(album.artworkId),
              width: width,
            );
          },
        ),
      );
    },
  );

  double _lineHeight(TextStyle style, TextScaler scaler) =>
      (scaler.scale(style.fontSize!) * style.height!).ceilToDouble();
}

class _AlbumTile extends StatelessWidget {
  const _AlbumTile({
    required this.album,
    required this.index,
    required this.selected,
    required this.artworkPath,
    required this.width,
  });

  final DeviceAlbum album;
  final int index;
  final bool selected;
  final Future<String?>? artworkPath;
  final double width;

  @override
  Widget build(BuildContext context) {
    final theme = MtTheme.of(context);
    final colors = theme.colors;
    final dark = theme.brightness == Brightness.dark;
    final palette = colors.placeholderArt;
    final (fill, foreground) = palette[index % palette.length];
    final placeholder = ColoredBox(
      color: fill,
      child: Align(
        alignment: Alignment.bottomLeft,
        child: Padding(
          padding: const EdgeInsets.all(10),
          child: Text(
            album.id.title.toUpperCase(),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: MtType.artCaption.copyWith(color: foreground),
          ),
        ),
      ),
    );
    final radius = BorderRadius.circular(MtRadius.albumTile);
    final pixels = (width * 2).round();
    return Semantics(
      container: true,
      selected: selected,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          SizedBox.square(
            dimension: width,
            child: DecoratedBox(
              decoration: BoxDecoration(
                borderRadius: radius,
                boxShadow: [
                  BoxShadow(
                    color: dark
                        ? (selected
                              ? const Color(0x66000000)
                              : const Color(0x59000000))
                        : (selected
                              ? const Color(0x2E141E2D)
                              : const Color(0x24141E2D)),
                    blurRadius: dark ? 20 : (selected ? 18 : 16),
                    offset: Offset(0, dark || selected ? 8 : 6),
                  ),
                  if (selected)
                    BoxShadow(color: colors.accent, spreadRadius: 2),
                ],
              ),
              child: ClipRRect(
                borderRadius: radius,
                child: ExcludeSemantics(
                  child: FutureBuilder<String?>(
                    future: artworkPath,
                    builder: (context, snapshot) {
                      final path = snapshot.data;
                      if (path == null) return placeholder;
                      return Image(
                        image: ResizeImage(
                          FileImage(File(path)),
                          width: pixels,
                          height: pixels,
                        ),
                        fit: BoxFit.cover,
                        errorBuilder: (context, error, stack) => placeholder,
                      );
                    },
                  ),
                ),
              ),
            ),
          ),
          const SizedBox(height: _captionGap),
          Text(
            album.id.title,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: MtType.albumTitle.copyWith(
              color: selected ? colors.accent : colors.textPrimary,
            ),
          ),
          if (album.id.artist case final artist?) ...[
            const SizedBox(height: _artistGap),
            Text(
              artist,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: MtType.albumArtist.copyWith(color: colors.textCount),
            ),
          ],
        ],
      ),
    );
  }
}
