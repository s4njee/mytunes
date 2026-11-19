import { createSignal } from "solid-js";
import { LibraryWindow, type SourceGroup, type Theme, type Track } from "../library/LibraryWindow";
import cover from "./aerial-lines.webp";

const initialTrack: Track = {
  id: "breakwater",
  name: "Breakwater",
  seconds: 278,
  artist: "Haven Circuit",
  album: "Aerial Lines",
  genre: "Electronic",
  rating: 5,
  plays: 24,
  checked: true,
};

const songs: Track[] = [
  {
    id: "afterimage",
    name: "Afterimage",
    seconds: 249,
    artist: "Aster Field",
    album: "Soft Focus",
    genre: "Ambient",
    rating: 4,
    plays: 18,
    checked: true,
  },
  {
    id: "blue-static",
    name: "Blue Static",
    seconds: 231,
    artist: "Golden Hour Club",
    album: "In Between",
    genre: "Indie",
    rating: 3,
    plays: 6,
    checked: true,
  },
  initialTrack,
  {
    id: "cold-meridian",
    name: "Cold Meridian",
    seconds: 322,
    artist: "Tessa North",
    album: "Paper Atlas",
    genre: "Ambient",
    rating: 4,
    plays: 12,
    checked: true,
  },
  {
    id: "daymark",
    name: "Daymark",
    seconds: 214,
    artist: "Glass Harbor",
    album: "Daymark",
    genre: "Indie",
    rating: 4,
    plays: 8,
    checked: true,
  },
  {
    id: "fallow",
    name: "Fallow",
    seconds: 306,
    artist: "Kite Assembly",
    album: "Open Water",
    genre: "Jazz",
    rating: 3,
    plays: 9,
    checked: false,
  },
  {
    id: "glass-current",
    name: "Glass Current",
    seconds: 345,
    artist: "Haven Circuit",
    album: "Aerial Lines",
    genre: "Electronic",
    rating: 4,
    plays: 19,
    checked: true,
  },
  {
    id: "half-moon",
    name: "Half Moon",
    seconds: 203,
    artist: "Luna Workshop",
    album: "Window Seat",
    genre: "Jazz",
    rating: 4,
    plays: 17,
    checked: true,
  },
  {
    id: "in-the-reeds",
    name: "In the Reeds",
    seconds: 242,
    artist: "Tessa North",
    album: "Paper Atlas",
    genre: "Folk",
    rating: 3,
    plays: 7,
    checked: true,
  },
  {
    id: "last-light",
    name: "Last Light",
    seconds: 195,
    artist: "Aster Field",
    album: "Soft Focus",
    genre: "Ambient",
    rating: 4,
    plays: 14,
    checked: true,
  },
  {
    id: "low-orbit",
    name: "Low Orbit",
    seconds: 265,
    artist: "Haven Circuit",
    album: "Aerial Lines",
    genre: "Electronic",
    rating: 5,
    plays: 23,
    checked: true,
  },
  {
    id: "night-ferry",
    name: "Night Ferry",
    seconds: 216,
    artist: "Glass Harbor",
    album: "Daymark",
    genre: "Indie",
    rating: 3,
    plays: 4,
    checked: false,
  },
  {
    id: "open-water",
    name: "Open Water",
    seconds: 299,
    artist: "Kite Assembly",
    album: "Open Water",
    genre: "Jazz",
    rating: 5,
    plays: 27,
    checked: true,
  },
  {
    id: "paper-lanterns",
    name: "Paper Lanterns",
    seconds: 248,
    artist: "Golden Hour Club",
    album: "In Between",
    genre: "Indie",
    rating: 4,
    plays: 11,
    checked: true,
  },
  {
    id: "quiet-hours",
    name: "Quiet Hours",
    seconds: 287,
    artist: "Haven Circuit",
    album: "Aerial Lines",
    genre: "Electronic",
    rating: 4,
    plays: 15,
    checked: true,
  },
  {
    id: "salt-lantern",
    name: "Salt Lantern",
    seconds: 177,
    artist: "Luna Workshop",
    album: "Window Seat",
    genre: "Jazz",
    rating: 3,
    plays: 5,
    checked: true,
  },
  {
    id: "tidelines",
    name: "Tidelines",
    seconds: 277,
    artist: "Mira Park",
    album: "East of Morning",
    genre: "Folk",
    rating: 4,
    plays: 16,
    checked: true,
  },
  {
    id: "window-seat",
    name: "Window Seat",
    seconds: 232,
    artist: "Luna Workshop",
    album: "Window Seat",
    genre: "Jazz",
    rating: 5,
    plays: 21,
    checked: true,
  },
];

