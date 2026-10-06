# MixInspo DJ — User Guide

## Connecting Traktor Pro (automatic now-playing)

> Status: setup steps below are the planned final ones. Automatic detection is
> still being built, so until that ships, set tracks as NOW PLAYING by search
> or drag.

MixInspo DJ listens for Traktor's **Broadcasting** feature on your own
machine. Nothing leaves your computer.

In Traktor Pro, open **Preferences → Broadcasting** and fill in **Server Settings**:

| Field | Value | Why |
|---|---|---|
| Address | `127.0.0.1` | Your own machine only |
| Port | `8765` | The port MixInspo DJ listens on |
| Mount path | `/mixinspo` | Any name starting with `/` works |
| Password | `mixinspo` | Any value; it only protects this local connection |
| Format | Ogg Vorbis (default) | Leave as is |

**Metadata Settings** (Stream Name, Description, Genre) don't affect track
detection. Leave them as they are.

Then:

1. Close Preferences.
2. Turn Broadcasting on with the antenna icon in the Audio Recorder section.
3. Play a track for about 20 seconds.

Traktor can take a while to send track details. If MixInspo DJ doesn't pick
up the track, check that Broadcasting is on, that the address and port match
exactly, and that no other app is using port 8765. Changing the port to
another number in both places also works.

Note: Traktor sends the next track's details when you cue it, so the detected
track can change slightly before you hear it.

## Keeping your library up to date

Add your music folders in **Settings → Select Music Folder**. MixInspo DJ then
watches them and adds any new file within about 30 seconds, so a track you
download and play in Traktor should be found. If it isn't in your folders yet,
the yellow line under NOW PLAYING says it's still looking for about a minute.
To stop watching a folder, use **Stop watching** in Settings. Your files aren't
touched.
