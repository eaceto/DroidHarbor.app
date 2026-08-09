# dh-qs-core

The Quick Share front door: adapts
[rquickshare](https://github.com/Martichou/rquickshare)'s `rqs_lib`
(GPL-3.0; our fork `eaceto/rquickshare` branch `aft`, rev-pinned) to the
`dh-domain` front-door seam.

`rqs_lib` owns the protocol: mDNS advertisement, UKEY2 handshake, encrypted
channel, payload reception. This crate owns the translation, and routes
received files through per-session staging subdirectories so `dh-core`'s
atomic finalization still controls what appears in the user's folder.

## API

- `spawn(QuickShareConfig { staging_dir, port, device_name }) ->
  (FrontDoorChannels, JoinHandle)` starts the service (initially invisible)
  and the adapter task; hand the channels to `dh_domain::spawn`. Stops when
  the domain engine shuts down. Clears staging leftovers on startup.

## Mapping

| rqs_lib | domain |
|---|---|
| `WaitingForUserConsent` + metadata | `Connected` (PIN as token), then `Introduction` (per-file names and sizes from `file_infos`) |
| `ReceivingFiles` ack bytes + `file_infos` | `Progress` (batch bytes + current file) |
| `Finished` | `FileStaged` per completed file at its exact staged path, then `Ended(Completed)` |
| `Rejected` (+ `ConsentTimeout` error) | `Ended(TimedOut)` |
| `Rejected` / `Cancelled` / `Disconnected` | `Ended` with attributed reason; partial files deleted |
| `StartAdvertising` / `StopAdvertising` | mDNS visibility toggle |
| `Respond` / `Cancel` | `ChannelMessage` actions on rqs_lib's bus |
| `StartDiscovery` / `StopDiscovery` | mDNS browsing for receive-ready phones (`EndpointUpdated` signals) |
| `SendFiles` | `SendInfo` into rqs_lib's outbound path; `SentIntroduction` → `SendAwaitingConsent`, `SendingFiles` → `Progress`, then `Ended` |
| `SendText` | the same path with a text payload; the phone is told whether it is a link, an address or a phone number so it can offer the right action |

Fork features relied upon: `file_infos` (per-file size/progress/path),
`ChannelMessage.error` classification, consent auto-reject (60 s default),
per-session staging subdirectories, synthetic mDNS SRV hostname.

## Known limitations

- The advertised name is fixed at construction
  (`QuickShareConfig::device_name`, hostname when unset); renaming while
  running requires a service restart.
- Text/URL payloads are acknowledged but not yet surfaced (M2 candidate).
- Building requires `protoc` (`brew install protobuf`).