const sources: SourceGroup[] = [
  { label: "Library", sources: [{ name: "Music", icon: "music" }] },
  { label: "Devices", sources: [{ name: "MyTunes Pod", icon: "phone", eject: true }] },
  {
    label: "Playlists",
    sources: [
      { name: "Recently Added", icon: "clock" },
      { name: "Late Night", icon: "playlist" },
      { name: "Coast Drive", icon: "playlist" },
      { name: "Studio Notes", icon: "playlist" },
    ],
  },
];

const facetValues = [
  {
    label: "Genre",
    values: ["All (6 genres)", "Ambient", "Electronic", "Folk", "Indie", "Jazz", "Soundtrack"],
  },
  {
    label: "Artist",
    values: [
      "All (8 artists)",
      "Aster Field",
      "Glass Harbor",
      "Golden Hour Club",
      "Haven Circuit",
      "Kite Assembly",
      "Luna Workshop",
      "Mira Park",
      "Tessa North",
    ],
  },
  {
    label: "Album",
    values: [
      "All (8 albums)",
      "Aerial Lines",
      "Daymark",
      "East of Morning",
      "In Between",
      "Open Water",
      "Paper Atlas",
      "Soft Focus",
      "Window Seat",
    ],
  },
];

export function DesktopMock() {
  const param = new URLSearchParams(window.location.search).get("theme");
  const preferred = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  const [theme, setTheme] = createSignal<Theme>(
    param === "light" || param === "dark" ? param : preferred,
  );
  const [source, setSource] = createSignal("Music");
  const [activeFacets, setFacets] = createSignal<Record<string, string>>({});
  const [track, setTrack] = createSignal(initialTrack);
  const [selected, setSelected] = createSignal(initialTrack.id);
  const [playing, setPlaying] = createSignal(true);
  const [position, setPosition] = createSignal(42);
  const [volume, setVolume] = createSignal(65);
  const [browser, setBrowser] = createSignal(true);
  const [artwork, setArtwork] = createSignal(true);
  document.documentElement.dataset.theme = theme();

  const playTrack = (song: Track) => {
    setTrack(song);
    setSelected(song.id);
    setPosition(0);
    setPlaying(true);
  };

  const step = (direction: number) => {
    const index = songs.indexOf(track());
    const song = songs[(index + songs.length + direction) % songs.length];
    if (song) playTrack(song);
  };

  return (
    <LibraryWindow
      tracks={songs}
      track={track()}
      selectedId={selected()}
      sources={sources}
      activeSource={source()}
      facets={facetValues.map((facet) => ({
        ...facet,
        active: activeFacets()[facet.label] ?? facet.values[0] ?? "",
      }))}
      playing={playing()}
      position={position()}
      volume={volume()}
      artwork={track().album === "Aerial Lines" ? cover : undefined}
      showArtwork={artwork()}
      showBrowser={browser()}
      theme={theme()}
      summary="1,284 songs, 3.4 days, 8.12 GB"
      onSource={(name) => setSource(name)}
      onFacet={(label, value) => setFacets((old) => ({ ...old, [label]: value }))}
      onSelect={(song) => setSelected(song.id)}
      onPlayTrack={playTrack}
      onTogglePlaying={() => setPlaying((active) => !active)}
      onPrevious={() => step(-1)}
      onNext={() => step(1)}
      onPosition={(seconds) => setPosition(seconds)}
      onVolume={(level) => setVolume(level)}
      onToggleArtwork={() => setArtwork((visible) => !visible)}
      onToggleBrowser={() => setBrowser((visible) => !visible)}
      onTheme={(mode) => {
        setTheme(mode);
        document.documentElement.dataset.theme = mode;
      }}
    />
  );
}
