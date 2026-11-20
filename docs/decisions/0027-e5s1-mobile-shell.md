# 0027 — E5.S1 Mobile shell and theming

The five wheel screens will share one Flutter display and deck layout.
`MtTheme` selects the generated 1a and 2a palettes from the system
appearance. The shell uses the handoff padding and makes room for any
larger safe-area insets. The OS draws the status and navigation bars
using foreground colours selected by the theme.

Android and iOS declare portrait orientation before Flutter starts. The
Android activity uses the Android 16 compatibility property for large
screens so the portrait requirement also applies to tablets on that
version. The iPad configuration requests full-screen presentation for
the same reason. [Android orientation rules](https://developer.android.com/about/versions/16/behavior-changes-16), [Flutter orientation API](https://api.flutter.dev/flutter/services/SystemChrome/setPreferredOrientations.html)

The app bundles IBM Plex Sans 400, 500, 600, and 700 and JetBrains Mono
400 and 500. It registers both SIL Open Font licences with Flutter's
licences page. The route is ready for the About entry in Settings.
[IBM Plex Sans release](https://github.com/IBM/plex/releases/tag/%40ibm/plex-sans%401.3.6), [JetBrains Mono release](https://github.com/JetBrains/JetBrainsMono/releases/tag/v2.304)

Flutter tests checked theme changes in the same widget tree at 402 × 874
logical pixels, header alignment, and licence registration and routing.
An Android 16 emulator at the same logical size showed both palettes and
the native bars. The activity finished in portrait after landscape was
requested at phone and tablet widths. Physical iOS and Android checks
remain pending. The checkout
has the written and HTML handoff, and no reference PNGs are available for
the five-screen golden comparisons.
