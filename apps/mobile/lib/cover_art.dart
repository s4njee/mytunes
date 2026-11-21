import 'dart:io';

import 'package:flutter/widgets.dart';

import 'theme/mt_theme.dart';
import 'theme/tokens.g.dart';

/// The local thumbnail or a captioned tile using the active theme.
class CoverArt extends StatelessWidget {
  const CoverArt({
    super.key,
    required this.caption,
    required this.paletteIndex,
    required this.artworkPath,
    required this.width,
    required this.radius,
    this.captionInset = 10,
  });

  final String caption;
  final int paletteIndex;
  final Future<String?>? artworkPath;
  final double width;
  final BorderRadius radius;
  final double captionInset;

  @override
  Widget build(BuildContext context) {
    final palette = MtTheme.of(context).colors.placeholderArt;
    final (fill, foreground) = palette[paletteIndex % palette.length];
    final placeholder = ColoredBox(
      color: fill,
      child: Align(
        alignment: Alignment.bottomLeft,
        child: Padding(
          padding: EdgeInsets.all(captionInset),
          child: Text(
            caption.toUpperCase(),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: MtType.artCaption.copyWith(color: foreground),
          ),
        ),
      ),
    );
    final pixels = (width * 2).round();
    return ClipRRect(
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
    );
  }
}
