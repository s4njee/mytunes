import 'package:flutter/widgets.dart';

import 'src/equalizer_settings.dart';
import 'theme/mt_theme.dart';
import 'theme/tokens.g.dart';

class Equalizer extends StatelessWidget {
  const Equalizer({
    super.key,
    required this.settings,
    required this.selection,
    required this.onSelect,
    required this.onAdjust,
    required this.onToggle,
  });

  final EqualizerSettings settings;
  final int selection;
  final ValueChanged<int> onSelect;
  final void Function(int band, int direction) onAdjust;
  final ValueChanged<EqualizerOption> onToggle;

  @override
  Widget build(BuildContext context) {
    final colors = MtTheme.of(context).colors;
    final frequencies = EqualizerSettings.frequencies;
    return ListView(
      padding: const EdgeInsets.only(top: 20),
      children: [
        SizedBox(
          height: MtSize.eqBandRow,
          child: Row(
            spacing: MtSize.eqBandGap,
            children: [
              for (var band = 0; band < frequencies.length; band++)
                Expanded(
                  child: _Band(
                    frequency: frequencies[band],
                    gain: settings.gains[band],
                    selected: selection == band,
                    onSelect: () => onSelect(band),
                    onAdjust: (direction) => onAdjust(band, direction),
                  ),
                ),
            ],
          ),
        ),
        const SizedBox(height: 18),
        SizedBox(height: 1, child: ColoredBox(color: colors.rule)),
        const SizedBox(height: 14),
        for (final option in EqualizerOption.values)
          _Option(
            option: option,
            enabled: settings.enabled(option),
            selected: selection == frequencies.length + option.index,
            onSelect: () => onSelect(frequencies.length + option.index),
            onToggle: () => onToggle(option),
          ),
      ],
    );
  }
}

class _Band extends StatelessWidget {
  const _Band({
    required this.frequency,
    required this.gain,
    required this.selected,
    required this.onSelect,
    required this.onAdjust,
  });

  final String frequency;
  final int gain;
  final bool selected;
  final VoidCallback onSelect;
  final ValueChanged<int> onAdjust;

  @override
  Widget build(BuildContext context) {
    final colors = MtTheme.of(context).colors;
    final signedGain = gain > 0 ? '+$gain' : '$gain';
    return Semantics(
      container: true,
      selected: selected,
      label: '$frequency hertz',
      value: '$signedGain decibels',
      slider: true,
      onDidGainAccessibilityFocus: onSelect,
      onIncrease: gain < 6 ? () => onAdjust(1) : null,
      onDecrease: gain > -6 ? () => onAdjust(-1) : null,
      increasedValue: gain < 6 ? '${gain + 1} decibels' : null,
      decreasedValue: gain > -6 ? '${gain - 1} decibels' : null,
      excludeSemantics: true,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        spacing: 9,
        children: [
          Expanded(
            child: ClipRRect(
              borderRadius: BorderRadius.circular(MtRadius.eqWell),
              child: LayoutBuilder(
                builder: (context, constraints) {
                  final centre = constraints.maxHeight / 2;
                  final height = centre * gain.abs() / 6;
                  return ColoredBox(
                    color: colors.well,
                    child: Stack(
                      fit: StackFit.expand,
                      children: [
                        Positioned(
                          top: centre,
                          left: 0,
                          right: 0,
                          height: 1,
                          child: ColoredBox(color: colors.eqZeroLine),
                        ),
                        Positioned(
                          top: gain > 0 ? centre - height : centre,
                          left: 0,
                          right: 0,
                          height: height,
                          child: DecoratedBox(
                            decoration: BoxDecoration(
                              borderRadius: BorderRadius.circular(2),
                              color: selected
                                  ? colors.accent
                                  : colors.accentMuted,
                            ),
                          ),
                        ),
                      ],
                    ),
                  );
                },
              ),
            ),
          ),
          Text(
            frequency,
            textAlign: TextAlign.center,
            maxLines: 1,
            style: MtType.eqHz.copyWith(
              color: selected ? colors.accent : colors.textTimecode,
            ),
          ),
          Text(
            signedGain,
            textAlign: TextAlign.center,
            maxLines: 1,
            style: MtType.eqGain.copyWith(color: colors.textCount),
          ),
        ],
      ),
    );
  }
}

class _Option extends StatelessWidget {
  const _Option({
    required this.option,
    required this.enabled,
    required this.selected,
    required this.onSelect,
    required this.onToggle,
  });

  final EqualizerOption option;
  final bool enabled;
  final bool selected;
  final VoidCallback onSelect;
  final VoidCallback onToggle;

  @override
  Widget build(BuildContext context) {
    final colors = MtTheme.of(context).colors;
    return Semantics(
      container: true,
      selected: selected,
      toggled: enabled,
      onDidGainAccessibilityFocus: onSelect,
      onTap: onToggle,
      child: SizedBox(
        height: MtSize.statusRow + 2,
        child: Row(
          spacing: MtSpace.rowGap,
          children: [
            Expanded(
              child: Text(
                option.label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: MtType.statusLabel.copyWith(
                  color: selected ? colors.accent : colors.textLabel,
                ),
              ),
            ),
            Text(
              enabled ? 'ON' : 'OFF',
              style: MtType.timecode.copyWith(
                fontWeight: FontWeight.w500,
                letterSpacing: 1.2,
                color: enabled ? colors.accent : colors.textCount,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
