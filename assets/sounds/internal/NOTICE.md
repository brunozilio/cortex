# iOS interface sounds

Source: the user-provided `iOSSystemSounds-main/mp3` collection, published
by David Blue at https://github.com/extratone/iOSSystemSounds. Its README
describes extraction from Apple's `System/Library/Audio/UISounds`.
The upstream repository includes the Unlicense (Dave Edition), preserved
in `LICENSE-upstream.md`; the original system audio is attributed to Apple.

Only clips used by the app are bundled. MP3 inputs were converted with
FFmpeg to mono, 44.1 kHz, signed 16-bit PCM WAV, preserving their names:

```sh
ffmpeg -i "$source.mp3" -map_metadata -1 -ac 1 -ar 44100 -c:a pcm_s16le "$target.wav"
```

| Cue | Source file stems (variants rotate) |
|---|---|
| Button | `SentMessage` |
| Select | `Tink` |
| Tap | `focus_change_small`, `focus_change_keyboard`, `focus_change_app_icon` |
| Type | `keyboard_press_normal`, `key_press_click`, `key_press_modifier` |
| Swipe | `shake` |
| Toggle on | `jbl_begin` |
| Toggle off | `jbl_cancel` |
| Transition up | `navigation_push` |
| Transition down | `navigation_pop` |
| Notification | `ReceivedMessage` |
| Caution | `payment_failure` |
| Disabled | `jbl_no_match` |
| Celebration | `payment_success` |
