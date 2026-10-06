# MixInspo DJ — User Guide

## Connecting Traktor Pro (automatic now-playing)

Traktor detection works now. The setup steps below are the ones to use.

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

## Using Traktor's BPM and key

MixInspo DJ reads Traktor's own collection file, `collection.nml`, which Traktor
keeps in `Documents/Native Instruments/Traktor <version>/`. It only reads the
file; it never changes it.

Traktor writes this file when you save or quit, so the BPM and key come from
the last save. After you analyse tracks in Traktor and save, they appear in
MixInspo DJ within about 30 seconds. For those tracks, Traktor's BPM and key
replace MixInspo DJ's own estimates, and analysis never overwrites them.

Traktor's file also lists tracks that aren't in your watched folders yet. Any
that are on this computer are added to the library.

## Analysis in the background

MixInspo DJ analyses your library in the background, one track at a time. It
pauses while Traktor is broadcasting to the app, so it never runs during a live
set. Analysis is slow on a large library. Use the release build, not a debug
build, for real sessions.

## Cloud-only files

If a folder is synced by Dropbox (or iCloud) with "online only" files, those
files aren't on your Mac until you open them. MixInspo DJ skips them, because
reading one would make the cloud service download it. A track is added once its
file is downloaded. The scan summary in Settings shows how many were skipped.

## Keeping your library up to date

Add your music folders in **Settings → Select Music Folder**. MixInspo DJ then
watches them and adds any new file within about 30 seconds, so a track you
download and play in Traktor should be found. If it isn't in your folders yet,
the yellow line under NOW PLAYING says it's still looking for about a minute.
To stop watching a folder, use **Stop watching** in Settings. Your files aren't
touched.
